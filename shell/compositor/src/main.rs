mod action_bus;
mod backend;
mod handlers;
mod render;
mod state;

use action_bus::WindowCommand;
use calloop::{channel, EventLoop};
use smithay::{
    input::SeatState,
    reexports::wayland_server::Display,
    utils::{Clock, Monotonic},
    wayland::{
        compositor::CompositorState,
        output::OutputManagerState,
        shell::{wlr_layer::WlrLayerShellState, xdg::XdgShellState},
        shm::ShmState,
        socket::ListeningSocketSource,
    },
};
use state::{ClientState, JarvisCompositor};
use std::collections::HashMap;

#[derive(Debug, clap::Parser)]
#[command(name = "jarvis-compositor", about = "LilithOS Wayland compositor")]
struct Args {
    /// Backend to use
    #[arg(long, default_value = "winit")]
    backend: Backend,

    /// Wayland socket name (default: auto-detect WAYLAND_DISPLAY or wayland-0)
    #[arg(long)]
    socket: Option<String>,
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum Backend {
    /// Winit window (development — runs inside existing session)
    Winit,
    /// DRM/KMS via udev (production hardware)
    Udev,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("jarvis_compositor=debug".parse()?)
                .add_directive("smithay=warn".parse()?),
        )
        .init();

    tracing::info!("Jarvis Compositor starting");

    // ── Event loop ──────────────────────────────────────────────────────
    let mut event_loop: EventLoop<JarvisCompositor> = EventLoop::try_new()?;
    let loop_handle = event_loop.handle();

    // ── Wayland display ─────────────────────────────────────────────────
    let display: Display<JarvisCompositor> = Display::new()?;
    let display_handle = display.handle();

    // ── Protocol state ──────────────────────────────────────────────────
    let compositor_state = CompositorState::new::<JarvisCompositor>(&display_handle);
    let xdg_shell_state = XdgShellState::new::<JarvisCompositor>(&display_handle);
    let layer_shell_state = WlrLayerShellState::new::<JarvisCompositor>(&display_handle);
    let shm_state = ShmState::new::<JarvisCompositor>(&display_handle, vec![]);
    let output_manager_state =
        OutputManagerState::new_with_xdg_output::<JarvisCompositor>(&display_handle);

    // ── Input / seat ────────────────────────────────────────────────────
    let mut seat_state = SeatState::new();
    let mut seat = seat_state.new_wl_seat(&display_handle, "seat-0");

    seat.add_keyboard(Default::default(), 200, 25)?;
    seat.add_pointer();

    // ── Action Bus channel ───────────────────────────────────────────────
    let (cmd_tx, cmd_rx) = channel::channel::<WindowCommand>();

    loop_handle
        .insert_source(cmd_rx, |event, _, state| {
            if let channel::Event::Msg(cmd) = event {
                state.handle_window_command(cmd);
            }
        })
        .map_err(|e| anyhow::anyhow!("Failed to insert Action Bus channel: {e}"))?;

    action_bus::spawn_action_bus_listener(cmd_tx.clone());

    // ── Compositor state ─────────────────────────────────────────────────
    let mut state = JarvisCompositor {
        display_handle: display_handle.clone(),
        loop_signal: event_loop.get_signal(),
        clock: Clock::<Monotonic>::new(),
        compositor_state,
        xdg_shell_state,
        layer_shell_state,
        shm_state,
        output_manager_state,
        seat_state,
        seat,
        pointer_location: (0.0, 0.0).into(),
        cursor_status: smithay::input::pointer::CursorImageStatus::default_named(),
        space: smithay::desktop::Space::default(),
        windows: HashMap::new(),
        window_ids: HashMap::new(),
        next_window_id: 1,
        cmd_tx: Some(cmd_tx),
    };

    // ── Wayland socket ──────────────────────────────────────────────────
    let socket_source = ListeningSocketSource::new_auto()?;
    let socket_name = socket_source.socket_name().to_os_string();

    loop_handle
        .insert_source(socket_source, |client_stream, _, state| {
            state
                .display_handle
                .insert_client(client_stream, std::sync::Arc::new(ClientState::default()))
                .unwrap();
        })
        .map_err(|e| anyhow::anyhow!("Failed to insert Wayland socket: {e}"))?;

    // Display dispatch: Phase 2 will register `display` with calloop via
    // smithay's WaylandSource (the raw `Generic<Display, _>` doesn't expose
    // DerefMut through NoIoDrop, so dispatch_clients can't be called from
    // inside the closure). For now we keep the Display alive in scope to
    // anchor the wayland-server backend; clients won't be dispatched until
    // the backend is fleshed out for real Linux testing.
    let _display_keepalive = display;

    tracing::info!(
        socket = ?socket_name,
        "Wayland socket ready — set WAYLAND_DISPLAY={:?}",
        socket_name
    );

    // ── Backend ─────────────────────────────────────────────────────────
    let use_winit = !std::env::args().any(|a| a == "--backend=udev" || a == "--backend udev");

    if use_winit {
        backend::winit::run(&mut event_loop, &mut state);
    } else {
        backend::udev::run(&mut event_loop, &mut state);
    }

    Ok(())
}

impl JarvisCompositor {
    fn handle_window_command(&mut self, cmd: WindowCommand) {
        match cmd {
            WindowCommand::Focus { window_id } => self.focus_window(window_id),
            WindowCommand::Minimize { window_id } => self.minimize_window(window_id),
            WindowCommand::Close { window_id, force } => self.close_window(window_id, force),
            WindowCommand::Maximize { window_id } => {
                tracing::info!(window_id, "Maximize — TODO");
            }
            WindowCommand::Move { window_id, x, y } => {
                if let Some(window) = self.windows.get(&window_id).cloned() {
                    self.space.map_element(window, (x, y), false);
                }
            }
            WindowCommand::Resize {
                window_id,
                width,
                height,
            } => {
                if let Some(window) = self.windows.get(&window_id) {
                    if let Some(t) = window.toplevel() {
                        t.with_pending_state(|s| {
                            s.size = Some((width as i32, height as i32).into());
                        });
                        t.send_pending_configure();
                    }
                }
            }
            WindowCommand::SnapLeft { window_id } => {
                if let Some(window) = self.windows.get(&window_id).cloned() {
                    let output_size = self
                        .space
                        .outputs()
                        .next()
                        .and_then(|o| self.space.output_geometry(o))
                        .map(|g| g.size)
                        .unwrap_or((1920, 1080).into());

                    self.space.map_element(window.clone(), (0, 0), false);
                    if let Some(t) = window.toplevel() {
                        t.with_pending_state(|s| {
                            s.size = Some((output_size.w / 2, output_size.h).into());
                        });
                        t.send_pending_configure();
                    }
                }
            }
            WindowCommand::SnapRight { window_id } => {
                if let Some(window) = self.windows.get(&window_id).cloned() {
                    let output_size = self
                        .space
                        .outputs()
                        .next()
                        .and_then(|o| self.space.output_geometry(o))
                        .map(|g| g.size)
                        .unwrap_or((1920, 1080).into());

                    let x = output_size.w / 2;
                    self.space.map_element(window.clone(), (x, 0), false);
                    if let Some(t) = window.toplevel() {
                        t.with_pending_state(|s| {
                            s.size = Some((output_size.w / 2, output_size.h).into());
                        });
                        t.send_pending_configure();
                    }
                }
            }
        }
    }
}
