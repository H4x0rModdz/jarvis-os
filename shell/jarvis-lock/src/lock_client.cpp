#include "lock_client.h"

#include <QCoreApplication>
#include <QDBusConnection>
#include <QDBusPendingCall>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLoggingCategory>

namespace {
Q_LOGGING_CATEGORY(lcLock, "jarvis.lock")

constexpr const char* kService = "com.jarvis.Lock";
constexpr const char* kPath = "/com/jarvis/Lock";
constexpr const char* kIface = "com.jarvis.Lock";
}

LockClient::LockClient(QObject* parent) : QObject(parent)
{
    auto bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        qCWarning(lcLock) << "Session bus not connected";
        setState(QStringLiteral("error"), tr("Sem conexão com o daemon de bloqueio"));
        return;
    }

    m_iface = new QDBusInterface(kService, kPath, kIface, bus, this);

    // The window quits as soon as the daemon's LockStateChanged(false)
    // arrives — that means PAM accepted us. Until then we sit here.
    const bool ok = bus.connect(
        kService, kPath, kIface,
        QStringLiteral("LockStateChanged"),
        this,
        SLOT(onLockStateChanged(bool)));
    if (!ok) {
        qCWarning(lcLock) << "Failed to subscribe to LockStateChanged";
    }
}

void LockClient::verify(const QString& password)
{
    if (!m_iface) return;
    setState(QStringLiteral("checking"));

    auto pending = m_iface->asyncCall(QStringLiteral("Verify"), password);
    auto* watcher = new QDBusPendingCallWatcher(pending, this);
    QObject::connect(watcher, &QDBusPendingCallWatcher::finished, this,
        [this](QDBusPendingCallWatcher* w) {
            QDBusPendingReply<QString> reply = *w;
            if (reply.isError()) {
                setState(QStringLiteral("error"), reply.error().message());
                w->deleteLater();
                return;
            }
            const auto doc = QJsonDocument::fromJson(reply.value().toUtf8());
            if (!doc.isObject()) {
                setState(QStringLiteral("error"), tr("Resposta inválida do daemon"));
                w->deleteLater();
                return;
            }
            const auto obj = doc.object();
            if (obj.value(QStringLiteral("ok")).toBool()) {
                // Success — daemon will fire LockStateChanged(false)
                // which lands in onLockStateChanged and quits.
                setState(QStringLiteral("verified"));
            } else {
                const QString reason = obj.value(QStringLiteral("reason"))
                                          .toString(tr("Senha incorreta"));
                setState(QStringLiteral("idle"), reason);
            }
            w->deleteLater();
        });
}

void LockClient::verifyVoice()
{
    if (!m_iface) return;
    setState(QStringLiteral("listening"));

    auto pending = m_iface->asyncCall(QStringLiteral("VerifyVoice"));
    auto* watcher = new QDBusPendingCallWatcher(pending, this);
    QObject::connect(watcher, &QDBusPendingCallWatcher::finished, this,
        [this](QDBusPendingCallWatcher* w) {
            QDBusPendingReply<QString> reply = *w;
            if (reply.isError()) {
                setState(QStringLiteral("idle"), reply.error().message());
                w->deleteLater();
                return;
            }
            const auto doc = QJsonDocument::fromJson(reply.value().toUtf8());
            if (!doc.isObject()) {
                setState(QStringLiteral("idle"), tr("Resposta inválida do daemon"));
                w->deleteLater();
                return;
            }
            const auto obj = doc.object();
            if (obj.value(QStringLiteral("ok")).toBool()) {
                // LockStateChanged(false) will quit us; same path as
                // the password verifier.
                setState(QStringLiteral("verified"));
            } else {
                const QString reason = obj.value(QStringLiteral("reason"))
                                          .toString(tr("Voz não reconhecida"));
                setState(QStringLiteral("idle"), reason);
            }
            w->deleteLater();
        });
}

void LockClient::onLockStateChanged(bool locked)
{
    qCInfo(lcLock) << "LockStateChanged:" << locked;
    if (!locked) {
        QCoreApplication::quit();
    }
}

void LockClient::setState(const QString& state, const QString& error)
{
    const bool changed = state != m_state || error != m_error;
    m_state = state;
    m_error = error;
    if (changed) emit stateChanged();
}
