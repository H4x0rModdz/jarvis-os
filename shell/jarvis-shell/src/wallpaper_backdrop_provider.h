#pragma once

#include <QQuickImageProvider>

/// Serves `image://backdrop/blur` — a heavily blurred copy of the current
/// wallpaper. Glass surfaces (GlassPanel) draw it behind their tint, cropped
/// to where they sit on screen, so the dock and the panels look like frosted
/// glass over the desktop.
///
/// Why the shell blurs the wallpaper itself: real backdrop blur is the
/// compositor's job, and labwc has no blur. This gets the look for the common
/// case (a panel over the desktop) on any compositor, and costs nothing at
/// runtime — the blur is computed once, at a tiny resolution, and the GPU
/// only stretches a small texture. It does not blur *windows* behind a panel;
/// when the compositor can do that, GlassPanel drops this image.
///
/// The wallpaper is looked up where the session's autostart looks:
/// $JARVIS_WALLPAPER (dev), the user's <data>/jarvis/wallpaper, then the
/// image's default.
class WallpaperBackdropProvider : public QQuickImageProvider
{
public:
    WallpaperBackdropProvider();

    QImage requestImage(const QString& id, QSize* size, const QSize& requestedSize) override;
};
