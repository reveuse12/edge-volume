#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod gesture;
use gesture::{Gesture, Settings};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Mutex, OnceLock},
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
    fn edge_configure_overlay(window: *mut std::ffi::c_void, show: i32);
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
}
static ENGINE: OnceLock<Mutex<Engine>> = OnceLock::new();
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
extern "C" fn contact(count: i32, id: i32, x: f32, y: f32) {
    if let Some(e) = ENGINE.get() {
        if let Ok(mut e) = e.lock() {
            e.status.x = x;
            e.status.y = y;
            e.status.contacts = count;
            e.status.frames += 1;
            let s = e.status.settings.clone();
            if let Some(delta) = e.gesture.feed(&s, count, id, x, y) {
                if delta.abs() > 0.0001 {
                    match volume(None).and_then(|before| {
                        volume(Some((before + delta).clamp(0.0, 1.0)))?;
                        let after = volume(None)?;
                        Ok((before, after))
                    }) {
                        Ok((before, after)) => {
                            e.status.volume = Some(after);
                            e.status.error = None;
                            let change = after - before;
                            let label = if change > 0.00001 {
                                "Increasing"
                            } else if change < -0.00001 {
                                "Decreasing"
                            } else if after >= 0.999 {
                                "Maximum volume"
                            } else if after <= 0.001 {
                                "Muted"
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
                        Err(err) => {
                            e.status.error = Some(err);
                            e.status.settings.enabled = false;
                            e.gesture.reset();
                        }
                    }
                }
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
    e.status.volume = volume(None).ok();
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
    e.gesture.reset();
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
                        volume: volume(None).ok(),
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
                }))
                .ok();
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
            ENGINE.get().unwrap().lock().unwrap().status.input = input;
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
                            e.gesture.reset();
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
            #[cfg(target_os = "macos")]
            unsafe {
                edge_configure_overlay(overlay.ns_window()?, 0);
            }
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let mut shown = false;
                loop {
                    std::thread::sleep(Duration::from_millis(50));
                    let visible = ENGINE
                        .get()
                        .and_then(|e| e.lock().ok())
                        .and_then(|e| e.feedback_until)
                        .is_some_and(|t| Instant::now() < t);
                    if visible == shown {
                        continue;
                    }
                    shown = visible;
                    let app = handle.clone();
                    let _ = handle.run_on_main_thread(move || {
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
                                #[cfg(target_os = "macos")]
                                if let Ok(window) = w.ns_window() {
                                    unsafe {
                                        edge_configure_overlay(window, 1);
                                    }
                                }
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
            test_windows_volume
        ])
        .run(tauri::generate_context!())
        .expect("Could not start EdgeVolume");
}
