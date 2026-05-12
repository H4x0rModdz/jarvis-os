#include "notifications_bridge.h"

#include <QDBusConnection>
#include <QDBusPendingCall>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
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

    // V2 signal carries an extra QStringList of actions. The daemon
    // emits the same signal name on the FreeDesktop spec interface
    // (org.freedesktop.Notifications) — that's where we bind.
    const bool ok = bus.connect(
        QStringLiteral("org.freedesktop.Notifications"),
        QStringLiteral("/org/freedesktop/Notifications"),
        QStringLiteral("org.freedesktop.Notifications"),
        QStringLiteral("NotificationPosted"),
        this,
        SLOT(onPosted(uint, QString, QString, QString, QString, QStringList)));
    if (!ok) {
        qCWarning(lcNotif) << "Failed to subscribe to NotificationPosted";
    }

    // The daemon's Jarvis-private interface lives at a separate path
    // and serves RecentNotifications. Keep a long-lived QDBusInterface
    // so refreshHistory + invokeAction don't re-dial each call.
    m_history_iface = new QDBusInterface(
        QStringLiteral("com.jarvis.Notifications"),
        QStringLiteral("/com/jarvis/Notifications"),
        QStringLiteral("com.jarvis.Notifications"),
        bus, this);
}

void NotificationsBridge::onPosted(uint id, const QString& app,
                                   const QString& summary,
                                   const QString& body,
                                   const QString& urgency,
                                   const QStringList& actions)
{
    qCInfo(lcNotif) << "Posted:" << id << app << summary << urgency
                    << "actions=" << actions.size() / 2;
    m_id = id;
    m_app = app;
    m_summary = summary;
    m_body = body;
    m_urgency = urgency;
    m_actions = actions;
    m_tick++;
    emit notificationChanged();
}

void NotificationsBridge::invokeAction(quint32 id, const QString& key)
{
    qCInfo(lcNotif) << "Invoking action" << key << "on" << id;
    // The InvokeAction method is on the FreeDesktop spec interface
    // (Service struct on /org/freedesktop/Notifications) so the
    // ActionInvoked re-emit travels back on the same path.
    auto bus = QDBusConnection::sessionBus();
    QDBusInterface iface(
        QStringLiteral("org.freedesktop.Notifications"),
        QStringLiteral("/org/freedesktop/Notifications"),
        QStringLiteral("org.freedesktop.Notifications"),
        bus);
    iface.asyncCall(QStringLiteral("InvokeAction"), id, key);
}

void NotificationsBridge::refreshHistory()
{
    if (!m_history_iface) return;

    auto pending = m_history_iface->asyncCall(
        QStringLiteral("RecentNotifications"), quint32{20});
    auto* watcher = new QDBusPendingCallWatcher(pending, this);
    QObject::connect(watcher, &QDBusPendingCallWatcher::finished, this,
        [this](QDBusPendingCallWatcher* w) {
            QDBusPendingReply<QString> reply = *w;
            if (reply.isError()) {
                qCWarning(lcNotif) << "RecentNotifications failed:"
                                   << reply.error().message();
                w->deleteLater();
                return;
            }
            const auto doc = QJsonDocument::fromJson(reply.value().toUtf8());
            if (!doc.isArray()) {
                qCWarning(lcNotif) << "RecentNotifications: expected array";
                w->deleteLater();
                return;
            }
            QVariantList out;
            for (const auto& v : doc.array()) {
                if (v.isObject()) out.append(v.toObject().toVariantMap());
            }
            // Newest first — the daemon returns oldest-first but the
            // drawer reads top-down.
            std::reverse(out.begin(), out.end());
            m_history = out;
            emit historyChanged();
            w->deleteLater();
        });
}
