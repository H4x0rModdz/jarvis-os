#include "settings_bridge.h"

#include <QDBusConnection>
#include <QDBusPendingCall>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonValue>
#include <QLoggingCategory>

namespace {
Q_LOGGING_CATEGORY(lcSet, "jarvis.shell.settings")

constexpr const char* kService = "com.jarvis.Settings";
constexpr const char* kPath = "/com/jarvis/Settings";
constexpr const char* kIface = "com.jarvis.Settings";
}

SettingsBridge::SettingsBridge(QObject* parent) : QObject(parent)
{
    auto bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        qCWarning(lcSet) << "Session bus not connected";
        return;
    }

    m_iface = new QDBusInterface(kService, kPath, kIface, bus, this);

    const bool changedOk = bus.connect(
        kService, kPath, kIface,
        QStringLiteral("Changed"),
        this,
        SLOT(onChanged(QString, QString)));
    if (!changedOk) {
        qCWarning(lcSet) << "Failed to subscribe to Settings.Changed";
    }

    // Probe reachability — Settings is Requires= in the session target,
    // so a failed call here means something has actually gone wrong.
    auto pending = m_iface->asyncCall(QStringLiteral("List"));
    auto* watcher = new QDBusPendingCallWatcher(pending, this);
    QObject::connect(watcher, &QDBusPendingCallWatcher::finished, this,
        [this](QDBusPendingCallWatcher* w) {
            QDBusPendingReply<QString> reply = *w;
            setReachable(!reply.isError());
            w->deleteLater();
        });
}

QVariant SettingsBridge::fetchRaw(const QString& key) const
{
    if (!m_iface) return QVariant();

    // Synchronous DBus call. We accept the latency because:
    //   - Settings is local, fast (SQLite WAL read = µs);
    //   - QML bindings have no equivalent async-then-return idiom;
    //   - callers always pass a default, so even a slow daemon doesn't
    //     wedge the UI thread for long.
    QDBusReply<QString> reply = m_iface->call(QStringLiteral("Get"), key);
    if (!reply.isValid()) {
        qCWarning(lcSet) << "Get" << key << "failed:" << reply.error().message();
        return QVariant();
    }
    const auto doc = QJsonDocument::fromJson(reply.value().toUtf8());
    if (!doc.isObject()) return QVariant();
    const auto obj = doc.object();
    if (!obj.value(QStringLiteral("found")).toBool()) return QVariant();
    return obj.value(QStringLiteral("value")).toVariant();
}

void SettingsBridge::writeRaw(const QString& key, const QString& valueJson)
{
    if (!m_iface) return;
    auto pending = m_iface->asyncCall(QStringLiteral("Set"), key, valueJson);
    auto* watcher = new QDBusPendingCallWatcher(pending, this);
    QObject::connect(watcher, &QDBusPendingCallWatcher::finished, this,
        [this, key](QDBusPendingCallWatcher* w) {
            QDBusPendingReply<QString> reply = *w;
            if (reply.isError()) {
                qCWarning(lcSet) << "Set" << key << "failed:" << reply.error().message();
            } else {
                qCInfo(lcSet) << "Set" << key << "ok";
                // We'll also see this via the Changed signal, but bindings
                // shouldn't have to wait for the round trip — emit now.
                emit valueChanged(key);
            }
            w->deleteLater();
        });
}

QString SettingsBridge::getString(const QString& key, const QString& defaultValue) const
{
    const QVariant v = fetchRaw(key);
    if (v.isValid() && v.canConvert<QString>()) return v.toString();
    return defaultValue;
}

bool SettingsBridge::getBool(const QString& key, bool defaultValue) const
{
    const QVariant v = fetchRaw(key);
    if (v.isValid() && v.canConvert<bool>()) return v.toBool();
    return defaultValue;
}

double SettingsBridge::getNumber(const QString& key, double defaultValue) const
{
    const QVariant v = fetchRaw(key);
    if (v.isValid() && v.canConvert<double>()) return v.toDouble();
    return defaultValue;
}

void SettingsBridge::setString(const QString& key, const QString& value)
{
    // The Settings daemon stores values as JSON documents, so a string
    // setting needs to be quoted + escaped. QJsonDocument only serialises
    // arrays/objects, so we wrap the value in a one-element array, take
    // the doc as JSON, and strip the brackets. Quoting + escaping the
    // string by hand would be one more thing to get wrong.
    const QJsonDocument doc{ QJsonArray{ value } };
    const QByteArray arr = doc.toJson(QJsonDocument::Compact);  // ["foo"]
    writeRaw(key, QString::fromUtf8(arr.mid(1, arr.size() - 2)));
}

void SettingsBridge::setBool(const QString& key, bool value)
{
    writeRaw(key, value ? QStringLiteral("true") : QStringLiteral("false"));
}

void SettingsBridge::setNumber(const QString& key, double value)
{
    writeRaw(key, QString::number(value, 'g', 15));
}

void SettingsBridge::onChanged(const QString& key, const QString& /*valueJson*/)
{
    qCInfo(lcSet) << "Changed" << key;
    emit valueChanged(key);
}

void SettingsBridge::setReachable(bool v)
{
    if (m_reachable == v) return;
    m_reachable = v;
    emit reachableChanged();
}
