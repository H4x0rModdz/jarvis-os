#pragma once

#include <QDBusInterface>
#include <QObject>
#include <QString>
#include <QStringList>
#include <QVariantList>
#include <qqmlintegration.h>

/// Bridge to `com.jarvis.Notifications`. Listens for the daemon's
/// `NotificationPosted` signal and exposes both the most recent
/// notification (for the toast) and the recent history (for the
/// drawer in the bar).
///
/// V2 additions:
///   - `currentActions` exposes the action button row, alternating
///     `key, label, key, label, …`, the same shape as FreeDesktop.
///   - `invokeAction(id, key)` calls back into the daemon so the
///     originating app sees the user's click.
///   - `history` is a model-friendly list of recent entries.
class NotificationsBridge : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    Q_PROPERTY(int tick READ tick NOTIFY notificationChanged)
    Q_PROPERTY(quint32 currentId READ currentId NOTIFY notificationChanged)
    Q_PROPERTY(QString currentApp READ currentApp NOTIFY notificationChanged)
    Q_PROPERTY(QString currentSummary READ currentSummary NOTIFY notificationChanged)
    Q_PROPERTY(QString currentBody READ currentBody NOTIFY notificationChanged)
    Q_PROPERTY(QString currentUrgency READ currentUrgency NOTIFY notificationChanged)
    Q_PROPERTY(QStringList currentActions READ currentActions NOTIFY notificationChanged)
    Q_PROPERTY(QVariantList history READ history NOTIFY historyChanged)

public:
    explicit NotificationsBridge(QObject* parent = nullptr);

    int tick() const { return m_tick; }
    quint32 currentId() const { return m_id; }
    QString currentApp() const { return m_app; }
    QString currentSummary() const { return m_summary; }
    QString currentBody() const { return m_body; }
    QString currentUrgency() const { return m_urgency; }
    QStringList currentActions() const { return m_actions; }
    QVariantList history() const { return m_history; }

    /// Fires the daemon's `InvokeAction` method — the originating
    /// app sees an `ActionInvoked(id, key)` signal and runs its
    /// callback. Toast / drawer buttons call this on click.
    Q_INVOKABLE void invokeAction(quint32 id, const QString& key);

    /// Refresh the history list (RecentNotifications RPC). Called
    /// on demand when the drawer opens.
    Q_INVOKABLE void refreshHistory();

    /// Drop one entry from the daemon's history. UI-only — the
    /// originating app is not notified. Used by the × on each
    /// drawer row.
    Q_INVOKABLE void dismiss(quint32 id);

    /// Wipe the daemon's history. Used by the drawer's "Clear all"
    /// button.
    Q_INVOKABLE void clear();

signals:
    void notificationChanged();
    void historyChanged();

private slots:
    void onPosted(uint id, const QString& app, const QString& summary,
                  const QString& body, const QString& urgency,
                  const QStringList& actions);
    void onHistoryChanged();

private:
    QDBusInterface* m_history_iface = nullptr;

    int m_tick = 0;
    quint32 m_id = 0;
    QString m_app;
    QString m_summary;
    QString m_body;
    QString m_urgency;
    QStringList m_actions;
    QVariantList m_history;
};
