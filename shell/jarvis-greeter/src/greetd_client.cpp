#include "greetd_client.h"

#include <QByteArray>
#include <QCoreApplication>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLoggingCategory>
#include <QtEndian>

namespace {
Q_LOGGING_CATEGORY(lcGreet, "jarvis.greeter")

QString socketPath()
{
    const QByteArray fromEnv = qgetenv("GREETD_SOCK");
    if (!fromEnv.isEmpty()) return QString::fromLocal8Bit(fromEnv);
    return QStringLiteral("/run/greetd.sock");
}
}

GreetdClient::GreetdClient(QObject* parent) : QObject(parent)
{
    QObject::connect(&m_socket, &QLocalSocket::readyRead,
                     this, &GreetdClient::onSocketReadyRead);
    QObject::connect(&m_socket, &QLocalSocket::errorOccurred,
                     this, &GreetdClient::onSocketError);
}

void GreetdClient::connectSocket()
{
    if (m_socket.state() == QLocalSocket::ConnectedState) return;
    const QString path = socketPath();
    qCInfo(lcGreet) << "Connecting to" << path;
    m_socket.connectToServer(path);
    if (!m_socket.waitForConnected(2000)) {
        setState(QStringLiteral("error"), {}, false,
                 tr("Não foi possível conectar ao greetd: ") + m_socket.errorString());
    }
}

void GreetdClient::beginLogin(const QString& username)
{
    m_username = username;
    m_inBuffer.clear();
    setState(QStringLiteral("creating_session"), {}, false, {});
    connectSocket();
    if (m_state == QStringLiteral("error")) return;

    QJsonObject msg;
    msg.insert(QStringLiteral("type"), QStringLiteral("create_session"));
    msg.insert(QStringLiteral("username"), username);
    sendMessage(QJsonDocument(msg).toJson(QJsonDocument::Compact));
}

void GreetdClient::answerPrompt(const QString& response)
{
    QJsonObject msg;
    msg.insert(QStringLiteral("type"), QStringLiteral("post_auth_message_response"));
    msg.insert(QStringLiteral("response"), response);
    setState(QStringLiteral("checking"), m_prompt, m_secret, {});
    sendMessage(QJsonDocument(msg).toJson(QJsonDocument::Compact));
}

void GreetdClient::cancel()
{
    QJsonObject msg;
    msg.insert(QStringLiteral("type"), QStringLiteral("cancel_session"));
    sendMessage(QJsonDocument(msg).toJson(QJsonDocument::Compact));
    setState(QStringLiteral("idle"), {}, false, {});
}

void GreetdClient::sendMessage(const QByteArray& json)
{
    if (m_socket.state() != QLocalSocket::ConnectedState) {
        qCWarning(lcGreet) << "sendMessage with no live socket";
        return;
    }
    quint32 len = static_cast<quint32>(json.size());
    quint32 le = qToLittleEndian(len);
    QByteArray frame(reinterpret_cast<const char*>(&le), sizeof(le));
    frame.append(json);
    qCDebug(lcGreet) << "→" << json;
    m_socket.write(frame);
}

void GreetdClient::onSocketReadyRead()
{
    m_inBuffer.append(m_socket.readAll());

    // Drain as many complete frames as we have.
    while (true) {
        if (m_inBuffer.size() < int(sizeof(quint32))) return;
        quint32 le;
        memcpy(&le, m_inBuffer.constData(), sizeof(le));
        quint32 len = qFromLittleEndian(le);
        if (m_inBuffer.size() < int(sizeof(quint32) + len)) return;

        QByteArray payload = m_inBuffer.mid(sizeof(quint32), len);
        m_inBuffer.remove(0, sizeof(quint32) + len);
        handleResponse(payload);
    }
}

void GreetdClient::handleResponse(const QByteArray& json)
{
    qCDebug(lcGreet) << "←" << json;
    QJsonDocument doc = QJsonDocument::fromJson(json);
    if (!doc.isObject()) {
        setState(QStringLiteral("error"), {}, false,
                 tr("Resposta inválida do greetd"));
        return;
    }
    const QJsonObject obj = doc.object();
    const QString type = obj.value(QStringLiteral("type")).toString();

    if (type == QStringLiteral("success")) {
        if (m_state == QStringLiteral("starting_session")) {
            // greetd has accepted start_session — we're done. The session
            // command will be exec'd by greetd; our process exits.
            qCInfo(lcGreet) << "Session start accepted — exiting";
            QCoreApplication::quit();
            return;
        }
        // success after create_session / post_auth_message_response means
        // auth is done; advance to start_session.
        requestStartSession();
        return;
    }

    if (type == QStringLiteral("auth_message")) {
        const QString prompt = obj.value(QStringLiteral("auth_message")).toString();
        const QString kind = obj.value(QStringLiteral("auth_message_type")).toString();
        const bool secret = kind == QStringLiteral("secret");
        setState(QStringLiteral("awaiting_response"), prompt, secret, {});
        return;
    }

    if (type == QStringLiteral("error")) {
        const QString desc = obj.value(QStringLiteral("description")).toString();
        const QString kind = obj.value(QStringLiteral("error_type")).toString();
        qCWarning(lcGreet) << "greetd error:" << kind << desc;
        setState(QStringLiteral("idle"), {}, false,
                 desc.isEmpty() ? tr("Falha na autenticação") : desc);
        return;
    }

    setState(QStringLiteral("error"), {}, false,
             tr("Tipo de mensagem desconhecido: ") + type);
}

void GreetdClient::requestStartSession()
{
    setState(QStringLiteral("starting_session"), {}, false, {});
    QJsonObject msg;
    msg.insert(QStringLiteral("type"), QStringLiteral("start_session"));
    // Wrap labwc in the session launcher so it inherits the same
    // renderer decision the greeter ran under — pixman on hosts where
    // EGL/dmabuf is broken (VirtualBox VMSVGA), GPU path everywhere
    // else. See iso/assets/launchers/jarvis-session-launch.
    QJsonArray cmd;
    cmd.append(QStringLiteral("/usr/libexec/jarvis-session-launch"));
    cmd.append(QStringLiteral("labwc"));
    msg.insert(QStringLiteral("cmd"), cmd);
    QJsonArray env;
    env.append(QStringLiteral("XDG_SESSION_TYPE=wayland"));
    env.append(QStringLiteral("XDG_SESSION_DESKTOP=jarvis"));
    msg.insert(QStringLiteral("env"), env);
    sendMessage(QJsonDocument(msg).toJson(QJsonDocument::Compact));
}

void GreetdClient::onSocketError(QLocalSocket::LocalSocketError /*err*/)
{
    setState(QStringLiteral("error"), {}, false, m_socket.errorString());
}

void GreetdClient::setState(const QString& state, const QString& prompt,
                            bool secret, const QString& error)
{
    const bool changed = state != m_state || prompt != m_prompt
                        || secret != m_secret || error != m_error;
    m_state = state;
    m_prompt = prompt;
    m_secret = secret;
    m_error = error;
    if (changed) emit stateChanged();
}
