#pragma once

#include <QLocalSocket>
#include <QObject>
#include <QString>
#include <qqmlintegration.h>

/// Speaks the greetd JSON-over-Unix-socket protocol.
///
/// Length-prefixed framing: every message is `[u32 LE length][JSON
/// payload]`. The protocol is documented at
/// https://man.sr.ht/~kennylevinsen/greetd/#protocol.
///
/// Exposes a tiny state machine to QML: the LoginScreen binds to
/// `state`, `prompt`, and `error` and reacts. The user types into a
/// password field, hits Enter, and the bridge fires the appropriate
/// next message. On `Success` after `start_session`, the greeter
/// quits — greetd takes over and execs the user's session command.
class GreetdClient : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    Q_PROPERTY(QString state READ state NOTIFY stateChanged)
    Q_PROPERTY(QString prompt READ prompt NOTIFY stateChanged)
    Q_PROPERTY(bool secret READ secret NOTIFY stateChanged)
    Q_PROPERTY(QString error READ error NOTIFY stateChanged)

public:
    explicit GreetdClient(QObject* parent = nullptr);

    QString state() const { return m_state; }
    QString prompt() const { return m_prompt; }
    /// When true, the prompt is for a secret (hide input). When false,
    /// it's an informational message.
    bool secret() const { return m_secret; }
    QString error() const { return m_error; }

    /// Start the auth flow with `username`. Transitions through the
    /// auth-message loop until greetd's PAM stack signals success.
    ///
    /// `password` is optional. When provided, it is held just long
    /// enough to answer greetd's first `secret` prompt automatically —
    /// so a user who typed their password and clicked UNLOCK logs in on
    /// that single click instead of having to click again once the
    /// prompt arrives. It is cleared the instant it leaves the socket.
    Q_INVOKABLE void beginLogin(const QString& username,
                                const QString& password = QString());

    /// Send the user's response to the current auth prompt.
    Q_INVOKABLE void answerPrompt(const QString& response);

    /// Cancel the in-flight session and reset to idle. Used when the
    /// user backs out mid-prompt.
    Q_INVOKABLE void cancel();

signals:
    void stateChanged();

private slots:
    void onSocketReadyRead();
    void onSocketError(QLocalSocket::LocalSocketError err);

private:
    void connectSocket();
    void sendMessage(const QByteArray& json);
    void handleResponse(const QByteArray& json);
    void requestStartSession();
    void setState(const QString& state, const QString& prompt = {},
                  bool secret = false, const QString& error = {});

    QLocalSocket m_socket;
    QByteArray m_inBuffer;
    QString m_state = QStringLiteral("idle");
    QString m_prompt;
    bool m_secret = false;
    QString m_error;
    QString m_username;
    // Password queued by beginLogin, consumed by the first secret
    // prompt. Held only across the create_session round-trip.
    QString m_pendingSecret;
    bool m_havePendingSecret = false;
};
