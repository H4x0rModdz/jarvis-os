#pragma once

#include <QDBusInterface>
#include <QObject>
#include <QString>
#include <QVariantList>
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
    Q_PROPERTY(QVariantList enrolledUsers READ enrolledUsers NOTIFY enrolledUsersChanged)
    Q_PROPERTY(QString lastEnrollMessage READ lastEnrollMessage NOTIFY lastEnrollMessageChanged)
    /// Human-readable status of the last `ensureModel` call (e.g. "baixando
    /// medium…", "medium pronto", "erro: …"). Empty when idle. The Settings
    /// panel shows it under the model picker.
    Q_PROPERTY(QString modelStatus READ modelStatus NOTIFY modelStatusChanged)
    /// Download progress of the model fetch, 0–100, or -1 when not downloading.
    /// The panel draws a real progress bar off this.
    Q_PROPERTY(int modelPercent READ modelPercent NOTIFY modelStatusChanged)
    /// `$USER` — same identity the PAM module sees during a verify
    /// call. Settings panel enrolls/verifies against this so the
    /// enrollment matches the lock-screen unlock target.
    Q_PROPERTY(QString currentUser READ currentUser CONSTANT)

public:
    explicit VoiceBridge(QObject* parent = nullptr);

    QString state() const { return m_state; }
    bool reachable() const { return m_reachable; }
    QString lastTranscript() const { return m_lastTranscript; }
    QString lastError() const { return m_lastError; }
    bool hotwordEnabled() const { return m_hotwordEnabled; }
    QVariantList enrolledUsers() const { return m_enrolledUsers; }
    QString lastEnrollMessage() const { return m_lastEnrollMessage; }
    QString currentUser() const { return m_currentUser; }
    QString modelStatus() const { return m_modelStatus; }
    int modelPercent() const { return m_modelPercent; }

    /// Ask the daemon to make whisper model `name` available, downloading it
    /// if missing. The QML writes `voice.model` to Settings (which stt reads
    /// live); this only triggers the download + reports progress via
    /// `modelStatus`. No-op-ish when the model is already present.
    Q_INVOKABLE void ensureModel(const QString& name);

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

    /// Capture `seconds` of audio and store a voiceprint for `user`.
    /// Updates `enrolledUsers` + emits `lastEnrollMessageChanged`
    /// when the DBus call returns.
    Q_INVOKABLE void enrollVoiceprint(const QString& user, int seconds);

    /// Capture ~2 s and compare against `user`'s stored voiceprint.
    /// Updates `lastEnrollMessage` with the verdict + score so the
    /// settings panel can render feedback.
    Q_INVOKABLE void verifyVoiceprint(const QString& user);

    /// Remove `user`'s enrolled voiceprint from the daemon's store.
    /// Updates `enrolledUsers`.
    Q_INVOKABLE void deleteVoiceprint(const QString& user);

    /// Pull the current enrolled-users list out of the daemon. The
    /// settings panel calls this on open + after every mutation.
    Q_INVOKABLE void refreshEnrolledUsers();

signals:
    void stateChanged();
    void reachableChanged();
    void lastTranscriptChanged();
    void lastErrorChanged();
    void hotwordEnabledChanged();
    void enrolledUsersChanged();
    void lastEnrollMessageChanged();
    void modelStatusChanged();

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
    void onModelReady(const QString& name, bool success, const QString& message);
    void onModelProgress(const QString& name, int percent);

private:
    void setReachable(bool v);
    void setHotwordEnabledInternal(bool v);

    QDBusInterface* m_iface = nullptr;
    QString m_state = QStringLiteral("idle");
    bool m_reachable = false;
    QString m_lastTranscript;
    QString m_lastError;
    bool m_hotwordEnabled = false;
    QVariantList m_enrolledUsers;
    QString m_lastEnrollMessage;
    QString m_currentUser;
    QString m_modelStatus;
    int m_modelPercent = -1;
};
