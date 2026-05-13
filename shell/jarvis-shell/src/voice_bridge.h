#pragma once

#include <QDBusInterface>
#include <QObject>
#include <QString>
#include <qqmlintegration.h>

/// Bridge between QML and `com.jarvis.Voice`.
///
/// Subscribes to `StateChanged` and exposes the current state as a Q_PROPERTY
/// so the mic button on the bar can render different visuals (idle / listening
/// / processing / speaking). Subscribes to `TranscriptionFinal` so a future
/// auto-pipe-into-Lilith feature can route the transcript through the same
/// path the typed input uses today.
///
/// V1 surface — the underlying daemon's STT/TTS still return Unavailable.
/// The bridge is shipped now so the contract is settled before V2's
/// whisper.cpp work changes things on the daemon side.
class VoiceBridge : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    Q_PROPERTY(QString state READ state NOTIFY stateChanged)
    Q_PROPERTY(bool reachable READ reachable NOTIFY reachableChanged)
    Q_PROPERTY(QString lastTranscript READ lastTranscript NOTIFY lastTranscriptChanged)
    Q_PROPERTY(QString lastError READ lastError NOTIFY lastErrorChanged)
    Q_PROPERTY(bool hotwordEnabled READ hotwordEnabled NOTIFY hotwordEnabledChanged)

public:
    explicit VoiceBridge(QObject* parent = nullptr);

    QString state() const { return m_state; }
    bool reachable() const { return m_reachable; }
    QString lastTranscript() const { return m_lastTranscript; }
    QString lastError() const { return m_lastError; }
    bool hotwordEnabled() const { return m_hotwordEnabled; }

    /// Toggle press-to-talk. When idle, sends StartListening; when listening,
    /// sends StopListening. Anything else (processing / speaking) is ignored.
    Q_INVOKABLE void toggle();

    Q_INVOKABLE void cancel();

    /// Ask the daemon to speak `text`. V1 cycles through the speaking state
    /// briefly but plays no audio; V3 wires piper.
    Q_INVOKABLE void speak(const QString& text);

    /// Engage / disengage the hotword listener on the daemon. Persisted
    /// to com.jarvis.Settings under `voice.hotword.enabled` so the
    /// preference survives across restarts; the daemon itself starts
    /// disabled every boot for safety.
    Q_INVOKABLE void setHotwordEnabled(bool enabled);

signals:
    void stateChanged();
    void reachableChanged();
    void lastTranscriptChanged();
    void lastErrorChanged();
    void hotwordEnabledChanged();

    /// Emitted after the daemon's HotwordDetected signal lands.
    /// `fullTranscript` is the Whisper output verbatim;
    /// `remainder` is whatever follows the wake-word (empty when the
    /// user said only "oi lilith"). The QML side decides whether to
    /// dispatch to Lilith immediately or pop the mic into listening.
    void wakeWordTriggered(const QString& fullTranscript, const QString& remainder);

private slots:
    void onStateChanged(const QString& state);
    void onTranscriptionFinal(const QString& text);
    void onTranscriptionFailed(const QString& reason);
    void onHotwordDetected(const QString& text);

private:
    void setReachable(bool v);
    void setHotwordEnabledInternal(bool v);

    QDBusInterface* m_iface = nullptr;
    QString m_state = QStringLiteral("idle");
    bool m_reachable = false;
    QString m_lastTranscript;
    QString m_lastError;
    bool m_hotwordEnabled = false;
};
