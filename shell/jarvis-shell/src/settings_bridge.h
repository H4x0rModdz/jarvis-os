#pragma once

#include <QDBusInterface>
#include <QObject>
#include <QString>
#include <QVariant>
#include <qqmlintegration.h>

/// Bridge between QML and `com.jarvis.Settings`.
///
/// Reads + writes settings synchronously via Q_INVOKABLE getters/setters
/// so QML bindings can express "give me this preference, falling back to
/// this default" in a single line. Subscribes to the daemon's `Changed`
/// signal and re-emits a generic `valueChanged(key)` so QML can re-resolve
/// without polling.
///
/// Values are JSON documents on the wire; the typed helpers parse them
/// into the matching Qt type. Unknown / missing keys silently fall back
/// to the default — keeping callers from littering every binding with
/// `key in settings ? settings[key] : fallback`.
class SettingsBridge : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    Q_PROPERTY(bool reachable READ reachable NOTIFY reachableChanged)

public:
    explicit SettingsBridge(QObject* parent = nullptr);

    bool reachable() const { return m_reachable; }

    Q_INVOKABLE QString getString(const QString& key, const QString& defaultValue) const;
    Q_INVOKABLE bool getBool(const QString& key, bool defaultValue) const;
    Q_INVOKABLE double getNumber(const QString& key, double defaultValue) const;

    Q_INVOKABLE void setString(const QString& key, const QString& value);
    Q_INVOKABLE void setBool(const QString& key, bool value);
    Q_INVOKABLE void setNumber(const QString& key, double value);

signals:
    void reachableChanged();
    /// Fires for every successful Set or Delete (server-side `Changed`),
    /// plus on local writes once the daemon ack'd.
    void valueChanged(const QString& key);

private slots:
    void onChanged(const QString& key, const QString& valueJson);

private:
    void setReachable(bool v);
    QVariant fetchRaw(const QString& key) const;
    void writeRaw(const QString& key, const QString& valueJson);

    QDBusInterface* m_iface = nullptr;
    bool m_reachable = false;
};
