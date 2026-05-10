#include "action_bus_bridge.h"

#include <QDBusConnection>
#include <QDBusPendingCall>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonParseError>
#include <QLoggingCategory>
#include <QUuid>

namespace {
Q_LOGGING_CATEGORY(lcBus, "jarvis.shell.actionbus")

constexpr const char* kService = "com.jarvis.ActionBus";
constexpr const char* kPath = "/com/jarvis/ActionBus";
constexpr const char* kIface = "com.jarvis.ActionBus";
constexpr int kTimeoutMs = 120000;
}

ActionBusBridge::ActionBusBridge(QObject* parent) : QObject(parent)
{
    auto bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        emit errorOccurred(tr("DBus session bus unavailable"));
        return;
    }
    m_iface = new QDBusInterface(kService, kPath, kIface, bus, this);
    m_iface->setTimeout(kTimeoutMs);
}

void ActionBusBridge::dispatch(const QString& action, const QString& paramsJson)
{
    if (!m_iface) {
        emit errorOccurred(tr("Action Bus bridge not initialised"));
        return;
    }

    // Parse the params JSON so we can validate up front. We then re-serialise
    // alongside the wrapping request envelope.
    QJsonParseError perr{};
    const QJsonDocument paramsDoc =
        paramsJson.isEmpty() ? QJsonDocument(QJsonObject())
                             : QJsonDocument::fromJson(paramsJson.toUtf8(), &perr);
    if (perr.error != QJsonParseError::NoError) {
        emit errorOccurred(tr("Invalid params JSON: %1").arg(perr.errorString()));
        return;
    }

    QJsonObject request{
        { QStringLiteral("action"), action },
        { QStringLiteral("caller"), QJsonObject{ { QStringLiteral("type"), QStringLiteral("app") },
                                                  { QStringLiteral("id"), QStringLiteral("jarvis-shell") } } },
        { QStringLiteral("params"), paramsDoc.isObject() ? paramsDoc.object() : QJsonObject() },
        { QStringLiteral("session_id"), QUuid::createUuid().toString(QUuid::WithoutBraces) },
        { QStringLiteral("idempotency_key"), QJsonValue::Null },
    };
    const QString envelope = QString::fromUtf8(QJsonDocument(request).toJson(QJsonDocument::Compact));

    auto pending = m_iface->asyncCall(QStringLiteral("Dispatch"), envelope);
    auto* watcher = new QDBusPendingCallWatcher(pending, this);
    QObject::connect(watcher, &QDBusPendingCallWatcher::finished, this,
        [this, action](QDBusPendingCallWatcher* w) {
            QDBusPendingReply<QString> reply = *w;
            if (reply.isError()) {
                qCWarning(lcBus) << "Dispatch failed:" << reply.error().message();
                emit errorOccurred(reply.error().message());
                emit dispatchFinished(action, QString(), false);
                w->deleteLater();
                return;
            }

            const QString json = reply.value();
            const auto doc = QJsonDocument::fromJson(json.toUtf8());
            const bool ok = doc.isObject() &&
                doc.object().value(QStringLiteral("status")).toString() == QStringLiteral("success");
            qCInfo(lcBus) << "Dispatch finished" << action << "ok=" << ok;
            emit dispatchFinished(action, json, ok);
            w->deleteLater();
        });
}
