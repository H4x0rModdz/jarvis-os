#include "power_bridge.h"

#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusReply>
#include <QLoggingCategory>
#include <QProcess>
#include <QVariant>

namespace {
Q_LOGGING_CATEGORY(lcPower, "jarvis.shell.power")

constexpr const char* kService = "org.freedesktop.UPower";
constexpr const char* kDevicePath = "/org/freedesktop/UPower/devices/DisplayDevice";
constexpr const char* kDeviceIface = "org.freedesktop.UPower.Device";
constexpr const char* kPropsIface = "org.freedesktop.DBus.Properties";

/// Map UPower's State enum to a human string.
/// 0 = Unknown, 1 = Charging, 2 = Discharging, 3 = Empty,
/// 4 = Fully charged, 5 = Pending charge, 6 = Pending discharge.
QString stateLabel(uint state)
{
    switch (state) {
        case 1: return QStringLiteral("charging");
        case 2: return QStringLiteral("discharging");
        case 3: return QStringLiteral("empty");
        case 4: return QStringLiteral("full");
        case 5: return QStringLiteral("pending-charge");
        case 6: return QStringLiteral("pending-discharge");
        default: return QStringLiteral("unknown");
    }
}
} // namespace

PowerBridge::PowerBridge(QObject* parent) : QObject(parent)
{
    auto bus = QDBusConnection::systemBus();
    if (!bus.isConnected()) {
        qCWarning(lcPower) << "System bus not connected — power state unavailable";
        return;
    }

    m_iface = new QDBusInterface(kService, kDevicePath, kDeviceIface, bus, this);

    // PropertiesChanged is the live update path. UPower fires it
    // every percentage point + on plug/unplug transitions.
    bus.connect(kService, kDevicePath, kPropsIface,
                QStringLiteral("PropertiesChanged"),
                this,
                SLOT(onPropertiesChanged(QString, QVariantMap, QStringList)));

    refreshAll();
}

void PowerBridge::refreshAll()
{
    if (!m_iface) return;

    // Type=2 means Battery in UPower's enum. Anything else (Line
    // Power, UPS, etc.) means "no laptop battery to display" for
    // our purposes — hide the indicator and skip the rest.
    QDBusReply<QVariant> typeReply = m_iface->call(
        QStringLiteral("Get"),
        QString::fromLatin1(kDeviceIface),
        QStringLiteral("Type"));
    const bool isBattery = typeReply.isValid()
        && typeReply.value().toUInt() == 2;
    if (isBattery != m_hasBattery) {
        m_hasBattery = isBattery;
        // Continue so we still pick up the rest of the snapshot.
    }
    if (!m_hasBattery) {
        emit stateChanged();
        return;
    }

    auto getDouble = [this](const char* prop) -> double {
        QDBusReply<QVariant> r = m_iface->call(
            QStringLiteral("Get"),
            QString::fromLatin1(kDeviceIface),
            QString::fromUtf8(prop));
        return r.isValid() ? r.value().toDouble() : 0.0;
    };
    auto getUInt = [this](const char* prop) -> uint {
        QDBusReply<QVariant> r = m_iface->call(
            QStringLiteral("Get"),
            QString::fromLatin1(kDeviceIface),
            QString::fromUtf8(prop));
        return r.isValid() ? r.value().toUInt() : 0u;
    };
    auto getInt64 = [this](const char* prop) -> qint64 {
        QDBusReply<QVariant> r = m_iface->call(
            QStringLiteral("Get"),
            QString::fromLatin1(kDeviceIface),
            QString::fromUtf8(prop));
        return r.isValid() ? r.value().toLongLong() : 0;
    };
    auto getString = [this](const char* prop) -> QString {
        QDBusReply<QVariant> r = m_iface->call(
            QStringLiteral("Get"),
            QString::fromLatin1(kDeviceIface),
            QString::fromUtf8(prop));
        return r.isValid() ? r.value().toString() : QString();
    };

    m_percentage = getDouble("Percentage");
    m_state = stateLabel(getUInt("State"));
    // TimeToEmpty matters when discharging; TimeToFull when charging.
    // Pick whichever is relevant + fall back to 0 when UPower hasn't
    // computed an estimate yet.
    const qint64 toEmpty = getInt64("TimeToEmpty");
    const qint64 toFull = getInt64("TimeToFull");
    m_timeRemaining = static_cast<int>(charging() ? toFull : toEmpty);
    m_iconName = getString("IconName");

    emit stateChanged();
    evaluateLowBattery();
}

