#include "updater_bridge.h"

#include <QDBusConnection>
#include <QLoggingCategory>

namespace {
Q_LOGGING_CATEGORY(lcUpd, "jarvis.shell.updater")

constexpr const char* kService = "com.jarvis.Updater";
constexpr const char* kPath = "/com/jarvis/Updater";
constexpr const char* kIface = "com.jarvis.Updater";
}

UpdaterBridge::UpdaterBridge(QObject* parent) : QObject(parent)
{
    auto bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        qCWarning(lcUpd) << "Session bus not connected";
        return;
    }

    m_iface = new QDBusInterface(kService, kPath, kIface, bus, this);

    // The updater may not be on the bus yet (depending on session start
    // order). We still subscribe — DBus delivers the signals when the
    // service appears.
    const bool progressOk = bus.connect(
        kService, kPath, kIface,
        QStringLiteral("Progress"),
        this,
        SLOT(onProgress(QString, int, QString)));
    const bool completedOk = bus.connect(
        kService, kPath, kIface,
        QStringLiteral("Completed"),
        this,
        SLOT(onCompleted(bool, QString)));
    const bool osUpdateOk = bus.connect(
        kService, kPath, kIface,
        QStringLiteral("OSUpdateAvailable"),
        this,
        SLOT(onOSUpdateAvailable(QString)));

    if (!progressOk || !completedOk || !osUpdateOk) {
        qCWarning(lcUpd) << "Failed to subscribe to updater signals"
                         << "progress=" << progressOk
                         << "completed=" << completedOk
                         << "osUpdate=" << osUpdateOk;
    }
}

void UpdaterBridge::applyOSUpgrade()
{
    if (!m_iface) return;
    qCInfo(lcUpd) << "ApplyOSUpgrade requested by UI";
    m_iface->asyncCall(QStringLiteral("ApplyOSUpgrade"));
}

void UpdaterBridge::onProgress(const QString& stage, int percent, const QString& message)
{
    qCInfo(lcUpd) << "Progress:" << stage << percent << message;
    setState(/*active=*/true, stage, percent, message,
             /*failed=*/false, /*requiresReboot=*/m_requiresReboot);
}

void UpdaterBridge::onCompleted(bool success, const QString& message)
{
    qCInfo(lcUpd) << "Completed:" << success << message;
    // Heuristic: a successful os.upgrade emits a message containing
    // "reboot" — surface that as a separate property so the splash
    // can show an Install / Restart Now button.
    const bool reboot = success && m_stage == QStringLiteral("os.upgrade")
                        && message.contains(QStringLiteral("reboot"));
    if (success) {
        // If the user just installed the OS upgrade, clear the
        // advisory flag — the upgrade is no longer "pending".
        if (m_stage == QStringLiteral("os.upgrade")) {
            m_osUpdateAvailable = false;
            m_osVersion.clear();
            emit osUpdateChanged();
        }
        setState(/*active=*/false, QString(), 100, message,
                 /*failed=*/false, /*requiresReboot=*/reboot);
    } else {
        setState(/*active=*/true, m_stage, m_percent, message,
                 /*failed=*/true, /*requiresReboot=*/false);
    }
}

void UpdaterBridge::onOSUpdateAvailable(const QString& version)
{
    qCInfo(lcUpd) << "OSUpdateAvailable:" << version;
    m_osUpdateAvailable = true;
    m_osVersion = version;
    emit osUpdateChanged();
}

void UpdaterBridge::setState(bool active, const QString& stage, int percent,
                             const QString& message, bool failed, bool requiresReboot)
{
    const bool changed = (active != m_active) || (stage != m_stage)
                        || (percent != m_percent) || (message != m_message)
                        || (failed != m_failed) || (requiresReboot != m_requiresReboot);
    m_active = active;
    m_stage = stage;
    m_percent = percent;
    m_message = message;
    m_failed = failed;
    m_requiresReboot = requiresReboot;
    if (changed) emit stateChanged();
}
