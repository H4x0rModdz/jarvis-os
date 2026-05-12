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

    if (!stateOk || !finalOk || !failedOk) {
        qCWarning(lcVoice) << "Subscription failed:"
                           << "state=" << stateOk
                           << "final=" << finalOk
                           << "failed=" << failedOk;
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

void VoiceBridge::setReachable(bool v)
{
    if (m_reachable == v) return;
    m_reachable = v;
    emit reachableChanged();
}
