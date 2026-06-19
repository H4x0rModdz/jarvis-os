#pragma once

#include <QDBusInterface>
#include <QObject>
#include <QString>
#include <QStringList>
#include <QTimer>
#include <QVariantList>
#include <qqmlintegration.h>

/// QObject bridge between QML and com.jarvis.Lilith.
///
/// Exposes:
///   - `reachable` (read-only Q_PROPERTY): true when the daemon answers a ping
///   - `busy` (read-only): true between send() and replyReceived
///   - `streamingText`: the accumulated partial-reply text for the current
///     command — chunks land via the daemon's PartialReply signal
///   - `chainSteps`: per-step action names for the current command —
///     entries land via the daemon's ChainStep signal
///   - `send(text)` Q_INVOKABLE: dispatch a natural-language command async
///   - `replyReceived(reply, action, result)` signal: fired when Lilith answers
///   - `errorOccurred(message)` signal: fired on DBus / network failure
///
/// Streaming state resets each time `send()` is called so the UI shows
/// the current command in flight, not the previous one's residue.
class LilithBridge : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    Q_PROPERTY(bool reachable READ reachable NOTIFY reachableChanged)
    Q_PROPERTY(bool busy READ busy NOTIFY busyChanged)
    /// Coarse mood of the most recent reply (neutral / happy / concerned),
    /// tagged by the daemon (ADR 0028). The embodied avatar reads this to pick
    /// a facial expression. Resets to "neutral" when a new command is sent.
    Q_PROPERTY(QString emotion READ emotion NOTIFY emotionChanged)
    Q_PROPERTY(QString streamingText READ streamingText NOTIFY streamingTextChanged)
    Q_PROPERTY(QVariantList chainSteps READ chainSteps NOTIFY chainStepsChanged)
    Q_PROPERTY(QVariantList conversation READ conversation NOTIFY conversationChanged)
    Q_PROPERTY(QString proactiveNudgeText READ proactiveNudgeText NOTIFY proactiveNudgeChanged)
    Q_PROPERTY(QString proactiveNudgeUrgency READ proactiveNudgeUrgency NOTIFY proactiveNudgeChanged)
    Q_PROPERTY(QString proactiveNudgeRule READ proactiveNudgeRule NOTIFY proactiveNudgeChanged)
    Q_PROPERTY(qint64 proactiveNudgeReceivedAt READ proactiveNudgeReceivedAt NOTIFY proactiveNudgeChanged)

public:
    explicit LilithBridge(QObject* parent = nullptr);

    bool reachable() const { return m_reachable; }
    bool busy() const { return m_busy; }
    QString emotion() const { return m_emotion; }
    QString streamingText() const { return m_streamingText; }
    QVariantList chainSteps() const { return m_chainSteps; }
    QVariantList conversation() const { return m_conversation; }

    QString proactiveNudgeText() const { return m_proactiveNudgeText; }
    QString proactiveNudgeUrgency() const { return m_proactiveNudgeUrgency; }
    QString proactiveNudgeRule() const { return m_proactiveNudgeRule; }
    qint64 proactiveNudgeReceivedAt() const { return m_proactiveNudgeReceivedAt; }

    Q_INVOKABLE void send(const QString& text);

    /// Wipe the conversation history. Calls into the daemon's Reset()
    /// so the session memory there also clears — keeps the two views
    /// in sync. Triggered by a UI "limpar" button (Phase 11 popup).
    Q_INVOKABLE void resetConversation();

    /// Clear the displayed proactive nudge. Doesn't tell the daemon
    /// anything — cooldowns there are per-rule and will pause that
    /// rule for its configured window regardless. This is purely a
    /// UI-side dismiss.
    Q_INVOKABLE void dismissProactiveNudge();

signals:
    void reachableChanged();
    void busyChanged();
    void emotionChanged();
    void streamingTextChanged();
    void chainStepsChanged();
    void conversationChanged();
    void replyReceived(const QString& reply, const QString& action, const QString& resultJson);
    void errorOccurred(const QString& message);
    /// Fires whenever the daemon emits a fresh ProactiveNudge. The
    /// popup uses this to auto-open if it's currently hidden.
    void proactiveNudgeReceived(const QString& rule,
                                const QString& text,
                                const QString& urgency);
    void proactiveNudgeChanged();

private slots:
    void ping();
    void onPartialReply(uint step, const QString& chunk);
    void onChainStep(uint step, const QString& action);
    void onProactiveNudge(const QString& rule,
                          const QString& text,
                          const QString& urgency);

private:
    void setReachable(bool v);
    void setBusy(bool v);
    void setEmotion(const QString& v);
    void resetStreamingState();
    void pushConversationUser(const QString& text);
    void pushConversationLilith(const QString& reply, const QString& action,
                                const QVariantList& chainSteps);

    QDBusInterface* m_iface = nullptr;
    QTimer m_pingTimer;
    bool m_reachable = false;
    bool m_busy = false;
    QString m_emotion = QStringLiteral("neutral");
    QString m_streamingText;
    QVariantList m_chainSteps; // [{ step: int, action: string }, …]
    QVariantList m_conversation;
    QString m_proactiveNudgeText;
    QString m_proactiveNudgeUrgency;
    QString m_proactiveNudgeRule;
    qint64 m_proactiveNudgeReceivedAt = 0;
    static constexpr int kConversationCap = 32;
};
