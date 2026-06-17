#pragma once

#include <QObject>

/// Application-wide "Escape closes the active panel" handler.
///
/// The shell's panels (notifications, preferences, Wi-Fi, About, ...) are
/// separate frameless Windows. Under labwc most of them never take keyboard
/// focus on their own (no text field grabs it), so a per-window
/// `Shortcut { sequence: "Escape" }` / `Keys.onEscapePressed` never fires —
/// the key is delivered to whichever shell surface *does* hold focus (usually
/// the top bar), not to the panel. Result: Esc did nothing on every panel
/// except the launcher (whose search field grabs focus).
///
/// Installed on QGuiApplication, this filter sees the Escape QKeyEvent
/// delivered to ANY shell window before QML focus scoping swallows it, and
/// hides the visible panel(s). One central rule instead of per-window plumbing
/// that the compositor's focus model defeats.
///
/// (Requires the IME to actually deliver Escape — see QT_IM_MODULE=none in
/// iso/assets/labwc/environment; with the Fedora default IM the key was eaten
/// even before reaching Qt.)
class EscapeCloser : public QObject
{
    Q_OBJECT
public:
    explicit EscapeCloser(QObject* parent = nullptr);

protected:
    bool eventFilter(QObject* watched, QEvent* event) override;
};
