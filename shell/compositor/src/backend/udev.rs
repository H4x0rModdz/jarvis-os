/// udev/DRM backend — runs on real hardware via DRM/KMS.
///
/// This is the production backend used when Jarvis OS boots.
/// It uses libseat for rootless GPU access (no root required).
///
/// Phase 1 status: SKELETON — not yet functional.
/// Implement after the winit backend is stable and tested.
use crate::state::JarvisCompositor;
use smithay::reexports::calloop::EventLoop;

pub fn run(_event_loop: &mut EventLoop<JarvisCompositor>, _state: &mut JarvisCompositor) {
    // TODO Phase 2:
    // 1. Initialize libseat session
    // 2. Discover GPU via udev
    // 3. Open DRM device
    // 4. Initialize GBM + EGL
    // 5. Set up KMS (connector, CRTC, mode)
    // 6. Create GlesRenderer on DRM
    // 7. Set up output with real physical properties
    // 8. Run render loop similar to winit.rs but with DRM page flipping
    //
    // Reference: smithay/examples/anvil/src/udev.rs

    tracing::error!(
        "udev/DRM backend is not yet implemented. \
         Run with --backend winit for development."
    );
}
