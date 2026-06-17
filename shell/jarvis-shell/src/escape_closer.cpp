#include "escape_closer.h"

#include <QEvent>
#include <QGuiApplication>
#include <QKeyEvent>
#include <QLoggingCategory>
#include <QSet>
#include <QString>
#include <QWindow>

namespace {
Q_LOGGING_CATEGORY(lcEsc, "jarvis.shell.esc")

// Windows Escape must NOT dismiss:
//   - the three always-on layer-shell roots (bar / dock / desktop)
//   - the security approval dialog (Esc there would silently drop a pending
//     permission request instead of forcing an explicit allow/deny)
bool isProtected(const QWindow* w)
{
    static const QSet<QString> keep = {
        QStringLiteral("jarvis-topbar"),
        QStringLiteral("jarvis-dock"),
        QStringLiteral("jarvis-desktop"),
        QStringLiteral("jarvis-approval"),
    };
    return keep.contains(w->objectName());
}
}  // namespace

EscapeCloser::EscapeCloser(QObject* parent) : QObject(parent) {}

bool EscapeCloser::eventFilter(QObject* watched, QEvent* event)
{
    if (event->type() != QEvent::KeyPress) {
        return QObject::eventFilter(watched, event);
    }
    const auto* key = static_cast<QKeyEvent*>(event);
    if (key->key() != Qt::Key_Escape) {
        return QObject::eventFilter(watched, event);
    }

    // Hide every visible, non-protected top-level shell window. Panel
    // exclusivity means there's normally just one; closing any strays too is
    // harmless. The debug line confirms Qt actually receives Escape (if this
    // never logs, the key is being eaten below Qt — e.g. by an IME).
    int closed = 0;
    const auto windows = QGuiApplication::topLevelWindows();
    for (QWindow* w : windows) {
        if (w->isVisible() && !isProtected(w)) {
            w->setVisible(false);
            ++closed;
        }
    }
    qCDebug(lcEsc) << "Escape pressed; closed" << closed << "panel(s)";

    if (closed > 0) {
        event->accept();
        return true;  // consumed — don't also forward to the focused item
    }
    return QObject::eventFilter(watched, event);
}
