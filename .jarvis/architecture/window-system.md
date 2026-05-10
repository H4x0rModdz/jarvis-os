# Architecture: Window System

## Purpose

Manages window lifecycle, positioning, focus, animations, and workspace layout on the Jarvis desktop.

## Components

```
window-system/
  window_manager.rs       ← lifecycle, state, focus
  workspace_manager.rs    ← virtual desktops, grouping
  window_animator.rs      ← open/close/minimize transitions
  window_rules.rs         ← per-app placement rules
  tiling_engine.rs        ← optional tiling layout support
```

## Window State Machine

```
CREATED → VISIBLE → FOCUSED
                  → UNFOCUSED
         → MINIMIZED
         → MAXIMIZED → FULLSCREEN
         → CLOSING → DESTROYED
```

Transitions fire events that the compositor and animator observe.

## Window Metadata

Each window carries:

```rust
struct Window {
    id: WindowId,
    app_id: AppId,
    title: String,
    geometry: Rect,
    state: WindowState,
    workspace: WorkspaceId,
    decorations: DecorationMode,
    blur_enabled: bool,
    z_order: i32,
}
```

## Animations

All window transitions are handled by `window_animator`:

- Open: scale from 0.95 + fade in, 200ms ease-out
- Close: scale to 0.95 + fade out, 180ms ease-in
- Minimize: slide to taskbar position, 220ms ease-in-out
- Maximize: expand to full geometry, 200ms ease-out

Animations run on the compositor thread, not the window manager thread.

## AI Integration

The window system exposes these actions to the Action Bus:

```
window.focus, window.minimize, window.maximize,
window.move, window.resize, window.close,
window.snap_left, window.snap_right,
workspace.switch, workspace.move_window
```

Lilith may call any of these with `window.control` permission.

## Wayland Surface Handling

- XDG Toplevel for normal application windows
- XDG Popup for menus and tooltips
- Layer Shell for taskbar, overlays, and lock screen
- XWayland bridge for legacy X11 apps (compatibility only)
