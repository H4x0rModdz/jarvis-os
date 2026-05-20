#pragma once

#include <QObject>
#include <QString>
#include <QStringList>
#include <QTimer>
#include <QVariantList>
#include <qqmlintegration.h>

/// Bridge to BlueZ via the `bluetoothctl` CLI. Same shape as
/// NetworkBridge — wrapping the official CLI in one-shot mode is
/// less code than driving BlueZ's DBus API directly (which has six
/// interfaces + a pairing agent + object-manager lookups).
///
/// V1 supports "just works" pairing only — headphones, mice,
/// keyboards from the last decade pair without a PIN. Devices that
/// need a numeric passkey confirmation prompt come in V2 (would
/// need to register a BlueZ agent in-process).
///
/// Q_PROPERTYs:
///   - poweredOn         — radio state
///   - pairedDevices     — `[{mac, name, connected, icon, trusted}, …]`
///   - nearbyDevices     — devices discovered during the last scan
///                          and not already paired
///   - scanning          — true while the discovery window is open
///   - busy              — any subprocess in flight
///   - lastError         — most recent bluetoothctl stderr line
class BluetoothBridge : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    Q_PROPERTY(bool poweredOn READ poweredOn NOTIFY poweredOnChanged)
    Q_PROPERTY(QVariantList pairedDevices READ pairedDevices NOTIFY pairedDevicesChanged)
    Q_PROPERTY(QVariantList nearbyDevices READ nearbyDevices NOTIFY nearbyDevicesChanged)
    Q_PROPERTY(bool scanning READ scanning NOTIFY scanningChanged)
    Q_PROPERTY(bool busy READ busy NOTIFY busyChanged)
    Q_PROPERTY(QString lastError READ lastError NOTIFY lastErrorChanged)

public:
    explicit BluetoothBridge(QObject* parent = nullptr);

    bool poweredOn() const { return m_poweredOn; }
    QVariantList pairedDevices() const { return m_pairedDevices; }
    QVariantList nearbyDevices() const { return m_nearbyDevices; }
    bool scanning() const { return m_scanning; }
    bool busy() const { return m_busy; }
    QString lastError() const { return m_lastError; }

    /// Toggle the BT radio. `bluetoothctl power on/off`.
    Q_INVOKABLE void setPowered(bool on);

    /// Start a 10 s discovery window. Auto-refreshes nearby + paired
    /// lists at the end.
    Q_INVOKABLE void scan();

    /// Just-works pairing. Calls `bluetoothctl pair <mac>`. If the
    /// device requires a numeric PIN this returns an error in
    /// lastError — V1 doesn't drive the agent's prompt.
    Q_INVOKABLE void pair(const QString& mac);

    Q_INVOKABLE void unpair(const QString& mac);

    Q_INVOKABLE void connectDevice(const QString& mac);
    Q_INVOKABLE void disconnectDevice(const QString& mac);

    /// Start the periodic refresh loop. Panel calls this on open.
    Q_INVOKABLE void startPolling();

    /// Stop the periodic refresh loop. Panel calls this on close.
    Q_INVOKABLE void stopPolling();

signals:
    void poweredOnChanged();
    void pairedDevicesChanged();
    void nearbyDevicesChanged();
    void scanningChanged();
    void busyChanged();
    void lastErrorChanged();

private slots:
    void poll();

private:
    void runBtCtl(const QStringList& args,
                  std::function<void(int, const QString&, const QString&)> on_done);
    void refreshAll();
    void refreshPower();
    void refreshDevices();
    void setError(const QString& msg);
    void setBusy(bool v);
    void setScanning(bool v);

    QTimer m_pollTimer;
    bool m_poweredOn = false;
    QVariantList m_pairedDevices;
    QVariantList m_nearbyDevices;
    bool m_scanning = false;
    bool m_busy = false;
    QString m_lastError;
};
