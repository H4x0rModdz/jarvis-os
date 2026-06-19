#include "voice_bridge.h"

#include <QDBusConnection>
#include <QDBusPendingCall>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLoggingCategory>

namespace {
Q_LOGGING_CATEGORY(lcVoice, "jarvis.shell.voice")

constexpr const char* kService = "com.jarvis.Voice";
constexpr const char* kPath = "/com/jarvis/Voice";
constexpr const char* kIface = "com.jarvis.Voice";
}

VoiceBridge::VoiceBridge(QObject* parent) : QObject(parent)
{
    // $USER is the identity that com.jarvis.Voice and pam-jarvis
    // both key voiceprints by. Falls back to "jarvis" so headless
    // dev environments don't crash on a missing env var.
    const QByteArray envUser = qgetenv("USER");
    m_currentUser = envUser.isEmpty() ? QStringLiteral("jarvis") : QString::fromLocal8Bit(envUser);

    auto bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        qCWarning(lcVoice) << "Session bus not connected";
        return;
    }

    m_iface = new QDBusInterface(kService, kPath, kIface, bus, this);

    const bool stateOk = bus.connect(
        kService, kPath, kIface,
        QStringLiteral("StateChanged"),
        this,
        SLOT(onStateChanged(QString)));
    const bool finalOk = bus.connect(
        kService, kPath, kIface,
        QStringLiteral("TranscriptionFinal"),
        this,
        SLOT(onTranscriptionFinal(QString)));
    const bool failedOk = bus.connect(
        kService, kPath, kIface,
        QStringLiteral("TranscriptionFailed"),
        this,
        SLOT(onTranscriptionFailed(QString)));
    const bool hotwordOk = bus.connect(
        kService, kPath, kIface,
        QStringLiteral("HotwordDetected"),
        this,
        SLOT(onHotwordDetected(QString)));
    const bool modelOk = bus.connect(
        kService, kPath, kIface,
        QStringLiteral("ModelReady"),
        this,
        SLOT(onModelReady(QString, bool, QString)));
    const bool modelProgOk = bus.connect(
        kService, kPath, kIface,
        QStringLiteral("ModelProgress"),
        this,
        SLOT(onModelProgress(QString, int)));

    if (!stateOk || !finalOk || !failedOk || !hotwordOk || !modelOk || !modelProgOk) {
        qCWarning(lcVoice) << "Subscription failed:"
                           << "state=" << stateOk
                           << "final=" << finalOk
                           << "failed=" << failedOk
                           << "hotword=" << hotwordOk
                           << "model=" << modelOk
                           << "modelProgress=" << modelProgOk;
    }

    // Probe reachability by asking for the current state. The voice daemon
    // may not be on the bus yet (Wants= not Requires= in the session
    // target), so a failed call here is informational, not fatal.
    auto pending = m_iface->asyncCall(QStringLiteral("GetState"));
    auto* watcher = new QDBusPendingCallWatcher(pending, this);
    QObject::connect(watcher, &QDBusPendingCallWatcher::finished, this,
        [this](QDBusPendingCallWatcher* w) {
            QDBusPendingReply<QString> reply = *w;
            setReachable(!reply.isError());
            w->deleteLater();
        });
}

void VoiceBridge::toggle()
{
    if (!m_iface) return;
    const QString method = (m_state == QStringLiteral("listening"))
        ? QStringLiteral("StopListening")
        : QStringLiteral("StartListening");

    auto pending = m_iface->asyncCall(method);
    auto* watcher = new QDBusPendingCallWatcher(pending, this);
    QObject::connect(watcher, &QDBusPendingCallWatcher::finished, this,
        [this, method](QDBusPendingCallWatcher* w) {
            QDBusPendingReply<QString> reply = *w;
            if (reply.isError()) {
                qCWarning(lcVoice) << method << "failed:" << reply.error().message();
                m_lastError = reply.error().message();
                emit lastErrorChanged();
            } else {
                qCInfo(lcVoice) << method << "reply=" << reply.value();
            }
            w->deleteLater();
        });
}

void VoiceBridge::cancel()
{
    if (!m_iface) return;
    m_iface->asyncCall(QStringLiteral("Cancel"));
}

void VoiceBridge::speak(const QString& text)
{
    if (!m_iface) return;
    m_iface->asyncCall(QStringLiteral("Speak"), text);
}

