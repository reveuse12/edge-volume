// Windows development adapter. HID diagnostics observe reports without decoding
// contacts, changing device modes, suppressing input, or installing a driver.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <mmdeviceapi.h>
#include <endpointvolume.h>
#include <wrl/client.h>
#include <atomic>
#include <chrono>
#include <future>
#include <memory>
#include <thread>
#include <vector>
#include <cmath>
#include <cstddef>
using Microsoft::WRL::ComPtr;
static std::atomic<int> devices{0}, listener{0};
static std::atomic<unsigned long long> reports{0};
static std::atomic<bool> started{false};

static bool touchpad(HANDLE device) {
    RID_DEVICE_INFO info{}; info.cbSize = sizeof(info);
    UINT bytes = sizeof(info);
    return GetRawInputDeviceInfoW(device, RIDI_DEVICEINFO, &info, &bytes) != UINT(-1)
        && info.dwType == RIM_TYPEHID
        && info.hid.usUsagePage == 0x0d && info.hid.usUsage == 0x05;
}
static void enumerate_devices() {
    UINT count = 0;
    if (GetRawInputDeviceList(nullptr, &count, sizeof(RAWINPUTDEVICELIST)) == UINT(-1)) {
        devices = -1; return;
    }
    std::vector<RAWINPUTDEVICELIST> list(count);
    if (!count) { devices = 0; return; }
    UINT received = GetRawInputDeviceList(list.data(), &count, sizeof(RAWINPUTDEVICELIST));
    if (received == UINT(-1)) { devices = -1; return; }
    int found = 0;
    for (UINT i = 0; i < received; ++i)
        if (list[i].dwType == RIM_TYPEHID && touchpad(list[i].hDevice)) ++found;
    devices = found;
}
static LRESULT CALLBACK input_window(HWND window, UINT msg, WPARAM wp, LPARAM lp) {
    if (msg == WM_INPUT) {
        UINT bytes = 0;
        if (GetRawInputData(reinterpret_cast<HRAWINPUT>(lp), RID_INPUT, nullptr,
                            &bytes, sizeof(RAWINPUTHEADER)) == 0
            && bytes >= sizeof(RAWINPUT) && bytes <= 1024 * 1024) {
            // uint64_t storage provides the alignment required by RAWINPUT.
            std::vector<unsigned long long> buffer((bytes + 7) / 8);
            UINT copied = GetRawInputData(reinterpret_cast<HRAWINPUT>(lp), RID_INPUT,
                                         buffer.data(), &bytes, sizeof(RAWINPUTHEADER));
            if (copied != UINT(-1) && copied >= sizeof(RAWINPUT)) {
                auto raw = reinterpret_cast<const RAWINPUT*>(buffer.data());
                const size_t offset = offsetof(RAWINPUT, data) + offsetof(RAWHID, bRawData);
                if (raw->header.dwType == RIM_TYPEHID && touchpad(raw->header.hDevice)
                    && raw->data.hid.dwSizeHid > 0
                    && copied >= offset
                    && raw->data.hid.dwCount <= (copied - offset) / raw->data.hid.dwSizeHid)
                    reports.fetch_add(raw->data.hid.dwCount);
            }
        }
        // Required cleanup for foreground WM_INPUT; harmless for INPUTSINK.
        return DefWindowProcW(window, msg, wp, lp);
    }
    if (msg == WM_INPUT_DEVICE_CHANGE || msg == WM_TIMER) enumerate_devices();
    return DefWindowProcW(window, msg, wp, lp);
}
extern "C" int edge_windows_start() {
    if (started.exchange(true)) return listener.load();
    auto ready = std::make_shared<std::promise<int>>();
    auto result = ready->get_future();
    try {
        std::thread([ready]() {
            HINSTANCE instance = GetModuleHandleW(nullptr);
            WNDCLASSW cls{}; cls.hInstance = instance;
            cls.lpfnWndProc = input_window; cls.lpszClassName = L"EdgeVolumeHidDiagnostics";
            if (!RegisterClassW(&cls)) {
                listener = -static_cast<int>(GetLastError()); ready->set_value(listener); return;
            }
            HWND window = CreateWindowExW(0, cls.lpszClassName, L"", 0, 0, 0, 0, 0,
                                           HWND_MESSAGE, nullptr, instance, nullptr);
            if (!window) {
                listener = -static_cast<int>(GetLastError()); ready->set_value(listener); return;
            }
            RAWINPUTDEVICE device{0x0d, 0x05, RIDEV_INPUTSINK | RIDEV_DEVNOTIFY, window};
            if (!RegisterRawInputDevices(&device, 1, sizeof(device))) {
                listener = -static_cast<int>(GetLastError()); ready->set_value(listener);
                DestroyWindow(window); return;
            }
            enumerate_devices();
            SetTimer(window, 1, 1000, nullptr);
            listener = 1; ready->set_value(1);
            MSG msg{}; int outcome;
            while ((outcome = GetMessageW(&msg, nullptr, 0, 0)) > 0) {
                TranslateMessage(&msg); DispatchMessageW(&msg);
            }
            listener = outcome == -1 ? -static_cast<int>(GetLastError()) : 0;
            DestroyWindow(window);
        }).detach();
    } catch (...) { listener = -1; return -1; }
    if (result.wait_for(std::chrono::seconds(3)) != std::future_status::ready) return 0;
    return result.get();
}
extern "C" void edge_windows_diagnostics(int *count, unsigned long long *received, int *state) {
    *count = devices.load(); *received = reports.load(); *state = listener.load();
}
extern "C" int edge_windows_volume(float *value, int write) {
    if (!value || !std::isfinite(*value) || (write && (*value < 0 || *value > 1)))
        return static_cast<int>(E_INVALIDARG);
    HRESULT init = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
    if (FAILED(init) && init != RPC_E_CHANGED_MODE) return static_cast<int>(init);
    HRESULT hr;
    {
        ComPtr<IMMDeviceEnumerator> enumerator;
        ComPtr<IMMDevice> device;
        ComPtr<IAudioEndpointVolume> endpoint;
        hr = CoCreateInstance(__uuidof(MMDeviceEnumerator), nullptr, CLSCTX_ALL,
                              IID_PPV_ARGS(&enumerator));
        if (SUCCEEDED(hr)) hr = enumerator->GetDefaultAudioEndpoint(eRender, eMultimedia, &device);
        if (SUCCEEDED(hr)) hr = device->Activate(__uuidof(IAudioEndpointVolume), CLSCTX_ALL,
                                                nullptr, reinterpret_cast<void**>(endpoint.GetAddressOf()));
        if (SUCCEEDED(hr)) hr = write ? endpoint->SetMasterVolumeLevelScalar(*value, nullptr)
                                     : endpoint->GetMasterVolumeLevelScalar(value);
    }
    if (SUCCEEDED(init)) CoUninitialize();
    return static_cast<int>(hr);
}
