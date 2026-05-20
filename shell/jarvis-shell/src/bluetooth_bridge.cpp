#include "bluetooth_bridge.h"

#include <QLoggingCategory>
#include <QProcess>
#include <QRegularExpression>
#include <QSet>
#include <QVariantMap>

namespace {
Q_LOGGING_CATEGORY(lcBt, "jarvis.shell.bluetooth")

/// Parse one line of `bluetoothctl devices` output:
///   "Device AA:BB:CC:DD:EE:FF Device Name Here"
/// Returns {mac, name} or empty on malformed input.
QVariantMap parseDeviceLine(const QString& line)
{
    static const QRegularExpression re(
        QStringLiteral("^Device\\s+([0-9A-Fa-f:]{17})\\s+(.+)$"));
    const auto match = re.match(line.trimmed());
    if (!match.hasMatch()) return {};
    QVariantMap m;
    m.insert(QStringLiteral("mac"), match.captured(1).toUpper());
    m.insert(QStringLiteral("name"), match.captured(2));
    return m;
}
} // namespace

BluetoothBridge::BluetoothBridge(QObject* parent) : QObject(parent)
{
    // 5 s refresh while the panel is open. Off until startPolling().
    m_pollTimer.setInterval(5000);
    QObject::connect(&m_pollTimer, &QTimer::timeout, this, &BluetoothBridge::poll);
    // One initial probe so the bar icon has something to show even
    // before any panel opens.
    refreshAll();
}

void BluetoothBridge::startPolling()
{
    if (m_pollTimer.isActive()) return;
    refreshAll();
    m_pollTimer.start();
}

void BluetoothBridge::stopPolling()
{
    m_pollTimer.stop();
}

void BluetoothBridge::poll()
{
    refreshAll();
}

void BluetoothBridge::refreshAll()
{
    refreshPower();
    refreshDevices();
}

void BluetoothBridge::refreshPower()
{
    runBtCtl({"show"}, [this](int code, const QString& out, const QString&) {
        if (code != 0) return;
        // `show` prints "\tPowered: yes" / "\tPowered: no" among
        // other lines; pick that one.
        bool on = false;
        for (const QString& line : out.split('\n', Qt::SkipEmptyParts)) {
            const QString t = line.trimmed();
            if (t.startsWith(QStringLiteral("Powered: "))) {
                on = t.endsWith(QStringLiteral("yes"));
                break;
            }
        }
        if (on != m_poweredOn) {
            m_poweredOn = on;
            emit poweredOnChanged();
        }
    });
}

void BluetoothBridge::refreshDevices()
{
    // Paired list first. `devices Paired` returns "Device <mac> <name>"
    // lines (bluez 5.66+); older bluez uses `paired-devices`. Try the
    // newer command and fall back on non-zero exit.
    runBtCtl({"devices", "Paired"},
        [this](int code, const QString& out, const QString&) {
            if (code != 0) {
                // Older bluez fallback path.
                runBtCtl({"paired-devices"},
                    [this](int, const QString& out2, const QString&) {
                        QVariantList paired;
                        for (const QString& line : out2.split('\n', Qt::SkipEmptyParts)) {
                            const QVariantMap dev = parseDeviceLine(line);
                            if (!dev.isEmpty()) paired.append(dev);
                        }
                        if (paired != m_pairedDevices) {
                            m_pairedDevices = paired;
                            emit pairedDevicesChanged();
                        }
                    });
                return;
            }
            QVariantList paired;
            for (const QString& line : out.split('\n', Qt::SkipEmptyParts)) {
                const QVariantMap dev = parseDeviceLine(line);
                if (!dev.isEmpty()) paired.append(dev);
            }
            // Augment each entry with connection state from `info`.
            // bluetoothctl doesn't expose connected state from
            // `devices` alone; the panel needs it to render
            // connect/disconnect buttons.
            for (auto& v : paired) {
                QVariantMap m = v.toMap();
                const QString mac = m.value(QStringLiteral("mac")).toString();
                m.insert(QStringLiteral("connected"), false);
                v = m;
            }
            // Fire a probe per paired device for Connected state.
            // Each callback updates the matching entry; we emit once
            // all responses have returned (tracked via a shared
            // counter held in a heap-allocated QSharedPointer).
            QSharedPointer<int> pending(new int(paired.size()));
            QSharedPointer<QVariantList> resultPtr(new QVariantList(paired));
            if (paired.isEmpty()) {
                m_pairedDevices = paired;
                emit pairedDevicesChanged();
                return;
            }
            for (int i = 0; i < paired.size(); ++i) {
                const QString mac = paired[i].toMap()
                    .value(QStringLiteral("mac")).toString();
                runBtCtl({"info", mac},
                    [this, pending, resultPtr, i](int /*code*/,
                                                   const QString& out,
                                                   const QString&) {
                        bool connected = false;
                        for (const QString& line : out.split('\n', Qt::SkipEmptyParts)) {
                            const QString t = line.trimmed();
                            if (t.startsWith(QStringLiteral("Connected: yes"))) {
                                connected = true;
                                break;
                            }
                        }
                        QVariantMap m = (*resultPtr)[i].toMap();
                        m.insert(QStringLiteral("connected"), connected);
                        (*resultPtr)[i] = m;
                        if (--(*pending) == 0) {
                            if (*resultPtr != m_pairedDevices) {
                                m_pairedDevices = *resultPtr;
                                emit pairedDevicesChanged();
                            }
                        }
                    });
            }
        });

    // Nearby (all known minus paired). Useful when the user just
    // ran scan() and we want to surface what showed up.
    runBtCtl({"devices"}, [this](int code, const QString& out, const QString&) {
        if (code != 0) return;
        QSet<QString> pairedMacs;
        for (const auto& v : m_pairedDevices) {
            pairedMacs.insert(v.toMap().value(QStringLiteral("mac")).toString());
        }
        QVariantList nearby;
        for (const QString& line : out.split('\n', Qt::SkipEmptyParts)) {
            const QVariantMap dev = parseDeviceLine(line);
            if (dev.isEmpty()) continue;
            const QString mac = dev.value(QStringLiteral("mac")).toString();
            if (pairedMacs.contains(mac)) continue;
            nearby.append(dev);
        }
        if (nearby != m_nearbyDevices) {
            m_nearbyDevices = nearby;
            emit nearbyDevicesChanged();
        }
    });
}