void VoiceBridge::ensureModel(const QString& name)
{
    if (!m_iface) return;
    qCInfo(lcVoice) << "ensureModel" << name;
    m_modelStatus = tr("verificando %1…").arg(name);
    emit modelStatusChanged();

    auto pending = m_iface->asyncCall(QStringLiteral("EnsureModel"), name);
    auto* watcher = new QDBusPendingCallWatcher(pending, this);
    QObject::connect(watcher, &QDBusPendingCallWatcher::finished, this,
        [this, name](QDBusPendingCallWatcher* w) {
            QDBusPendingReply<QString> reply = *w;
            if (reply.isError()) {
                m_modelStatus = tr("erro: %1").arg(reply.error().message());
                m_modelPercent = -1;
            } else {
                const auto obj = QJsonDocument::fromJson(reply.value().toUtf8()).object();
                if (obj.value(QStringLiteral("present")).toBool()) {
                    m_modelStatus = tr("%1 pronto").arg(name);
                    m_modelPercent = -1;
                } else if (obj.value(QStringLiteral("started")).toBool()) {
                    // Download running in the daemon; ModelProgress drives the
                    // bar, ModelReady finishes it.
                    m_modelStatus = tr("baixando %1…").arg(name);
                    m_modelPercent = 0;
                } else {
                    m_modelStatus = obj.value(QStringLiteral("reason"))
                                        .toString(tr("erro ao baixar %1").arg(name));
                    m_modelPercent = -1;
                }
            }
            emit modelStatusChanged();
            w->deleteLater();
        });
}

void VoiceBridge::onModelProgress(const QString& name, int percent)
{
    m_modelPercent = percent;
    m_modelStatus = tr("baixando %1… %2%").arg(name).arg(percent);
    emit modelStatusChanged();
}

void VoiceBridge::onModelReady(const QString& name, bool success, const QString& message)
{
    qCInfo(lcVoice) << "model ready" << name << success << message;
    m_modelStatus = success ? tr("%1 pronto").arg(name) : tr("erro: %1").arg(message);
    m_modelPercent = -1;
    emit modelStatusChanged();
}

void VoiceBridge::onStateChanged(const QString& state)
{
    qCInfo(lcVoice) << "state ->" << state;
    if (state == m_state) return;
    m_state = state;
    emit stateChanged();
    if (!m_reachable) setReachable(true);
}

void VoiceBridge::onTranscriptionFinal(const QString& text)
{
    qCInfo(lcVoice) << "transcript:" << text;
    m_lastTranscript = text;
    emit lastTranscriptChanged();
}

void VoiceBridge::onTranscriptionFailed(const QString& reason)
{
    qCWarning(lcVoice) << "transcription failed:" << reason;
    m_lastError = reason;
    emit lastErrorChanged();
}

void VoiceBridge::onHotwordDetected(const QString& text)
{
    qCInfo(lcVoice) << "hotword detected:" << text;
    // Strip everything up to and including the wake-word so QML sees
    // just the user's command. Keep the matching loose — the daemon
    // accepts several phrasings (oi/ei/olá/hey/ok lilith), and
    // whichever fired here is the one to remove.
    static const QStringList wakeWords = {
        QStringLiteral("oi lilith"),
        QStringLiteral("ei lilith"),
        QStringLiteral("olá lilith"),
        QStringLiteral("ola lilith"),
        QStringLiteral("hey lilith"),
        QStringLiteral("ok lilith"),
    };
    QString remainder;
    const QString lower = text.toLower();
    for (const auto& w : wakeWords) {
        const int idx = lower.indexOf(w);
        if (idx >= 0) {
            remainder = text.mid(idx + w.size()).trimmed();
            break;
        }
    }
    // Drop a leading comma / punctuation the user might've spoken
    // ("oi lilith, abre o navegador").
    while (!remainder.isEmpty() &&
           (remainder.front() == QChar(',') ||
            remainder.front() == QChar('.') ||
            remainder.front() == QChar(';'))) {
        remainder.remove(0, 1);
    }
    remainder = remainder.trimmed();

    emit wakeWordTriggered(text, remainder);
}

void VoiceBridge::setHotwordEnabled(bool enabled)
{
    if (!m_iface) return;
    qCInfo(lcVoice) << "setHotwordEnabled" << enabled;
    const QString method = enabled
        ? QStringLiteral("StartHotword")
        : QStringLiteral("StopHotword");
    auto pending = m_iface->asyncCall(method);
    auto* watcher = new QDBusPendingCallWatcher(pending, this);
    QObject::connect(watcher, &QDBusPendingCallWatcher::finished, this,
        [this, enabled, method](QDBusPendingCallWatcher* w) {
            QDBusPendingReply<QString> reply = *w;
            if (reply.isError()) {
                qCWarning(lcVoice) << method << "failed:" << reply.error().message();
                m_lastError = reply.error().message();
                emit lastErrorChanged();
            } else {
                setHotwordEnabledInternal(enabled);
            }
            w->deleteLater();
        });
}

void VoiceBridge::setReachable(bool v)
{
    if (m_reachable == v) return;
    m_reachable = v;
    emit reachableChanged();
}

void VoiceBridge::setHotwordEnabledInternal(bool v)
{
    if (m_hotwordEnabled == v) return;
    m_hotwordEnabled = v;
    emit hotwordEnabledChanged();
}

