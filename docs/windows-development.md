# Windows development build

This is an experimental hardware diagnostic and audio test build, not a finished trackpad volume utility. Physical edge gestures are disabled until contact decoding is implemented and validated on Windows hardware.

## Try the development installer

1. Open the repository's **Actions → Windows development build** and choose a successful run.
2. Download **EdgeVolume-windows-development-x64** under Artifacts. GitHub requires sign-in to download artifacts.
3. Extract the ZIP and run the setup executable on a Windows 10/11 x64 laptop. This installer is unsigned; Windows may flag it as an unknown publisher. Only use a build from this repository that you trust.
4. Open EdgeVolume. The output percentage should match Windows volume. **Preview volume indicator** reads volume without changing it. **Volume −5% / +5%** deliberately change system volume by five percentage points, bounded to 0–100%. The separate Windows mute state is preserved.
5. Move a finger on the trackpad and check **Touchpad collections** and **HID reports received**. Switch to Chrome, move the finger again, then return and compare the report count.
6. Switch audio outputs and repeat the explicit audio check. Closing preferences leaves the app in the system tray; use its Quit command to exit.

No Rust, Node.js, or C++ tools are needed for these packaged downloads. Windows x64 builds statically link the MSVC runtime, and CI checks the app and probe for external MSVC runtime DLL dependencies. Tauri requires WebView2; the installer uses Tauri's default WebView2 bootstrapper when needed, which may require an internet connection. A Windows CI runner can build the installer but cannot validate your physical trackpad.

## Read-only hardware probe

Open PowerShell in the extracted artifact folder and run:

```powershell
.\edgevolume-windows-probe.exe
```

For ten seconds it reports the listener state, exposed touchpad collection count, cumulative HID report count, and current volume or audio error. It never writes volume, records report contents, requests administrator privileges, changes HID feature reports, suppresses system gestures, or installs a driver.

- **listener=1:** Raw Input registration succeeded. This alone does not prove hardware input is available.
- **collections=0:** Windows did not expose a Digitizer/Touch Pad top-level collection through Raw Input. Legacy mouse-style touchpads and some drivers may not expose this path.
- **collections=-1:** Device enumeration failed.
- **reports=0:** No matching HID reports arrived. Try moving your finger; zero can also mean the driver does not expose reports to this observer.
- **reports increasing:** Background input delivery is available for this collection. Finger coordinates, contact states, frame assembly and physical edge gestures remain unverified.

For hardware testing, capture the probe's text and record laptop model, Windows build, trackpad driver/version, whether counts increased with the app hidden, and audio-output results. No device identifiers or raw touch data are exported.

## Implementation and next step

`src-tauri/native/windows.cpp` uses `IMMDeviceEnumerator` and `IAudioEndpointVolume` for the current default multimedia output. Each call resolves the output again, so the next call follows output changes. COM is initialized and released on the calling thread; audio interfaces are not shared across threads.

A dedicated message-only window registers Digitizer/Touch Pad usage page `0x0D`, usage `0x05`, using `RIDEV_INPUTSINK | RIDEV_DEVNOTIFY`. It counts matching reports and refreshes device enumeration after device notifications and once per second. It does not assume mouse pointer coordinates are physical trackpad coordinates.

Next, use real hardware results to select a viable input path. If reports are accessible, implement descriptor-based HID contact decoding, hybrid/multi-report frame assembly, contact identity and lift handling, normalized physical coordinates, and multi-device isolation. Feed complete verified frames into the shared gesture engine. If this path exposes no usable input, reassess the input approach before claiming support; do not change device modes or install a driver automatically.

## Build from source

On Windows, install Node.js, Rust (MSVC toolchain), Microsoft C++ Build Tools with Desktop development with C++, and WebView2, following [Tauri's prerequisites](https://v2.tauri.app/start/prerequisites/). Then:

```powershell
npm ci
cargo test --locked --target x86_64-pc-windows-msvc --manifest-path src-tauri/Cargo.toml
npm run tauri dev -- --target x86_64-pc-windows-msvc
npm run tauri build -- --target x86_64-pc-windows-msvc --bundles nsis
```

Pass the explicit Windows target so static runtime flags apply to the app, rather than host build scripts and procedural macros. The normal Mac release workflow remains separate. Windows development artifacts are not published as a supported GitHub release.

References: [Microsoft Raw Input](https://learn.microsoft.com/en-us/windows/win32/inputdev/about-raw-input), [RegisterRawInputDevices](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerrawinputdevices), [Windows Precision Touchpad collection](https://learn.microsoft.com/en-us/windows-hardware/design/component-guidelines/touchpad-windows-precision-touchpad-collection), [IAudioEndpointVolume](https://learn.microsoft.com/en-us/windows/win32/api/endpointvolume/nn-endpointvolume-iaudioendpointvolume), [Tauri installers](https://v2.tauri.app/distribute/windows-installer/).
