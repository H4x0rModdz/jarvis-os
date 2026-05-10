/// Winit backend — runs the compositor inside an existing desktop session.
///
/// Used for development and testing. You can run jarvis-compositor inside
/// your current X11 or Wayland session and see it in a window.
///
/// Switch to the udev backend for real hardware boot.
use crate::{render, state::JarvisCompositor};
use smithay::{
    backend::{
        renderer::damage::OutputDamageTracker,
        winit::{self, WinitEvent},
    },
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::{calloop::EventLoop, winit::platform::pump_events::PumpStatus},
    utils::Transform,
};

pub fn run(event_loop: &mut EventLoop<JarvisCompositor>, state: &mut JarvisCompositor) {
    tracing::info!("Starting winit backend (development mode)");

    let (mut backend, mut winit_loop) =
        match winit::init::<smithay::backend::renderer::gles::GlesRenderer>() {
            Ok(b) => b,
            Err(e) => {
                tracing::error!("Failed to initialize winit backend: {e}");
                return;
            }
        };

    let output = Output::new(
        "winit-output".into(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "Jarvis".into(),
            model: "Winit Dev Backend".into(),
        },
    );

    let mode = Mode {
        size: backend.window_size(),
        refresh: 60_000,
    };

    output.change_current_state(
        Some(mode),
        Some(Transform::Normal),
        None,
        Some((0, 0).into()),
    );
    output.set_preferred(mode);
    state.space.map_output(&output, (0, 0));

    let _global = output.create_global::<JarvisCompositor>(&state.display_handle);

    let mut damage_tracker = OutputDamageTracker::from_output(&output);

    tracing::info!(size = ?mode.size, "Winit output ready — compositor window open");

    loop {
        let result = winit_loop.dispatch_new_events(|event| match event {
            WinitEvent::Resized { size, .. } => {
                let new_mode = Mode {
                    size,
                    refresh: 60_000,
                };
                output.change_current_state(Some(new_mode), None, None, None);
                damage_tracker = OutputDamageTracker::from_output(&output);
                tracing::debug!(size = ?size, "Output resized");
            }
            WinitEvent::Input(input_event) => {
                let _ = input_event;
            }
            WinitEvent::CloseRequested => {
                tracing::info!("Winit window closed — stopping compositor");
                state.loop_signal.stop();
            }
            WinitEvent::Redraw => {
                let renderer = backend.renderer();
                if let Err(e) =
                    render::render_output(renderer, &output, &state.space, &mut damage_tracker)
                {
                    tracing::error!("Render error: {e}");
                }
                // TODO Phase 2: bind framebuffer, submit, send frame callbacks
            }
            WinitEvent::Focus(_) => {}
        });

        if matches!(result, PumpStatus::Exit(_)) {
            break;
        }

        event_loop
            .dispatch(Some(std::time::Duration::from_millis(1)), state)
            .unwrap();

        state
            .display_handle
            .flush_clients()
            .unwrap_or_else(|e| tracing::warn!("Flush error: {e}"));
    }
}
