#pragma once

#include <QObject>
#include <QString>
#include <QTimer>
#include <QVariantList>
#include <QVariantMap>
#include <qqmlintegration.h>

/// Bridge to NetworkManager via the `nmcli` subprocess.
///
/// Why nmcli instead of zbus directly: NetworkManager's DBus surface is
/// large and requires a polkit prompt for write operations (toggles,
/// connect). nmcli handles polkit + the secret-agent dance for us, so
/// wrapping it as a subprocess is ~10x less code than re-implementing
/// the protocol. The price is one fork+exec per refresh; for a 5 s
/// poll cycle that's invisible.
///
/// Exposes:
///   - `wifiEnabled` — radio state, set via setWifiEnabled
///   - `activeConnection` — `{ssid, signal, security}` or empty when offline
///   - `availableNetworks` — list of `{ssid, signal, security, in_use}`
///   - `scan()` — kick a scan; auto-refresh on completion
///   - `connectTo(ssid, password)` — `connectTo` because `connect` clashes with QObject
///   - `disconnect()`
///
/// V1: Wi-Fi only. Ethernet / Bluetooth / cellular are NetworkManager
/// concepts too; future phases extend the bridge.
class NetworkBridge : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    Q_PROPERTY(bool wifiEnabled READ wifiEnabled NOTIFY wifiEnabledChanged)
    Q_PROPERTY(QVariantMap activeConnection READ activeConnection NOTIFY activeConnectionChanged)
    Q_PROPERTY(QVariantList availableNetworks READ availableNetworks NOTIFY availableNetworksChanged)
    Q_PROPERTY(QString lastError READ lastError NOTIFY lastErrorChanged)
    Q_PROPERTY(bool busy READ busy NOTIFY busyChanged)

public:
    explicit NetworkBridge(QObject* parent = nullptr);

    bool wifiEnabled() const { return m_wifiEnabled; }
    QVariantMap activeConnection() const { return m_activeConnection; }
    QVariantList availableNetworks() const { return m_availableNetworks; }
    QString lastError() const { return m_lastError; }
    bool busy() const { return m_busy; }

    /// Trigger a Wi-Fi scan + refresh. Idempotent.
    Q_INVOKABLE void scan();

    /// Connect to `ssid`. Pass an empty `password` for open networks;
    /// nmcli will refuse if the network needs one. New connection is
    /// saved as a profile so subsequent boots reconnect automatically.
    Q_INVOKABLE void connectTo(const QString& ssid, const QString& password);

    /// Drop the active Wi-Fi connection (radio stays on).
    Q_INVOKABLE void disconnectWifi();

    /// Toggle the Wi-Fi radio.
    Q_INVOKABLE void setWifiEnabled(bool enabled);

    /// Start the periodic refresh loop. Called by the panel on open
    /// so we're not running nmcli every 5 s when the user isn't
    /// looking.
    Q_INVOKABLE void startPolling();

    /// Stop the periodic refresh loop. Called on panel close.
    Q_INVOKABLE void stopPolling();

signals:
    void wifiEnabledChanged();
    void activeConnectionChanged();
    void availableNetworksChanged();
    void lastErrorChanged();
    void busyChanged();

private slots:
    void poll();

private:
    void runNmcli(const QStringList& args,
                  std::function<void(int, const QString&, const QString&)> on_done);
    void refreshAll();
    void parseWifiList(const QString& output);
    void parseActiveConnection(const QString& output);
    void parseWifiEnabled(const QString& output);
    void setError(const QString& msg);
    void setBusy(bool v);

    QTimer m_pollTimer;
    bool m_wifiEnabled = false;
    QVariantMap m_activeConnection;
    QVariantList m_availableNetworks;
    QString m_lastError;
    bool m_busy = false;
};
