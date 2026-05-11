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

    if (!progressOk || !completedOk) {
        qCWarning(lcUpd) << "Failed to subscribe to updater signals"
                         << "progress=" << progressOk
                         << "completed=" << completedOk;
    }
}

void UpdaterBridge::onProgress(const QString& stage, int percent, const QString& message)
{
    qCInfo(lcUpd) << "Progress:" << stage << percent << message;
    setState(/*active=*/true, stage, percent, message, /*failed=*/false);
}

void UpdaterBridge::onCompleted(bool success, const QString& message)
{
    qCInfo(lcUpd) << "Completed:" << success << message;
    if (success) {
        // Mark inactive — the splash fades out and the regular bar takes over.
        setState(/*active=*/false, QString(), 100, message, /*failed=*/false);
    } else {
        // Stay visible so the user sees the error. UI shows a retry path.
        setState(/*active=*/true, m_stage, m_percent, message, /*failed=*/true);
    }
}

void UpdaterBridge::setState(bool active, const QString& stage, int percent,
                             const QString& message, bool failed)
{
    const bool changed = (active != m_active) || (stage != m_stage)
                        || (percent != m_percent) || (message != m_message)
                        || (failed != m_failed);
    m_active = active;
    m_stage = stage;
    m_percent = percent;
    m_message = message;
    m_failed = failed;
    if (changed) emit stateChanged();
}
