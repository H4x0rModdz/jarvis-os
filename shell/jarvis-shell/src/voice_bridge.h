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

public:
    explicit VoiceBridge(QObject* parent = nullptr);

    QString state() const { return m_state; }
    bool reachable() const { return m_reachable; }
    QString lastTranscript() const { return m_lastTranscript; }
    QString lastError() const { return m_lastError; }

    /// Toggle press-to-talk. When idle, sends StartListening; when listening,
    /// sends StopListening. Anything else (processing / speaking) is ignored.
    Q_INVOKABLE void toggle();

    Q_INVOKABLE void cancel();

    /// Ask the daemon to speak `text`. V1 cycles through the speaking state
    /// briefly but plays no audio; V3 wires piper.
    Q_INVOKABLE void speak(const QString& text);

signals:
    void stateChanged();
    void reachableChanged();
    void lastTranscriptChanged();
    void lastErrorChanged();

private slots:
    void onStateChanged(const QString& state);
    void onTranscriptionFinal(const QString& text);
    void onTranscriptionFailed(const QString& reason);

private:
    void setReachable(bool v);

    QDBusInterface* m_iface = nullptr;
    QString m_state = QStringLiteral("idle");
    bool m_reachable = false;
    QString m_lastTranscript;
    QString m_lastError;
};
