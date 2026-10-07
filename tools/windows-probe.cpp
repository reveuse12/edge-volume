// Read-only Windows hardware probe. It never changes volume or device modes.
#include <windows.h>
#include <cstdio>
#include <cstring>
extern "C" int edge_windows_start();
extern "C" int edge_windows_volume(float*, int);
extern "C" void edge_windows_diagnostics(int*, unsigned long long*, int*);
int main(int argc, char **argv) {
    static_assert(sizeof(unsigned long long) == 8, "Rust FFI report counter must be 64-bit");
    if (argc == 2 && std::strcmp(argv[1], "--self-test") == 0) {
        float invalid = 1.5f;
        if (edge_windows_volume(&invalid, 1) != static_cast<int>(E_INVALIDARG)) return 1;
        if (edge_windows_volume(nullptr, 0) != static_cast<int>(E_INVALIDARG)) return 1;
        if (edge_windows_start() != 1) return 1;
        int devices = 0, state = 0; unsigned long long reports = 0;
        edge_windows_diagnostics(&devices, &reports, &state);
        if (state != 1 || devices < 0) return 1;
        std::puts("Adapter self-check passed; hardware contact decoding is not tested.");
        return 0;
    }
    std::puts("EdgeVolume Windows read-only probe: touch the trackpad for 10 seconds.");
    int state = edge_windows_start();
    std::printf("Listener startup: %d (1 = registered)\n", state);
    for (int second = 0; second < 10; ++second) {
        int devices = 0; unsigned long long reports = 0;
        edge_windows_diagnostics(&devices, &reports, &state);
        float volume = 0; int audio = edge_windows_volume(&volume, 0);
        std::printf("collections=%d reports=%llu listener=%d ", devices, reports, state);
        if (audio >= 0) std::printf("volume=%.0f%%\n", volume * 100);
        else std::printf("audio HRESULT=0x%08X\n", static_cast<unsigned>(audio));
        Sleep(1000);
    }
    std::puts("Report counts do not prove usable finger coordinates or edge gestures.");
    return state == 1 ? 0 : 1;
}
