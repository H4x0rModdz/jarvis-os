#pragma once

#include <QDBusInterface>
#include <QObject>
#include <QString>
#include <qqmlintegration.h>

/// Bridge between QML and `com.jarvis.Lock`. The window only needs
/// one path: send the typed password, get back ok/reason. On
/// success the daemon emits LockStateChanged(false), at which
/// point we quit and let the daemon clean up. On failure we surface
/// the reason and let the user try again.
class LockClient : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    Q_PROPERTY(QString state READ state NOTIFY stateChanged)
    Q_PROPERTY(QString error READ error NOTIFY stateChanged)

public:
    explicit LockClient(QObject* parent = nullptr);

    QString state() const { return m_state; }
    QString error() const { return m_error; }

    Q_INVOKABLE void verify(const QString& password);

signals:
    void stateChanged();

private slots:
    void onLockStateChanged(bool locked);

private:
    void setState(const QString& state, const QString& error = {});

    QDBusInterface* m_iface = nullptr;
    QString m_state = QStringLiteral("idle");
    QString m_error;
};