void BluetoothBridge::setPowered(bool on)
{
    runBtCtl({"power", on ? "on" : "off"},
        [this](int code, const QString&, const QString& err) {
            if (code != 0) setError(err.trimmed());
            refreshPower();
        });
}

void BluetoothBridge::scan()
{
    setScanning(true);
    // `--timeout` puts bluetoothctl into a fixed-duration discovery
    // window then returns. 10 s is roughly the time it takes for
    // most devices to announce themselves.
    runBtCtl({"--timeout", "10", "scan", "on"},
        [this](int code, const QString&, const QString& err) {
            setScanning(false);
            if (code != 0) setError(err.trimmed());
            refreshDevices();
        });
}

void BluetoothBridge::pair(const QString& mac)
{
    setBusy(true);
    runBtCtl({"pair", mac},
        [this, mac](int code, const QString&, const QString& err) {
            setBusy(false);
            if (code != 0) {
                setError(err.trimmed().isEmpty() ? tr("Falha ao parear") : err.trimmed());
                return;
            }
            // Trust + connect right after pair so it auto-reconnects
            // on next boot — matches what bluetoothctl's interactive
            // session does by default.
            runBtCtl({"trust", mac},
                [this, mac](int, const QString&, const QString&) {
                    runBtCtl({"connect", mac},
                        [this](int code, const QString&, const QString& err) {
                            if (code != 0) setError(err.trimmed());
                            refreshDevices();
                        });
                });
        });
}

void BluetoothBridge::unpair(const QString& mac)
{
    setBusy(true);
    runBtCtl({"remove", mac},
        [this](int code, const QString&, const QString& err) {
            setBusy(false);
            if (code != 0) setError(err.trimmed());
            refreshDevices();
        });
}

void BluetoothBridge::connectDevice(const QString& mac)
{
    setBusy(true);
    runBtCtl({"connect", mac},
        [this](int code, const QString&, const QString& err) {
            setBusy(false);
            if (code != 0) setError(err.trimmed());
            refreshDevices();
        });
}

void BluetoothBridge::disconnectDevice(const QString& mac)
{
    setBusy(true);
    runBtCtl({"disconnect", mac},
        [this](int code, const QString&, const QString& err) {
            setBusy(false);
            if (code != 0) setError(err.trimmed());
            refreshDevices();
        });
}

void BluetoothBridge::runBtCtl(const QStringList& args,
                              std::function<void(int, const QString&, const QString&)> on_done)
{
    auto* proc = new QProcess(this);
    QObject::connect(
        proc,
        QOverload<int, QProcess::ExitStatus>::of(&QProcess::finished),
        this,
        [proc, on_done](int code, QProcess::ExitStatus) {
            const QString out = QString::fromUtf8(proc->readAllStandardOutput());
            const QString err = QString::fromUtf8(proc->readAllStandardError());
            on_done(code, out, err);
            proc->deleteLater();
        });
    QObject::connect(proc, &QProcess::errorOccurred, this,
        [proc, on_done](QProcess::ProcessError) {
            on_done(-1, {}, proc->errorString());
            proc->deleteLater();
        });
    proc->start(QStringLiteral("bluetoothctl"), args);
}

void BluetoothBridge::setError(const QString& msg)
{
    if (msg == m_lastError) return;
    m_lastError = msg;
    emit lastErrorChanged();
    if (!msg.isEmpty()) {
        qCWarning(lcBt) << "bluetoothctl error:" << msg;
    }
}

void BluetoothBridge::setBusy(bool v)
{
    if (m_busy == v) return;
    m_busy = v;
    emit busyChanged();
}

void BluetoothBridge::setScanning(bool v)
{
    if (m_scanning == v) return;
    m_scanning = v;
    emit scanningChanged();
}
