#pragma once

#include <QObject>
#include <QString>
#include <QTimer>
#include <QDBusInterface>
#include <qqmlintegration.h>

/// QObject bridge between QML and com.jarvis.Lilith.
///
/// Exposes:
///   - `reachable` (read-only Q_PROPERTY): true when the daemon answers a ping
///   - `send(text)` Q_INVOKABLE: dispatch a natural-language command async
///   - `replyReceived(reply, action, result)` signal: fired when Lilith answers
///   - `errorOccurred(message)` signal: fired on DBus / network failure
class LilithBridge : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    Q_PROPERTY(bool reachable READ reachable NOTIFY reachableChanged)
    Q_PROPERTY(bool busy READ busy NOTIFY busyChanged)

public:
    explicit LilithBridge(QObject* parent = nullptr);

    bool reachable() const { return m_reachable; }
    bool busy() const { return m_busy; }

    Q_INVOKABLE void send(const QString& text);

signals:
    void reachableChanged();
    void busyChanged();
    void replyReceived(const QString& reply, const QString& action, const QString& resultJson);
    void errorOccurred(const QString& message);

private slots:
    void ping();

private:
    void setReachable(bool v);
    void setBusy(bool v);

    QDBusInterface* m_iface = nullptr;
    QTimer m_pingTimer;
    bool m_reachable = false;
    bool m_busy = false;
};
