# **Product Requirements Document (PRD)**

# **Cross-Platform Trackpad Edge Volume Control Utility**

## **1\. Document Overview**

* **Product Name:** EdgeVolume (Working Title)  
* **Target Platforms:** macOS (Apple Silicon & Intel) and Windows 10/11  
* **Architecture:** Tauri (Rust backend \+ React / TypeScript frontend)  
* **Document Status:** Draft / v1.0

## **2\. Executive Summary & Problem Statement**

Laptops provide multitouch trackpads with generous surface areas. However, adjusting master audio volume typically requires dedicating one hand to reach keyboard function keys, clicking into the system tray/menu bar, or using on-screen sliders.

EdgeVolume is a lightweight, cross-platform background utility that maps vertical scroll gestures along the outer edge(s) of the trackpad/display boundary directly to master system audio adjustment. It delivers frictionless volume changes without stealing focus or interfering with regular navigation.

## **3\. Goals & Non-Goals**

### **3.1 Primary Goals**

* **Seamless Volume Modulation:** Enable real-time, responsive master volume control when scrolling along configured active edge zones.  
* **Minimal Footprint:** Run quietly as a menu bar / system tray utility consuming minimal memory (\< 30 MB idle) and negligible CPU overhead.  
* **Native Safety & Transparency:** Intercept events strictly within the active zone; transparently forward or ignore standard gestures outside the zone so normal gestures/clicks are never blocked or broken.  
* **Cross-Platform Parity:** Provide equivalent performance, responsiveness, and UX on macOS and Windows 10/11.

### **3.2 Non-Goals**

* Custom audio processing, equalizer (EQ), or spatial audio filtering.  
* Per-application audio routing or mixing (Phase 1 focuses exclusively on Master System Volume).  
* Linux support (deferred to post-v1 evaluation).

## **4\. Technical Architecture & System Design**

### **4.1 Framework Selection**

* **Core Runtime:** Tauri v2.  
  * **Why Tauri?** Native OS footprint, minimal bundle size, Rust-backed safety, low latency event loops, and direct C/native FFI bindings.  
* **Backend:** Rust (`src-tauri`).  
  * Direct OS event tap / input hooks.  
  * Audio endpoint APIs.  
* **Frontend:** React \+ TypeScript \+ Tailwind CSS.  
  * Settings window / preferences panel, onboarding tour, permissions checklist, and tray menu popover.

### **4.2 System-Level Integrations & Permissions**

#### **macOS Integration**

* **Input Monitoring:** Uses `CGEventTap` via Quartz Event Services to listen for scrolling and pointer events globally.  
* **Required Permissions:** Accessibility Permissions (`AXIsProcessTrustedWithOptions`) and Input Monitoring.  
* **Volume API:** Native macOS CoreAudio (`AudioObjectGetPropertyData` / `AudioObjectSetPropertyData` on the default output device) or CoreAudio HAL bindings for instant latency-free volume adjustments.

#### **Windows Integration**

* **Input Monitoring:** Uses Low-Level Mouse Hooks (`SetWindowsHookExW` with `WH_MOUSE_LL`) or Windows Precision Touchpad / Raw Input APIs.  
* **Required Permissions:** Standard user permissions (no Administrator elevation required for basic mouse hooks and volume endpoints).  
* **Volume API:** Windows Core Audio APIs (`IAudioEndpointVolume` via WASAPI / COM interface).

## **5\. Detailed Functional Specifications**

### **5.1 Edge Detection Engine**

* **Active Boundary Zones:** User can configure whether the trigger zone is the Right Edge, Left Edge, or Top Edge. Default is **Right Edge**.  
* **Zone Thickness / Tolerance:** Configurable margin in pixels or percentage (default: outer 2% to 4% of cursor boundary or trackpad coordinate bounds).  
* **Activation Heuristics:**  
  * Velocity / acceleration dampening to prevent accidental volume spikes.  
  * Hysteresis / deadzone threshold: Gesture must travel a minimum vertical distance within the edge zone before volume changes trigger.

### **5.2 Volume Adjustment Behavior**

* **Step Size & Sensitivity:** User-selectable step multiplier (1% to 5% increments per scroll tick).  
* **Visual / Overlay Feedback (OSD):** Option to display a sleek, minimalist floating volume bar overlay near the screen edge during adjustment, or rely on native OS indicators.  
* **Mute Shortcut:** Optional double-tap or long-hold on the edge boundary to toggle mute/unmute.

### **5.3 User Interface & Settings App**

* **System Tray / Menu Bar Icon:**  
  * Quick toggle: Enable / Disable utility.  
  * Status indicator: Active, Suspended, Missing Permissions.  
  * Open Preferences / Check for Updates / Quit.  
