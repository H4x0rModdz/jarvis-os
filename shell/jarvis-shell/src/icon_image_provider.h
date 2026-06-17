#pragma once

#include <QQuickImageProvider>

/// Resolves `image://theme/<name>` URLs to real freedesktop icons.
///
/// The dock, launcher grid and desktop icons all reference apps by their
/// `.desktop` `Icon=` value (e.g. "firefox", "org.kde.dolphin"). QML's
/// `Image` has no built-in way to turn that name into a pixmap — it needs a
/// provider. Without this every tile fell through to the monogram fallback
/// (the dreaded single-letter icons). We hand the name straight to
/// `QIcon::fromTheme`, which walks the active icon theme + its inherited
/// fallbacks (set once in main.cpp).
///
/// Returns a null QPixmap when the theme can't resolve the name; the QML
/// side reads that as `Image.Error` and shows its monogram, so a missing
/// icon degrades gracefully instead of leaving a blank tile.
class IconImageProvider : public QQuickImageProvider
{
public:
    IconImageProvider();

    QPixmap requestPixmap(const QString& id, QSize* size, const QSize& requestedSize) override;
};