void PowerBridge::onPropertiesChanged(const QString& iface,
                                     const QVariantMap& changed,
                                     const QStringList& /*invalidated*/)
{
    if (iface != QString::fromLatin1(kDeviceIface)) return;

    bool anyChanged = false;

    if (changed.contains(QStringLiteral("Percentage"))) {
        m_percentage = changed.value(QStringLiteral("Percentage")).toDouble();
        anyChanged = true;
    }
    if (changed.contains(QStringLiteral("State"))) {
        const uint s = changed.value(QStringLiteral("State")).toUInt();
        m_state = stateLabel(s);
        anyChanged = true;
    }
    if (changed.contains(QStringLiteral("TimeToEmpty"))
            || changed.contains(QStringLiteral("TimeToFull"))) {
        const qint64 toEmpty = changed.contains(QStringLiteral("TimeToEmpty"))
            ? changed.value(QStringLiteral("TimeToEmpty")).toLongLong()
            : m_timeRemaining;
        const qint64 toFull = changed.contains(QStringLiteral("TimeToFull"))
            ? changed.value(QStringLiteral("TimeToFull")).toLongLong()
            : m_timeRemaining;
        m_timeRemaining = static_cast<int>(charging() ? toFull : toEmpty);
        anyChanged = true;
    }
    if (changed.contains(QStringLiteral("IconName"))) {
        m_iconName = changed.value(QStringLiteral("IconName")).toString();
        anyChanged = true;
    }

    if (anyChanged) {
        emit stateChanged();
        evaluateLowBattery();
    }
}

void PowerBridge::evaluateLowBattery()
{
    if (!m_hasBattery) return;
    if (m_state != QStringLiteral("discharging")) {
        // Reset the warning state on plug-in so the next discharge
        // session can warn again.
        m_warnedAt = 100;
        return;
    }
    // Two thresholds: 15% (heads-up) and 5% (critical). Fire one
    // notification per threshold per discharge session.
    if (m_percentage <= 5.0 && m_warnedAt > 5) {
        notifyLow(5);
        m_warnedAt = 5;
    } else if (m_percentage <= 15.0 && m_warnedAt > 15) {
        notifyLow(15);
        m_warnedAt = 15;
    }
}

void PowerBridge::notifyLow(int threshold)
{
    // Shell out to notify-send rather than driving the DBus call
    // ourselves — keeps the body small and the toast goes through
    // the same notifications daemon every other system message
    // uses. replaces_id=1004 so successive thresholds (15 → 5)
    // overwrite the heads-up in place.
    QStringList args;
    args << QStringLiteral("--replace-id=1004")
         << QStringLiteral("--expire-time=8000");
    if (threshold == 5) {
        args << QStringLiteral("--icon=battery-caution")
             << QStringLiteral("--urgency=critical")
             << QStringLiteral("Bateria crítica")
             << QStringLiteral("Bateria em %1%. Conecte o carregador.")
                .arg(static_cast<int>(m_percentage));
    } else {
        args << QStringLiteral("--icon=battery-low")
             << QStringLiteral("Bateria fraca")
             << QStringLiteral("Bateria em %1%.")
                .arg(static_cast<int>(m_percentage));
    }
    QProcess::startDetached(QStringLiteral("notify-send"), args);
}