* **Preferences Panel:**  
  * **Edge Selection:** Left / Right / Both.  
  * **Sensitivity & Inversion:** Invert scroll direction (natural vs. traditional).  
  * **Exclusion List:** Option to automatically pause when fullscreen applications or specific games/DAWs are focused.  
  * **Launch at Login:** Native auto-start toggle on macOS (LaunchAgent) and Windows (Registry Run key).

### **5.4 Permissions & Onboarding Flow**

* First-launch wizard detecting platform and evaluating permission status.  
* On macOS: Clear guided prompts directing the user directly to `System Settings > Privacy & Security > Accessibility`.  
* Live status checkmark verifying that permissions have been granted before activating event hooks.

## **6\. Non-Functional Requirements**

| Metric | Target |
| :---- | :---- |
| **Idle Memory (RAM)** | \< 30 MB |
| **CPU Usage (Idle)** | \< 0.2% |
| **Input Latency** | \< 10 ms from trackpad scroll event to audio API dispatch |
| **Binary Size** | \< 15 MB installer |
| **Compatibility** | macOS 12+ (Monterey, Ventura, Sonoma, Sequoia); Windows 10 (1809+) & Windows 11 |

## **7\. Implementation Roadmap & Milestones**

1. **Milestone 1: Prototype Engine (Weeks 1–2)**  
   * Rust-based input listener testing on macOS (`CGEventTap`) and Windows (`WH_MOUSE_LL`).  
   * Rust audio volume controller integration (`coreaudio` on Mac, `wasapi` on Windows).  
2. **Milestone 2: Tauri Integration & Settings UI (Weeks 3–4)**  
   * Scaffold Tauri frontend with React \+ TypeScript.  
   * Inter-Process Communication (IPC) commands for settings persistence (`tauri-plugin-store`).  
   * System tray menu and auto-start configuration.  
3. **Milestone 3: Edge Tuning & Permission Wizard (Weeks 5–6)**  
   * Accurate edge deadzones, dampening algorithms, and accidental touch rejection.  
   * Polished macOS accessibility permission onboarding flow.  
4. **Milestone 4: Packaging & Release (Weeks 7–8)**  
   * macOS `.dmg` with code-signing / notarization preparation.  
   * Windows `.msi` / lightweight `.exe` installer.  
   * Beta testing and performance profiling.


## 8. Implementation clarification — 7 October 2026

The trigger is the **physical laptop trackpad edge**, independent of screen cursor position. Screen-edge detection and mouse wheel hooks do not satisfy this requirement.

### First usable prototype

- Tauri 2, Rust engine, React/TypeScript preferences.
- Physical left, right or both side edges; top edge deferred.
- Single-finger vertical slide, beginning inside the chosen edge. Default width 8% of normalized trackpad surface, adjustable 3–20% for hardware calibration.
- 2.5% vertical activation deadzone. Additional contacts or leaving the zone cancel until all contacts lift. Reject large coordinate jumps; clamp changes to 2.5 percentage points per frame at normal sensitivity.
- Continuous sensitivity multiplier replaces scroll-tick increments because raw contacts have no scroll ticks.
- Saved settings, tray preferences/pause/quit controls, live input diagnostic and output volume indicator.
- Explicit enable each launch during prototype validation.

### Platform feasibility and release gates

macOS global physical coordinates use the private MultitouchSupport framework; Accessibility and Input Monitoring are not presented as requirements for this observation-only adapter. If a later event suppression adapter requires them, add precise onboarding and live checks then. Private framework compatibility must be tested across supported OS/hardware versions before release.

Windows requires Precision Touchpad HID contact reports, normalized using descriptor logical bounds, plus Core Audio endpoint volume control. Generic non-Precision devices may be unsupported. The first Mac prototype does not provide this Windows adapter.

Observing contacts does not suppress cursor motion or system gestures. The original requirement that edge volume control never interferes with navigation remains a release gate, not a prototype guarantee. Test edge gestures, taps, multitouch, mouse wheels, device reconnects, sleep/wake, multiple trackpads and output changes on real hardware.

Unsupported/nonadjustable audio output devices must show an actionable state and pause volume gestures. Do not silently substitute another output. Permission revocation or adapter failure must never leave input interception active.

Auto-start, exclusion lists, mute shortcuts, floating overlay and updates follow verified input behavior. Performance targets remain goals until measured with preferences open and closed, including WebView processes. Signing/notarization and Windows packaging follow platform validation.

### Volume feedback implementation

The Mac prototype now includes a floating volume indicator showing the actual output volume percentage after each successful adjustment, with Increasing / Decreasing direction and explicit limit states. It is always on top, non-focusable, click-through, and dismisses after 1.4 seconds without further adjustments. A read-only preview shows current volume for five seconds. This replaces the earlier deferral of the floating overlay; autostart, exclusions and mute shortcuts remain deferred.
