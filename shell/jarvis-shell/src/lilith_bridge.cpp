#include "lilith_bridge.h"

#include <QDBusConnection>
#include <QDBusPendingCall>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QDateTime>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLoggingCategory>
#include <QVariantMap>

namespace {
Q_LOGGING_CATEGORY(lcLilith, "jarvis.shell.lilith")

constexpr const char* kService = "com.jarvis.Lilith";
constexpr const char* kPath = "/com/jarvis/Lilith";
constexpr const char* kIface = "com.jarvis.Lilith";
constexpr int kPingIntervalMs = 3000;
constexpr int kCommandTimeoutMs = 120000; // matches the Ollama-aware timeout in Lilith
}

LilithBridge::LilithBridge(QObject* parent) : QObject(parent)
{
    auto bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        qCWarning(lcLilith) << "Session bus not connected";
        emit errorOccurred(tr("DBus session bus unavailable"));
        return;
    }

    m_iface = new QDBusInterface(kService, kPath, kIface, bus, this);
    m_iface->setTimeout(kCommandTimeoutMs);

    // Streaming signals from the daemon — PartialReply ferries Ollama
    // tokens, ChainStep ferries tool-dispatch transitions.
    const bool partialOk = bus.connect(
        kService, kPath, kIface,
        QStringLiteral("PartialReply"),
        this,
        SLOT(onPartialReply(uint, QString)));
    const bool chainOk = bus.connect(
        kService, kPath, kIface,
        QStringLiteral("ChainStep"),
        this,
        SLOT(onChainStep(uint, QString)));
    const bool nudgeOk = bus.connect(
        kService, kPath, kIface,
        QStringLiteral("ProactiveNudge"),
        this,
        SLOT(onProactiveNudge(QString, QString, QString)));
    if (!partialOk || !chainOk || !nudgeOk) {
        qCWarning(lcLilith) << "Streaming subscriptions failed:"
                            << "partial=" << partialOk
                            << "chain=" << chainOk
                            << "nudge=" << nudgeOk;
    }

    // Probe reachability immediately, then on a slow heartbeat.
    QObject::connect(&m_pingTimer, &QTimer::timeout, this, &LilithBridge::ping);
    m_pingTimer.start(kPingIntervalMs);
    ping();
}

void LilithBridge::ping()
{
    if (!m_iface) return;
    // ListFacts is the cheapest method that proves the daemon is alive AND
    // that our methods are registered. No side effects.
    auto pending = m_iface->asyncCall(QStringLiteral("ListFacts"));
    auto* watcher = new QDBusPendingCallWatcher(pending, this);
    QObject::connect(watcher, &QDBusPendingCallWatcher::finished, this,
        [this](QDBusPendingCallWatcher* w) {
            QDBusPendingReply<QString> reply = *w;
            setReachable(!reply.isError());
            w->deleteLater();
        });
}

void LilithBridge::send(const QString& text)
{
    if (!m_iface) {
        emit errorOccurred(tr("Lilith bridge not initialised"));
        return;
    }
    if (m_busy) {
        qCDebug(lcLilith) << "send() ignored while busy";
        return;
    }

    // Clear streaming state so the UI shows the current command in
    // flight, not residue from the previous one. Conversation history
    // accumulates across commands — push the user line now so the
    // popup can render the question even before the reply lands.
    resetStreamingState();
    pushConversationUser(text);
    setBusy(true);
    auto pending = m_iface->asyncCall(QStringLiteral("Command"), text);
    auto* watcher = new QDBusPendingCallWatcher(pending, this);
    QObject::connect(watcher, &QDBusPendingCallWatcher::finished, this,
        [this](QDBusPendingCallWatcher* w) {
            QDBusPendingReply<QString> reply = *w;
            setBusy(false);
            if (reply.isError()) {
                emit errorOccurred(reply.error().message());
                w->deleteLater();
                return;
            }

            // Lilith returns a JSON string with shape:
            //   { "reply": "...", "action": "..."|null, "result": {...}|null }
            const QByteArray raw = reply.value().toUtf8();
            const auto doc = QJsonDocument::fromJson(raw);
            if (!doc.isObject()) {
                emit errorOccurred(tr("Lilith returned non-JSON response"));
                w->deleteLater();
                return;
            }
            const auto obj = doc.object();
            const QString replyText = obj.value(QStringLiteral("reply")).toString();
            const QString action = obj.value(QStringLiteral("action")).toString();
            const QJsonValue result = obj.value(QStringLiteral("result"));
            const QString resultJson = result.isObject()
                ? QString::fromUtf8(QJsonDocument(result.toObject()).toJson(QJsonDocument::Compact))
                : QString();

            // Pin the chain-steps that landed during this command
            // onto the conversation entry so the popup can render
            // them even after the next command resets streaming
            // state.
            pushConversationLilith(replyText, action, m_chainSteps);
            emit replyReceived(replyText, action, resultJson);
            w->deleteLater();
        });
}

