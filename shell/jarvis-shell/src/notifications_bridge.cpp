#include "notifications_bridge.h"

#include <QDBusConnection>
#include <QLoggingCategory>

namespace {
Q_LOGGING_CATEGORY(lcNotif, "jarvis.shell.notifications")
}

NotificationsBridge::NotificationsBridge(QObject* parent) : QObject(parent)
{
    auto bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        qCWarning(lcNotif) << "Session bus not connected";
        return;
    }

    // The daemon emits NotificationPosted from the same object that
    // serves the FreeDesktop spec — same path + interface name as the
    // spec itself. That keeps Notify() and its re-emit on one DBus
    // surface and means the shell doesn't need an extra subscription
    // for our private com.jarvis.Notifications interface in V1.
    const bool ok = bus.connect(
        QStringLiteral("org.freedesktop.Notifications"),
        QStringLiteral("/org/freedesktop/Notifications"),
        QStringLiteral("org.freedesktop.Notifications"),
        QStringLiteral("NotificationPosted"),
        this,
        SLOT(onPosted(uint, QString, QString, QString, QString)));
    if (!ok) {
        qCWarning(lcNotif) << "Failed to subscribe to NotificationPosted";
    }
}

void NotificationsBridge::onPosted(uint id, const QString& app, const QString& summary,
                                   const QString& body, const QString& urgency)
{
    qCInfo(lcNotif) << "Posted:" << id << app << summary << urgency;
    m_id = id;
    m_app = app;
    m_summary = summary;
    m_body = body;
    m_urgency = urgency;
    m_tick++;
    emit notificationChanged();
}
