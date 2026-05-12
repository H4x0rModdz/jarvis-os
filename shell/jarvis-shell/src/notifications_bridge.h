#pragma once

#include <QDBusInterface>
#include <QObject>
#include <QString>
#include <qqmlintegration.h>

/// Bridge to `com.jarvis.Notifications`. Listens for the daemon's
/// `NotificationPosted` signal and exposes the most recent notification
/// as Q_PROPERTYs the toast QML binds to. A `tick` counter increments
/// on every new arrival so the toast component can re-trigger its
/// entry animation even if the same notification id is replaced.
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

public:
    explicit NotificationsBridge(QObject* parent = nullptr);

    int tick() const { return m_tick; }
    quint32 currentId() const { return m_id; }
    QString currentApp() const { return m_app; }
    QString currentSummary() const { return m_summary; }
    QString currentBody() const { return m_body; }
    QString currentUrgency() const { return m_urgency; }

signals:
    void notificationChanged();

private slots:
    void onPosted(uint id, const QString& app, const QString& summary,
                  const QString& body, const QString& urgency);

private:
    int m_tick = 0;
    quint32 m_id = 0;
    QString m_app;
    QString m_summary;
    QString m_body;
    QString m_urgency;
};
