#include "network_bridge.h"

#include <QLoggingCategory>
#include <QProcess>

namespace {
Q_LOGGING_CATEGORY(lcNet, "jarvis.shell.network")

/// `nmcli -t -f <FIELDS>` produces colon-separated output with
/// escaping (`\:` and `\\`). Real SSIDs with colons in them get
/// escaped this way, so the parser has to honour the escape rules
/// rather than naive split.
QStringList splitNmcliRow(const QString& line)
{
    QStringList fields;
    QString current;
    for (int i = 0; i < line.size(); ++i) {
        const QChar c = line[i];
        if (c == QChar('\\') && i + 1 < line.size()) {
            current.append(line[++i]);
            continue;
        }
        if (c == QChar(':')) {
            fields.append(current);
            current.clear();
            continue;
        }
        current.append(c);
    }
    fields.append(current);
    return fields;
}
} // namespace

NetworkBridge::NetworkBridge(QObject* parent) : QObject(parent)
{
    // 5 s is fast enough to feel live without burning CPU when the
    // panel happens to stay open. Off until startPolling() — we
    // don't run nmcli every 5 s for every shell instance just to
    // power a bar icon.
    m_pollTimer.setInterval(5000);
    QObject::connect(&m_pollTimer, &QTimer::timeout, this, &NetworkBridge::poll);

    // One initial probe so the bar icon has something to show on
    // startup, before any panel opens.
    refreshAll();
}

void NetworkBridge::startPolling()
{
    if (m_pollTimer.isActive()) return;
    refreshAll();
    m_pollTimer.start();
}

void NetworkBridge::stopPolling()
{
    m_pollTimer.stop();
}

void NetworkBridge::poll()
{
    refreshAll();
}

void NetworkBridge::refreshAll()
{
    // Radio state first — the other two queries are no-ops when off.
    runNmcli({"-t", "radio", "wifi"},
        [this](int code, const QString& out, const QString& err) {
            if (code != 0) {
                setError(err.trimmed());
                return;
            }
            parseWifiEnabled(out);
        });
    runNmcli({"-t", "-f", "NAME,TYPE,DEVICE", "connection", "show", "--active"},
        [this](int code, const QString& out, const QString&) {
            if (code != 0) {
                // Don't surface — empty active connection is normal.
                m_activeConnection.clear();
                emit activeConnectionChanged();
                return;
            }
            parseActiveConnection(out);
        });
    runNmcli({"-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY", "device", "wifi", "list"},
        [this](int code, const QString& out, const QString& err) {
            if (code != 0) {
                setError(err.trimmed());
                return;
            }
            parseWifiList(out);
        });
}

void NetworkBridge::scan()
{
    setBusy(true);
    // `--rescan yes` forces a fresh scan rather than returning the
    // cached list. Slower (~3 s) but the panel is the one place
    // the user expects fresh data.
    runNmcli({"-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY", "device", "wifi", "list", "--rescan", "yes"},
        [this](int code, const QString& out, const QString& err) {
            setBusy(false);
            if (code != 0) {
                setError(err.trimmed());
                return;
            }
            parseWifiList(out);
        });
}

void NetworkBridge::connectTo(const QString& ssid, const QString& password)
{
    setBusy(true);
    QStringList args = {"device", "wifi", "connect", ssid};
    if (!password.isEmpty()) {
        args << "password" << password;
    }
    runNmcli(args, [this](int code, const QString&, const QString& err) {
        setBusy(false);
        if (code != 0) {
            setError(err.trimmed().isEmpty() ? tr("Falha ao conectar") : err.trimmed());
            return;
        }
        setError({});
        refreshAll();
    });
}

void NetworkBridge::disconnectWifi()
{
    setBusy(true);
    // Disconnect via the device name nmcli stored in the active
    // connection map; if we don't have one, no-op.
    const QString device = m_activeConnection
        .value(QStringLiteral("device"))
        .toString();
    if (device.isEmpty()) {
        setBusy(false);
        return;
    }
    runNmcli({"device", "disconnect", device},
        [this](int code, const QString&, const QString& err) {
            setBusy(false);
            if (code != 0) {
                setError(err.trimmed());
                return;
            }
            refreshAll();
        });
}

