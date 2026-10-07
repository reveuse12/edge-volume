#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod gesture;
mod instance;
mod recovery;
#[cfg(target_os = "macos")]
use gesture::Touch;
use gesture::{Gesture, Settings};
use recovery::AudioGuard;
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, OnceLock,
    },
    time::{Duration, Instant},
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};
#[cfg(target_os = "macos")]
extern "C" {
    fn edge_start(callback: extern "C" fn(i32, i32, f32, f32)) -> i32;
    fn edge_volume(value: *mut f32, write: i32) -> i32;
    fn edge_restart() -> i32;
    fn edge_option_down() -> i32;
    fn edge_audio_state(value: *mut f32, device: *mut u32, muted: *mut i32) -> i32;
    fn edge_adjust_volume(
        delta: f32,
        expected: u32,
        before: *mut f32,
        after: *mut f32,
        muted: *mut i32,
    ) -> i32;
    fn edge_watch_system(
        callback: extern "C" fn(i32, *const std::ffi::c_char, *const std::ffi::c_char),
    );
    fn edge_running_apps() -> *mut std::ffi::c_char;
    fn edge_free_string(value: *mut std::ffi::c_char);
    fn edge_show_overlay(volume: f32, label: *const std::ffi::c_char, visible: i32);
}
#[cfg(target_os = "windows")]
extern "C" {
    fn edge_windows_start() -> i32;
    fn edge_windows_volume(value: *mut f32, write: i32) -> i32;
    fn edge_windows_diagnostics(devices: *mut i32, reports: *mut u64, state: *mut i32);
}
#[derive(Serialize, Clone)]
struct WindowsDiagnostics {
    devices: i32,
    reports: u64,
    listener: i32,
}
#[derive(Serialize, Clone)]
struct Feedback {
    volume: f32,
    change: f32,
    label: String,
}
#[derive(Serialize, Clone)]
struct Status {
    settings: Settings,
    platform: String,
    diagnostics: Option<WindowsDiagnostics>,
    input: String,
    volume: Option<f32>,
    muted: Option<bool>,
    audio_available: bool,
    pause_reason: Option<String>,
    error: Option<String>,
    x: f32,
    y: f32,
    contacts: i32,
    frames: u64,
    feedback: Option<Feedback>,
}
struct Engine {
    status: Status,
    gesture: Gesture,
    path: PathBuf,
    feedback_until: Option<Instant>,
    started: Instant,
    audio: AudioGuard,
    front_app: Option<(String, String)>,
    suspended: bool,
    retry_at: Option<Instant>,
    _instance: std::fs::File,
}
static ENGINE: OnceLock<Mutex<Engine>> = OnceLock::new();
static RECONNECT: AtomicBool = AtomicBool::new(false);
impl Engine {
    fn pause_reason(&self) -> Option<String> {
        if self.suspended {
            return Some("Computer asleep or session inactive".into());
        }
        if self.status.input != "Listening" && self.status.platform == "macos" {
            return Some(self.status.input.clone());
        }
        if let Some((id, name)) = &self.front_app {
            if self
                .status
                .settings
                .excluded_apps
                .iter()
                .any(|excluded| excluded.eq_ignore_ascii_case(id))
            {
                return Some(format!("Paused in {name}"));
            }
        }
        if !self.status.audio_available {
            return Some("Waiting for an adjustable audio output".into());
        }
        None
    }
    fn cancel(&mut self) {
        self.gesture.cancel();
        self.feedback_until = None;
        self.status.feedback = None;
    }
}
fn refresh_audio(e: &mut Engine) {
    #[cfg(target_os = "macos")]
    let (device, value, muted, error) = {
        let (mut value, mut device, mut muted) = (0.0, 0, -1);
        let code = unsafe { edge_audio_state(&mut value, &mut device, &mut muted) };
        let valid = code == 0 && value.is_finite() && (0.0..=1.0).contains(&value);
        (device,valid.then_some(value), (muted>=0).then_some(muted!=0),
            (!valid).then(||format!("Audio output unavailable (code {code}). Gestures resume when an adjustable output is selected.")))
    };
    #[cfg(not(target_os = "macos"))]
    let (device, value, muted, error) = match volume(None) {
        Ok(value) => (0, Some(value), None, None),
        Err(error) => (0, None, None, Some(error)),
    };
    if e.audio.update(device, value.is_some()) {
        e.cancel();
    }
    e.status.volume = value;
    e.status.muted = muted;
    e.status.audio_available = value.is_some();
    e.status.error = error;
    e.status.pause_reason = e.pause_reason();
}
#[cfg(target_os = "macos")]
extern "C" fn system_event(event: i32, id: *const std::ffi::c_char, name: *const std::ffi::c_char) {
    if let Some(engine) = ENGINE.get() {
        if let Ok(mut e) = engine.lock() {
            match event {
                0 => {
                    e.suspended = true;
                    e.retry_at = None;
                    e.status.input = "Computer asleep or session inactive".into();
                    RECONNECT.store(false, Ordering::SeqCst);
                    e.cancel();
                }
                1 | 3 if event == 1 || !e.suspended => {
                    e.suspended = false;
                    e.status.input = "Reconnecting trackpad".into();
                    e.cancel();
                    RECONNECT.store(true, Ordering::SeqCst);
                }
                2 => {
                    e.front_app = if id.is_null() {
                        None
                    } else {
                        let id = unsafe { std::ffi::CStr::from_ptr(id) }
                            .to_string_lossy()
                            .into_owned();
                        let name = if name.is_null() {
                            id.clone()
                        } else {
                            unsafe { std::ffi::CStr::from_ptr(name) }
                                .to_string_lossy()
                                .into_owned()
                        };
                        Some((id, name))
                    };
                    e.cancel();
                }
                _ => {}
            }
            e.status.pause_reason = e.pause_reason();
        }
    }
}
#[derive(Serialize, serde::Deserialize)]
struct RunningApp {
    id: String,
    name: String,
}
#[tauri::command]
fn get_running_apps() -> Result<Vec<RunningApp>, String> {
    #[cfg(target_os = "macos")]
    {
        let pointer = unsafe { edge_running_apps() };
        if pointer.is_null() {
            return Err("Could not list running apps".into());
        }
        let text = unsafe { std::ffi::CStr::from_ptr(pointer) }
            .to_string_lossy()
            .into_owned();
        unsafe {
            edge_free_string(pointer);
        }
        let mut apps: Vec<RunningApp> = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        apps.sort_by(|a, b| a.id.cmp(&b.id));
        apps.dedup_by(|a, b| a.id == b.id);
        apps.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(apps)
    }
    #[cfg(not(target_os = "macos"))]
    Ok(Vec::new())
}
#[tauri::command]
fn reconnect_trackpad() -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Err("Trackpad recovery is currently available on macOS only".into());
    }
    let mut e = ENGINE
        .get()
        .unwrap()
        .lock()
        .map_err(|_| "Engine unavailable")?;
    e.status.input = "Reconnecting trackpad".into();
    e.cancel();
    RECONNECT.store(true, Ordering::SeqCst);
    Ok(())
}
fn volume(write: Option<f32>) -> Result<f32, String> {
    #[cfg(target_os = "macos")]
    {
        let mut v = write.unwrap_or(0.0);
        let code = unsafe { edge_volume(&mut v, if write.is_some() { 1 } else { 0 }) };
        if code == 0 {
            Ok(v)
        } else {
            Err(format!(
                "Output device does not expose adjustable volume (code {code})."
            ))
        }
    }
    #[cfg(target_os = "windows")]
    {
        let mut value = write.unwrap_or(0.0);
        let code = unsafe { edge_windows_volume(&mut value, i32::from(write.is_some())) };
        if code >= 0 {
            Ok(value)
        } else {
            Err(format!(
                "Windows output volume unavailable (HRESULT 0x{:08X}).",
                code as u32
            ))
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = write;
        Err("This platform is not supported.".into())
    }
}
#[cfg(target_os = "macos")]
extern "C" fn contact(count: i32, id: i32, x: f32, y: f32) {
    if let Some(engine) = ENGINE.get() {
        if let Ok(mut e) = engine.lock() {
            e.status.x = if x.is_finite() {
                x.clamp(0.0, 1.0)
            } else {
                0.0
            };
            e.status.y = if y.is_finite() {
                y.clamp(0.0, 1.0)
            } else {
                0.0
            };
            e.status.contacts = count.max(0);
            e.status.frames += 1;
            if count == 0 {
                e.gesture.reset();
                return;
            }
            if e.pause_reason().is_some() {
                e.gesture.cancel();
                return;
            }
            let settings = e.status.settings.clone();
            let at = e.started.elapsed();
            let modifier = unsafe { edge_option_down() != 0 };
            if let Some(delta) = e.gesture.feed(
                &settings,
                Touch {
                    count,
                    id,
                    x,
                    y,
                    at,
                    modifier,
                },
            ) {
                if delta.abs() < 0.0001 {
                    return;
                }
                let (mut before, mut after, mut muted) = (0.0, 0.0, -1);
                let code = unsafe {
                    edge_adjust_volume(delta, e.audio.device, &mut before, &mut after, &mut muted)
                };
                if code != 0 || !after.is_finite() || !(0.0..=1.0).contains(&after) {
                    e.cancel();
                    refresh_audio(&mut e);
                    e.status.error=Some(format!("Volume adjustment interrupted (code {code}). Lift your finger and try again."));
                    return;
                }
                let change = after - before;
                e.status.volume = Some(after);
                e.status.muted = (muted >= 0).then_some(muted != 0);
                e.status.error = None;
                let label = if muted == 1 {
                    "Muted"
                } else if change > 0.00001 {
                    "Increasing"
                } else if change < -0.00001 {
                    "Decreasing"
                } else if after >= 0.999 {
                    "Maximum volume"
                } else if after <= 0.001 {
                    "Minimum volume"
                } else {
                    "Volume unchanged"
                };
                e.status.feedback = Some(Feedback {
                    volume: after,
                    change,
                    label: label.into(),
                });
                e.feedback_until = Some(Instant::now() + Duration::from_millis(1400));
            }
        }
    }
}
#[tauri::command]
fn get_status() -> Result<Status, String> {
    let mut e = ENGINE
        .get()
        .unwrap()
        .lock()
        .map_err(|_| "Engine unavailable")?;
    refresh_audio(&mut e);
    #[cfg(target_os = "windows")]
    {
        let (mut devices, mut reports, mut listener) = (0, 0, 0);
        unsafe {
            edge_windows_diagnostics(&mut devices, &mut reports, &mut listener);
        }
        e.status.diagnostics = Some(WindowsDiagnostics {
            devices,
            reports,
            listener,
        });
        e.status.input = if listener < 0 {
            format!("Touchpad diagnostic failed (code {listener})")
        } else if listener == 0 {
            "Touchpad diagnostic is not listening".into()
        } else if devices < 0 {
            "Touchpad device enumeration failed".into()
        } else if devices == 0 {
            "No touchpad HID collection exposed".into()
        } else {
            "Touchpad diagnostic active; edge gestures pending".into()
        };
    }
    Ok(e.status.clone())
}
#[tauri::command]
fn save_settings(settings: Settings) -> Result<(), String> {
    if !settings.valid() {
        return Err("Invalid edge or sensitivity settings".into());
    }
    let mut e = ENGINE
        .get()
        .unwrap()
        .lock()
        .map_err(|_| "Engine unavailable")?;
    if settings.enabled && e.status.input != "Listening" {
        return Err(e.status.input.clone());
    }
    let data = serde_json::to_vec_pretty(&settings).map_err(|e| e.to_string())?;
    let tmp = e.path.with_extension("tmp");
    std::fs::write(&tmp, data).map_err(|e| e.to_string())?;
    std::fs::rename(tmp, &e.path).map_err(|e| e.to_string())?;
    e.status.settings = settings;
    e.cancel();
    e.status.pause_reason = e.pause_reason();
    Ok(())
}
#[tauri::command]
fn get_feedback() -> Result<Option<Feedback>, String> {
    let e = ENGINE
        .get()
        .unwrap()
        .lock()
        .map_err(|_| "Engine unavailable")?;
    Ok(e.status.feedback.clone())
}
#[tauri::command]
fn preview_feedback() -> Result<(), String> {
    let v = volume(None)?;
    let mut e = ENGINE
        .get()
        .unwrap()
        .lock()
        .map_err(|_| "Engine unavailable")?;
    e.status.feedback = Some(Feedback {
        volume: v,
        change: 0.0,
        label: "Current volume".into(),
    });
    e.feedback_until = Some(Instant::now() + Duration::from_secs(5));
    Ok(())
}
#[tauri::command]
fn test_windows_volume(delta: f32) -> Result<(), String> {
    if !cfg!(target_os = "windows") {
        return Err("Windows audio test only".into());
    }
    if delta != 0.05 && delta != -0.05 {
        return Err("Choose a five percentage point step".into());
    }
    let before = volume(None)?;
    volume(Some((before + delta).clamp(0.0, 1.0)))?;
    let after = volume(None)?;
    let mut e = ENGINE
        .get()
        .unwrap()
        .lock()
        .map_err(|_| "Engine unavailable")?;
    e.status.volume = Some(after);
    e.status.feedback = Some(Feedback {
        volume: after,
        change: after - before,
        label: if after > before {
            "Increasing"
        } else if after < before {
            "Decreasing"
        } else {
            "Volume unchanged"
        }
        .into(),
    });
    e.feedback_until = Some(Instant::now() + Duration::from_millis(1400));
    Ok(())
}
fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            std::fs::create_dir_all(&dir)?;
            let Some(instance) = instance::acquire(&dir.join("instance.lock"))? else {
                // Exit before registering any input callback or audio writer.
                std::process::exit(0);
            };
            let path = dir.join("settings.json");
            let mut settings = std::fs::read(&path)
                .ok()
                .and_then(|v| serde_json::from_slice::<Settings>(&v).ok())
                .filter(Settings::valid)
                .unwrap_or_default();
            // Explicit enable on each launch while the private input adapter is experimental.
            settings.enabled = false;
            ENGINE
                .set(Mutex::new(Engine {
                    status: Status {
                        settings,
                        platform: std::env::consts::OS.into(),
                        diagnostics: None,
                        input: "Starting".into(),
                        volume: None,
                        muted: None,
                        audio_available: false,
                        pause_reason: None,
                        error: None,
                        x: 0.0,
                        y: 0.0,
                        contacts: 0,
                        frames: 0,
                        feedback: None,
                    },
                    gesture: Gesture::default(),
                    path,
                    feedback_until: None,
                    started: Instant::now(),
                    audio: AudioGuard::default(),
                    front_app: None,
                    suspended: false,
                    retry_at: None,
                    _instance: instance,
                }))
                .ok();
            refresh_audio(&mut ENGINE.get().unwrap().lock().unwrap());
            #[cfg(target_os = "macos")]
            let input = match unsafe { edge_start(contact) } {
                0 => "Listening".into(),
                c => format!("Trackpad unavailable (code {c})"),
            };
            #[cfg(target_os = "windows")]
            let input = {
                unsafe {
                    edge_windows_start();
                }
                "Touchpad diagnostic active; edge gestures pending".to_string()
            };
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            let input = "Unsupported platform".to_string();
            {
                let mut e = ENGINE.get().unwrap().lock().unwrap();
                e.status.input = input;
                if e.status.platform == "macos" && e.status.input != "Listening" {
                    e.retry_at = Some(Instant::now() + Duration::from_secs(3));
                }
                e.status.pause_reason = e.pause_reason();
            }
            #[cfg(target_os = "macos")]
            unsafe {
                edge_watch_system(system_event);
            }
            let show = MenuItem::with_id(app, "show", "Preferences", true, None::<&str>)?;
            let pause =
                MenuItem::with_id(app, "pause", "Pause volume gestures", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit EdgeVolume", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &pause, &quit])?;
            let mut pixels = vec![0u8; 32 * 32 * 4];
            for y in 5..27 {
                for x in 22..27 {
                    let i = (y * 32 + x) * 4;
                    pixels[i..i + 4].copy_from_slice(&[90, 190, 150, 255]);
                }
            }
            TrayIconBuilder::new()
                .icon(tauri::image::Image::new_owned(pixels, 32, 32))
                .tooltip("EdgeVolume")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "pause" => {
                        if let Ok(mut e) = ENGINE.get().unwrap().lock() {
                            e.status.settings.enabled = false;
                            e.cancel();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            let overlay = app
                .get_webview_window("overlay")
                .ok_or("Overlay window missing")?;
            overlay.set_ignore_cursor_events(true)?;
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let mut shown = false;
                let mut ticks = 0u32;
                loop {
                    std::thread::sleep(Duration::from_millis(50));
                    ticks = ticks.wrapping_add(1);
                    if ticks % 10 == 0 {
                        #[cfg(target_os = "macos")]
                        {
                            let retry = ENGINE.get().and_then(|e| e.lock().ok()).is_some_and(|e| {
                                !e.suspended && e.retry_at.is_some_and(|t| Instant::now() >= t)
                            });
                            if RECONNECT.swap(false, Ordering::SeqCst) || retry {
                                // Never hold ENGINE while native stop waits for a callback.
                                let code = unsafe { edge_restart() };
                                if let Ok(mut e) = ENGINE.get().unwrap().lock() {
                                    e.cancel();
                                    e.status.contacts = 0;
                                    e.status.input = if code == 0 {
                                        "Listening".into()
                                    } else {
                                        format!("Trackpad unavailable (code {code})")
                                    };
                                    e.retry_at = if code == 0 {
                                        None
                                    } else {
                                        Some(Instant::now() + Duration::from_secs(3))
                                    };
                                }
                            }
                        }
                        if let Ok(mut e) = ENGINE.get().unwrap().lock() {
                            refresh_audio(&mut e);
                        }
                    }
                    let (visible, feedback) = ENGINE
                        .get()
                        .and_then(|e| e.lock().ok())
                        .map(|e| {
                            (
                                e.feedback_until.is_some_and(|t| Instant::now() < t),
                                e.status.feedback.clone(),
                            )
                        })
                        .unwrap_or((false, None));
                    // Native macOS HUD also refreshes its value during a gesture;
                    // it never relies on a hidden/background WebView rendering.
                    if !visible && !shown {
                        continue;
                    }
                    #[cfg(not(target_os = "macos"))]
                    if visible == shown {
                        continue;
                    }
                    shown = visible;
                    let app = handle.clone();
                    let _ = handle.run_on_main_thread(move || {
                        #[cfg(target_os = "macos")]
                        {
                            let _ = app;
                            if let Some(feedback) = feedback {
                                if let Ok(label) = std::ffi::CString::new(feedback.label) {
                                    unsafe {
                                        edge_show_overlay(
                                            feedback.volume,
                                            label.as_ptr(),
                                            i32::from(visible),
                                        );
                                    }
                                }
                            } else {
                                unsafe {
                                    edge_show_overlay(0.0, c"Current volume".as_ptr(), 0);
                                }
                            }
                        }
                        #[cfg(not(target_os = "macos"))]
                        if let Some(w) = app.get_webview_window("overlay") {
                            if visible {
                                if let Ok(Some(m)) = w.current_monitor() {
                                    let scale = m.scale_factor();
                                    let x = m.position().x as f64 / scale
                                        + m.size().width as f64 / scale
                                        - 310.0;
                                    let y = m.position().y as f64 / scale
                                        + m.size().height as f64 / scale
                                        - 150.0;
                                    let _ = w.set_position(tauri::LogicalPosition::new(x, y));
                                }
                                let _ = w.show();
                            } else {
                                let _ = w.hide();
                            }
                        }
                    });
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            save_settings,
            preview_feedback,
            get_feedback,
            test_windows_volume,
            get_running_apps,
            reconnect_trackpad
        ])
        .run(tauri::generate_context!())
        .expect("Could not start EdgeVolume");
}
