#pragma once

#include <QObject>
#include <QSettings>
#include <QString>
#include <qqmlintegration.h>

/// Persistent slice of greeter state — last username + last mode the
/// user logged in with. Stored under
/// `~/.config/Jarvis/jarvis-greeter.conf` (QSettings INI format) so
/// the next boot remembers the user's preferred entry point.
///
/// V1.5 polish: keeps the greeter from feeling generic — "Welcome
/// back, Lucas" + already-selected Lilith mode reads as personal,
/// not a fresh form every time.
class GreeterState : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    Q_PROPERTY(QString username READ username WRITE setUsername NOTIFY changed)
    Q_PROPERTY(int modeIndex READ modeIndex WRITE setModeIndex NOTIFY changed)

public:
    explicit GreeterState(QObject* parent = nullptr);

    QString username() const { return m_username; }
    int modeIndex() const { return m_modeIndex; }

    void setUsername(const QString& v);
    void setModeIndex(int v);

    /// Persist current values to disk. Called by QML right before
    /// submitting auth so the next boot picks up where this one
    /// left off — even if auth ends up failing, the chosen mode +
    /// typed username are remembered for the retry.
    Q_INVOKABLE void persist();

signals:
    void changed();

private:
    QSettings m_settings;
    QString m_username;
    int m_modeIndex = 0;
};
