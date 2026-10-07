Control volume by sliding one finger along your MacBook trackpad's physical edge. No Rust, Node.js, or developer tools are needed to use these downloads.

### Install

1. In **Apple menu → About This Mac**, check your Chip or Processor.
2. Download the **apple-silicon.dmg** for an M-series Mac, or **intel.dmg** for an Intel Mac.
3. Open the disk image and drag **EdgeVolume** onto **Applications**.
4. Open EdgeVolume from Applications and click **Enable gestures**.
5. Slide one finger up or down along the right edge of your trackpad. The popup shows the volume percentage.

Requires macOS 15 or newer. A ZIP containing the same application is also provided for each architecture.

### Early-access limitations

These builds are **ad-hoc signed, not Developer ID signed or notarized**. macOS may block the first launch pending your approval. Review [Apple's instructions for opening an app from an unidentified developer](https://support.apple.com/en-us/102445) if you choose to run this release. Do not disable Gatekeeper or other system protections.

Apple Silicon hardware input was tested locally on macOS 27.0.1. Intel compilation is checked in CI; Intel trackpad behavior and other macOS versions still need hands-on validation. The input adapter uses Apple's private multitouch framework and can stop working after OS updates.

Gestures start paused each launch. Closing preferences keeps the app in the menu bar; choose Quit there to exit. The app observes trackpad input and does not suppress cursor movement or other system gestures. Windows support is not included.

The SHA256SUMS files provide checksums for the corresponding downloads. Source code and setup details are in the repository README.