// ── Voiceprint enrollment surface ─────────────────────────────────

void VoiceBridge::enrollVoiceprint(const QString& user, int seconds)
{
    if (!m_iface) return;
    const quint32 clamped = static_cast<quint32>(qBound(1, seconds, 10));
    m_lastEnrollMessage = QStringLiteral("Capturando %1s para %2…").arg(clamped).arg(user);
    emit lastEnrollMessageChanged();

    auto pending = m_iface->asyncCall(QStringLiteral("EnrollVoiceprint"), user, clamped);
    auto* watcher = new QDBusPendingCallWatcher(pending, this);
    QObject::connect(watcher, &QDBusPendingCallWatcher::finished, this,
        [this, user](QDBusPendingCallWatcher* w) {
            QDBusPendingReply<QString> reply = *w;
            if (reply.isError()) {
                m_lastEnrollMessage = reply.error().message();
                emit lastEnrollMessageChanged();
                w->deleteLater();
                return;
            }
            const auto doc = QJsonDocument::fromJson(reply.value().toUtf8());
            const auto obj = doc.object();
            if (obj.value(QStringLiteral("ok")).toBool()) {
                m_lastEnrollMessage = tr("Voz registrada para %1.").arg(user);
                refreshEnrolledUsers();
            } else {
                m_lastEnrollMessage = obj.value(QStringLiteral("reason"))
                    .toString(tr("Falha ao registrar voz."));
            }
            emit lastEnrollMessageChanged();
            w->deleteLater();
        });
}

void VoiceBridge::verifyVoiceprint(const QString& user)
{
    if (!m_iface) return;
    m_lastEnrollMessage = QStringLiteral("Verificando %1…").arg(user);
    emit lastEnrollMessageChanged();

    auto pending = m_iface->asyncCall(QStringLiteral("VerifyVoiceprint"), user);
    auto* watcher = new QDBusPendingCallWatcher(pending, this);
    QObject::connect(watcher, &QDBusPendingCallWatcher::finished, this,
        [this, user](QDBusPendingCallWatcher* w) {
            QDBusPendingReply<QString> reply = *w;
            if (reply.isError()) {
                m_lastEnrollMessage = reply.error().message();
                emit lastEnrollMessageChanged();
                w->deleteLater();
                return;
            }
            const auto doc = QJsonDocument::fromJson(reply.value().toUtf8());
            const auto obj = doc.object();
            const bool ok = obj.value(QStringLiteral("ok")).toBool();
            const double score = obj.value(QStringLiteral("score")).toDouble();
            if (ok) {
                m_lastEnrollMessage = tr("Voz de %1 reconhecida (score %2).")
                    .arg(user).arg(score, 0, 'f', 2);
            } else {
                const auto reason = obj.value(QStringLiteral("reason")).toString();
                if (!reason.isEmpty()) {
                    m_lastEnrollMessage = reason;
                } else {
                    m_lastEnrollMessage = tr("Voz não reconhecida (score %1).")
                        .arg(score, 0, 'f', 2);
                }
            }
            emit lastEnrollMessageChanged();
            w->deleteLater();
        });
}

void VoiceBridge::deleteVoiceprint(const QString& user)
{
    if (!m_iface) return;
    auto pending = m_iface->asyncCall(QStringLiteral("DeleteVoiceprint"), user);
    auto* watcher = new QDBusPendingCallWatcher(pending, this);
    QObject::connect(watcher, &QDBusPendingCallWatcher::finished, this,
        [this, user](QDBusPendingCallWatcher* w) {
            QDBusPendingReply<QString> reply = *w;
            if (!reply.isError()) {
                refreshEnrolledUsers();
                m_lastEnrollMessage = tr("Registro de %1 removido.").arg(user);
                emit lastEnrollMessageChanged();
            }
            w->deleteLater();
        });
}

void VoiceBridge::refreshEnrolledUsers()
{
    if (!m_iface) return;
    auto pending = m_iface->asyncCall(QStringLiteral("ListEnrolled"));
    auto* watcher = new QDBusPendingCallWatcher(pending, this);
    QObject::connect(watcher, &QDBusPendingCallWatcher::finished, this,
        [this](QDBusPendingCallWatcher* w) {
            QDBusPendingReply<QString> reply = *w;
            if (reply.isError()) {
                w->deleteLater();
                return;
            }
            const auto doc = QJsonDocument::fromJson(reply.value().toUtf8());
            const auto arr = doc.object().value(QStringLiteral("users")).toArray();
            QVariantList out;
            for (const auto& v : arr) {
                if (v.isObject()) out.append(v.toObject().toVariantMap());
            }
            m_enrolledUsers = out;
            emit enrolledUsersChanged();
            w->deleteLater();
        });
}
