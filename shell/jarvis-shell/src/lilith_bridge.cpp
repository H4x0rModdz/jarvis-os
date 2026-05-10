#include "lilith_bridge.h"

#include <QDBusConnection>
#include <QDBusPendingCall>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLoggingCategory>

namespace {
Q_LOGGING_CATEGORY(lcLilith, "jarvis.shell.lilith")

constexpr const char* kService = "com.jarvis.Lilith";
constexpr const char* kPath = "/com/jarvis/Lilith";
constexpr const char* kIface = "com.jarvis.Lilith";
constexpr int kPingIntervalMs = 3000;
constexpr int kCommandTimeoutMs = 120000; // matches the Ollama-aware timeout in Lilith
}

LilithBridge::LilithBridge(QObject* parent) : QObject(parent)
{
    auto bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        qCWarning(lcLilith) << "Session bus not connected";
        emit errorOccurred(tr("DBus session bus unavailable"));
        return;
    }

    m_iface = new QDBusInterface(kService, kPath, kIface, bus, this);
    m_iface->setTimeout(kCommandTimeoutMs);

    // Probe reachability immediately, then on a slow heartbeat.
    QObject::connect(&m_pingTimer, &QTimer::timeout, this, &LilithBridge::ping);
    m_pingTimer.start(kPingIntervalMs);
    ping();
}

void LilithBridge::ping()
{
    if (!m_iface) return;
    // ListFacts is the cheapest method that proves the daemon is alive AND
    // that our methods are registered. No side effects.
    auto pending = m_iface->asyncCall(QStringLiteral("ListFacts"));
    auto* watcher = new QDBusPendingCallWatcher(pending, this);
    QObject::connect(watcher, &QDBusPendingCallWatcher::finished, this,
        [this](QDBusPendingCallWatcher* w) {
            QDBusPendingReply<QString> reply = *w;
            setReachable(!reply.isError());
            w->deleteLater();
        });
}

void LilithBridge::send(const QString& text)
{
    if (!m_iface) {
        emit errorOccurred(tr("Lilith bridge not initialised"));
        return;
    }
    if (m_busy) {
        qCDebug(lcLilith) << "send() ignored while busy";
        return;
    }

    setBusy(true);
    auto pending = m_iface->asyncCall(QStringLiteral("Command"), text);
    auto* watcher = new QDBusPendingCallWatcher(pending, this);
    QObject::connect(watcher, &QDBusPendingCallWatcher::finished, this,
        [this](QDBusPendingCallWatcher* w) {
            QDBusPendingReply<QString> reply = *w;
            setBusy(false);
            if (reply.isError()) {
                emit errorOccurred(reply.error().message());
                w->deleteLater();
                return;
            }

            // Lilith returns a JSON string with shape:
            //   { "reply": "...", "action": "..."|null, "result": {...}|null }
            const QByteArray raw = reply.value().toUtf8();
            const auto doc = QJsonDocument::fromJson(raw);
            if (!doc.isObject()) {
                emit errorOccurred(tr("Lilith returned non-JSON response"));
                w->deleteLater();
                return;
            }
            const auto obj = doc.object();
            const QString replyText = obj.value(QStringLiteral("reply")).toString();
            const QString action = obj.value(QStringLiteral("action")).toString();
            const QJsonValue result = obj.value(QStringLiteral("result"));
            const QString resultJson = result.isObject()
                ? QString::fromUtf8(QJsonDocument(result.toObject()).toJson(QJsonDocument::Compact))
                : QString();

            emit replyReceived(replyText, action, resultJson);
            w->deleteLater();
        });
}

void LilithBridge::setReachable(bool v)
{
    if (m_reachable == v) return;
    m_reachable = v;
    emit reachableChanged();
}

void LilithBridge::setBusy(bool v)
{
    if (m_busy == v) return;
    m_busy = v;
    emit busyChanged();
}
