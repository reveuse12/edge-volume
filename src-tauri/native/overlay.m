#import <AppKit/AppKit.h>

// A real nonactivating NSPanel is independent of the preferences WebView and
// its background rendering lifecycle. All calls run on Tauri's main thread.
@interface EdgeVolumePanel : NSPanel
@end
@implementation EdgeVolumePanel
- (BOOL)canBecomeKeyWindow { return NO; }
- (BOOL)canBecomeMainWindow { return NO; }
@end

@interface EdgeVolumeHUD : NSView
@property(nonatomic) float volume;
@property(nonatomic, copy) NSString *label;
@end
@implementation EdgeVolumeHUD
- (void)drawRect:(NSRect)dirty {
    (void)dirty;
    NSColor *mint = [NSColor colorWithSRGBRed:.55 green:.87 blue:.67 alpha:1];
    [[NSColor colorWithSRGBRed:.063 green:.09 blue:.078 alpha:1] setFill];
    NSRectFill(self.bounds);
    [[NSColor colorWithSRGBRed:.20 green:.28 blue:.24 alpha:1] setStroke];
    [NSBezierPath strokeRect:NSInsetRect(self.bounds, .5, .5)];
    NSDictionary *small = @{NSFontAttributeName:[NSFont systemFontOfSize:9],
                            NSForegroundColorAttributeName:mint};
    [@"EDGEVOLUME" drawAtPoint:NSMakePoint(22, 69) withAttributes:small];
    [self.label drawAtPoint:NSMakePoint(22, 43) withAttributes:@{
        NSFontAttributeName:[NSFont systemFontOfSize:13 weight:NSFontWeightMedium],
        NSForegroundColorAttributeName:[NSColor colorWithSRGBRed:.84 green:.91 blue:.86 alpha:1]}];
    NSString *percent = [NSString stringWithFormat:@"%.0f%%", self.volume * 100];
    NSDictionary *large = @{NSFontAttributeName:[NSFont systemFontOfSize:27 weight:NSFontWeightMedium],
                            NSForegroundColorAttributeName:[NSColor whiteColor]};
    NSSize size = [percent sizeWithAttributes:large];
    [percent drawAtPoint:NSMakePoint(258 - size.width, 49) withAttributes:large];
    NSRect bar = NSMakeRect(22, 22, 236, 4);
    [[NSColor colorWithSRGBRed:.16 green:.22 blue:.19 alpha:1] setFill];
    NSRectFill(bar);
    [mint setFill]; bar.size.width *= self.volume; NSRectFill(bar);
}
@end

static EdgeVolumePanel *panel;
static EdgeVolumeHUD *hud;
void edge_show_overlay(float volume, const char *label, int visible) {
    if (!visible) { [panel orderOut:nil]; return; }
    if (!panel) {
        panel = [[EdgeVolumePanel alloc] initWithContentRect:NSMakeRect(0, 0, 280, 100)
            styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel
            backing:NSBackingStoreBuffered defer:NO];
        [panel setReleasedWhenClosed:NO];
        [panel setHidesOnDeactivate:NO];
        [panel setIgnoresMouseEvents:YES];
        [panel setLevel:NSStatusWindowLevel];
        [panel setCollectionBehavior:NSWindowCollectionBehaviorCanJoinAllSpaces
            | NSWindowCollectionBehaviorFullScreenAuxiliary
            | NSWindowCollectionBehaviorCanJoinAllApplications
            | NSWindowCollectionBehaviorTransient
            | NSWindowCollectionBehaviorIgnoresCycle];
        hud = [[EdgeVolumeHUD alloc] initWithFrame:NSMakeRect(0, 0, 280, 100)];
        [panel setContentView:hud];
        [panel setTitle:@"EdgeVolume volume indicator"];
    }
    hud.volume = fminf(1, fmaxf(0, volume));
    hud.label = [NSString stringWithUTF8String:label] ?: @"Current volume";
    NSPoint pointer = [NSEvent mouseLocation];
    NSScreen *screen = [NSScreen mainScreen];
    for (NSScreen *candidate in [NSScreen screens])
        if (NSPointInRect(pointer, candidate.frame)) { screen = candidate; break; }
    NSRect frame = screen.visibleFrame;
    [panel setFrameOrigin:NSMakePoint(NSMaxX(frame) - 310, NSMinY(frame) + 30)];
    [hud setNeedsDisplay:YES];
    [hud displayIfNeeded];
    [panel orderFrontRegardless];
}
