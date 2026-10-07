#import <AppKit/AppKit.h>

// Called on Tauri's main thread. Keep the read-only HUD visible independently
// of the preferences window, including another app's fullscreen Space.
void edge_configure_overlay(void *pointer, int show) {
    NSWindow *window = (__bridge NSWindow *)pointer;
    [window setHidesOnDeactivate:NO];
    [window setIgnoresMouseEvents:YES];
    [window setLevel:NSStatusWindowLevel];
    [window setCollectionBehavior:NSWindowCollectionBehaviorCanJoinAllSpaces
        | NSWindowCollectionBehaviorFullScreenAuxiliary
        | NSWindowCollectionBehaviorCanJoinAllApplications
        | NSWindowCollectionBehaviorTransient
        | NSWindowCollectionBehaviorIgnoresCycle];
    if (show) [window orderFrontRegardless];
}
