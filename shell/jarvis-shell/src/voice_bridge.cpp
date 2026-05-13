#include "voice_bridge.h"

#include <QDBusConnection>
#include <QDBusPendingCall>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QLoggingCategory>

namespace {
Q_LOGGING_CATEGORY(lcVoice, "jarvis.shell.voice")

constexpr const char* kService = "com.jarvis.Voice";
constexpr const char* kPath = "/com/jarvis/Voice";
constexpr const char* kIface = "com.jarvis.Voice";
}

VoiceBridge::VoiceBridge(QObject* parent) : QObject(parent)
{
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

    if (!stateOk || !finalOk || !failedOk || !hotwordOk) {
        qCWarning(lcVoice) << "Subscription failed:"
                           << "state=" << stateOk
                           << "final=" << finalOk
                           << "failed=" << failedOk
                           << "hotword=" << hotwordOk;
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