void NetworkBridge::setWifiEnabled(bool enabled)
{
    runNmcli({"radio", "wifi", enabled ? "on" : "off"},
        [this](int code, const QString&, const QString& err) {
            if (code != 0) {
                setError(err.trimmed());
                return;
            }
            refreshAll();
        });
}

void NetworkBridge::runNmcli(const QStringList& args,
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
    proc->start(QStringLiteral("nmcli"), args);
}

void NetworkBridge::parseWifiEnabled(const QString& output)
{
    const QString state = output.trimmed().toLower();
    // nmcli prints "enabled" or "disabled".
    const bool enabled = state == QStringLiteral("enabled");
    if (enabled == m_wifiEnabled) return;
    m_wifiEnabled = enabled;
    emit wifiEnabledChanged();
}

void NetworkBridge::parseActiveConnection(const QString& output)
{
    QVariantMap active;
    for (const QString& line : output.split('\n', Qt::SkipEmptyParts)) {
        const QStringList fields = splitNmcliRow(line);
        if (fields.size() < 3) continue;
        const QString& type = fields[1];
        // Only Wi-Fi connections for V1; future phases handle ethernet etc.
        if (type != QStringLiteral("802-11-wireless")
            && type != QStringLiteral("wifi")) {
            continue;
        }
        active.insert(QStringLiteral("ssid"), fields[0]);
        active.insert(QStringLiteral("device"), fields[2]);
        break;
    }
    if (active != m_activeConnection) {
        m_activeConnection = active;
        emit activeConnectionChanged();
    }
}

void NetworkBridge::parseWifiList(const QString& output)
{
    QVariantList nets;
    int active_signal = 0;
    QString active_ssid;
    QString active_security;
    for (const QString& line : output.split('\n', Qt::SkipEmptyParts)) {
        const QStringList fields = splitNmcliRow(line);
        // Expect 4 fields: IN-USE, SSID, SIGNAL, SECURITY.
        if (fields.size() < 4) continue;
        const bool in_use = fields[0] == QStringLiteral("*");
        const QString ssid = fields[1];
        // Skip hidden / empty SSIDs — nothing useful to show the user.
        if (ssid.trimmed().isEmpty()) continue;
        const int signal = fields[2].toInt();
        const QString security = fields[3];

        QVariantMap net;
        net.insert(QStringLiteral("ssid"), ssid);
        net.insert(QStringLiteral("signal"), signal);
        net.insert(QStringLiteral("security"), security);
        net.insert(QStringLiteral("in_use"), in_use);
        nets.append(net);

        if (in_use) {
            active_signal = signal;
            active_ssid = ssid;
            active_security = security;
        }
    }
    if (nets != m_availableNetworks) {
        m_availableNetworks = nets;
        emit availableNetworksChanged();
    }
    // Patch active connection with the signal pulled from the scan
    // — nmcli connection show doesn't include signal.
    if (!active_ssid.isEmpty()
        && m_activeConnection.value(QStringLiteral("ssid")).toString() == active_ssid) {
        const int prev = m_activeConnection.value(QStringLiteral("signal")).toInt();
        if (prev != active_signal
            || m_activeConnection.value(QStringLiteral("security")).toString() != active_security) {
            m_activeConnection.insert(QStringLiteral("signal"), active_signal);
            m_activeConnection.insert(QStringLiteral("security"), active_security);
            emit activeConnectionChanged();
        }
    }
}

void NetworkBridge::setError(const QString& msg)
{
    if (msg == m_lastError) return;
    m_lastError = msg;
    emit lastErrorChanged();
    if (!msg.isEmpty()) {
        qCWarning(lcNet) << "nmcli error:" << msg;
    }
}

void NetworkBridge::setBusy(bool v)
{
    if (m_busy == v) return;
    m_busy = v;
    emit busyChanged();
}
