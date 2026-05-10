# Architecture: Compositor

## Purpose

The Jarvis compositor is the Wayland compositor responsible for rendering all surfaces, effects, and animations on the desktop.

## Technology Choice

Wayland compositor built on:
- **wlroots** as the compositor library foundation (proven, maintained)
- **Vulkan** for all rendering (future-proof, GPU-native)
- **GLSL/SPIR-V** shaders for blur, shadows, and glass effects

## Rendering Pipeline

```
Wayland surfaces (apps, shell)
        ↓
  Surface compositor (wlroots)
        ↓
  Effect pass (blur, shadows, transparency)
        ↓
  UI overlay pass (taskbar, notifications, overlays)
        ↓
  Output (DRM/KMS → display)
```

## Glass Effect Implementation

Blur algorithm: dual Kawase blur (cheaper than Gaussian, visually equivalent)

```
1. Downsample scene to 1/2 resolution
2. Apply horizontal Kawase pass
3. Apply vertical Kawase pass
4. Upsample to full resolution
5. Blend with surface at target opacity
```

Blur radius is fixed per surface type (not dynamic per frame) to avoid visual instability.

## Performance Targets

- Frame time: < 2ms on integrated GPU, < 1ms on discrete GPU
- Target: consistent 60fps on 1080p, 120fps on high-refresh displays
- Blur must not drop frame rate below 60fps on target hardware

## Adaptive Quality

On low-end hardware or thermal throttling:
1. Disable blur effects first
2. Reduce animation complexity
3. Fall back to solid backgrounds
4. Never drop below 30fps before disabling animations entirely

## Surface Layering (z-order)

```
Layer 5: Lock screen / full-screen overlay
Layer 4: Notifications, tooltips
Layer 3: Floating panels (control center, launcher)
Layer 2: Normal windows
Layer 1: Taskbar, desktop shell
Layer 0: Wallpaper
```

## Compositor ↔ Window Manager IPC

The compositor does not make window management decisions.
It receives instructions from the window manager via a Unix socket:

```
window_manager → compositor: "apply blur to surface <id>"
window_manager → compositor: "animate surface <id> open"
compositor → window_manager: "surface <id> damaged" (repaint trigger)
```

Separation of concerns: compositor renders, window manager decides.