void LilithBridge::resetConversation()
{
    if (m_iface) {
        // Best-effort; nothing actionable on error since the local
        // conversation reset still proceeds.
        m_iface->asyncCall(QStringLiteral("Reset"));
    }
    if (!m_conversation.isEmpty()) {
        m_conversation.clear();
        emit conversationChanged();
    }
    resetStreamingState();
}

void LilithBridge::pushConversationUser(const QString& text)
{
    QVariantMap entry;
    entry.insert(QStringLiteral("role"), QStringLiteral("user"));
    entry.insert(QStringLiteral("text"), text);
    if (m_conversation.size() >= kConversationCap) {
        m_conversation.removeFirst();
    }
    m_conversation.append(entry);
    emit conversationChanged();
}

void LilithBridge::pushConversationLilith(const QString& reply,
                                          const QString& action,
                                          const QVariantList& chainSteps)
{
    QVariantMap entry;
    entry.insert(QStringLiteral("role"), QStringLiteral("lilith"));
    entry.insert(QStringLiteral("text"), reply);
    if (!action.isEmpty()) {
        entry.insert(QStringLiteral("action"), action);
    }
    if (!chainSteps.isEmpty()) {
        entry.insert(QStringLiteral("chainSteps"), chainSteps);
    }
    if (m_conversation.size() >= kConversationCap) {
        m_conversation.removeFirst();
    }
    m_conversation.append(entry);
    emit conversationChanged();
}

void LilithBridge::setReachable(bool v)
{
    if (m_reachable == v) return;
    m_reachable = v;
    emit reachableChanged();
}

void LilithBridge::setBusy(bool v)
{
    if (m_busy == v) return;
    m_busy = v;
    emit busyChanged();
}

void LilithBridge::onPartialReply(uint step, const QString& chunk)
{
    // Append to the current streaming buffer. Step boundaries are
    // already tracked via ChainStep; partial chunks just accumulate
    // text. The bar's input swaps to streamingText while busy.
    Q_UNUSED(step);
    if (chunk.isEmpty()) return;
    m_streamingText += chunk;
    emit streamingTextChanged();
}

void LilithBridge::onChainStep(uint step, const QString& action)
{
    QVariantMap entry;
    entry.insert(QStringLiteral("step"), static_cast<int>(step));
    entry.insert(QStringLiteral("action"), action);
    m_chainSteps.append(entry);
    emit chainStepsChanged();
}

void LilithBridge::onProactiveNudge(const QString& rule,
                                    const QString& text,
                                    const QString& urgency)
{
    m_proactiveNudgeRule = rule;
    m_proactiveNudgeText = text;
    m_proactiveNudgeUrgency = urgency;
    m_proactiveNudgeReceivedAt =
        QDateTime::currentMSecsSinceEpoch();
    emit proactiveNudgeChanged();
    emit proactiveNudgeReceived(rule, text, urgency);
}

void LilithBridge::dismissProactiveNudge()
{
    if (m_proactiveNudgeText.isEmpty()) return;
    m_proactiveNudgeText.clear();
    m_proactiveNudgeUrgency.clear();
    m_proactiveNudgeRule.clear();
    // Keep ReceivedAt so the UI can still display "há 3 min" history
    // if it wants; rendering hides on empty text anyway.
    emit proactiveNudgeChanged();
}

void LilithBridge::resetStreamingState()
{
    bool changed = false;
    if (!m_streamingText.isEmpty()) {
        m_streamingText.clear();
        emit streamingTextChanged();
        changed = true;
    }
    if (!m_chainSteps.isEmpty()) {
        m_chainSteps.clear();
        emit chainStepsChanged();
        changed = true;
    }
    Q_UNUSED(changed);
}
