pub mod app;
pub mod audio;
pub mod bluetooth;
pub mod browser;
pub mod clipboard;
pub mod compat;
pub mod desktop;
pub mod file;
pub mod input;
pub mod network;
pub mod screenshot;
pub mod sdk;
pub mod system;
pub mod updater;
pub mod window;
pub mod workspace;

/// `which`-style probe via PATH lookup. Used by handlers that fall back
/// between Wayland and X11 tools (`grim`/`scrot`, `wl-copy`/`xclip`, …).
/// Cheaper than spawning `which` for every call and avoids depending on
/// any particular `which` implementation being installed.
pub(crate) async fn which_exists(cmd: &str) -> bool {
    let path = match std::env::var_os("PATH") {
        Some(p) => p,
        None => return false,
    };
    std::env::split_paths(&path)
        .map(|dir| dir.join(cmd))
        .any(|p| std::fs::metadata(&p).map(|m| m.is_file()).unwrap_or(false))
}
