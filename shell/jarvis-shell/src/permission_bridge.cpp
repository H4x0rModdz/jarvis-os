#include "permission_bridge.h"

#include <QDBusConnection>
#include <QDBusPendingCall>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QLoggingCategory>

namespace {
Q_LOGGING_CATEGORY(lcPerm, "jarvis.shell.permission")

constexpr const char* kService = "com.jarvis.PermissionSystem";
constexpr const char* kPath = "/com/jarvis/PermissionSystem";
constexpr const char* kIface = "com.jarvis.PermissionSystem";
}

PermissionBridge::PermissionBridge(QObject* parent) : QObject(parent)
{
    auto bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        qCWarning(lcPerm) << "Session bus not connected";
        emit errorOccurred(tr("DBus session bus unavailable"));
        return;
    }

    m_iface = new QDBusInterface(kService, kPath, kIface, bus, this);

    // Subscribe to ApprovalRequested. The signal carries four strings:
    //   request_id, caller, scope, action
    bool ok = bus.connect(
        kService,
        kPath,
        kIface,
        QStringLiteral("ApprovalRequested"),
        this,
        SLOT(onApprovalRequested(QString, QString, QString, QString)));
    if (!ok) {
        qCWarning(lcPerm) << "Failed to subscribe to ApprovalRequested";
        emit errorOccurred(tr("Could not subscribe to permission approvals"));
    }
}

void PermissionBridge::onApprovalRequested(const QString& requestId,
                                           const QString& caller,
                                           const QString& scope,
                                           const QString& action)
{
    qCInfo(lcPerm) << "Approval requested:" << caller << scope << action << "id=" << requestId;
    m_pendingId = requestId;
    m_pendingCaller = caller;
    m_pendingScope = scope;
    m_pendingAction = action;
    emit pendingChanged();
}

void PermissionBridge::approveOnce()      { resolve(QStringLiteral("approve")); }
void PermissionBridge::approvePersistent() { resolve(QStringLiteral("approve_persistent")); }
void PermissionBridge::deny()             { resolve(QStringLiteral("deny")); }

void PermissionBridge::resolve(const QString& decision)
{
    if (!m_iface || m_pendingId.isEmpty()) return;

    const QString id = m_pendingId;
    // Clear local state before the round-trip so the dialog hides
    // immediately and the user can issue more commands while the daemon
    // finishes propagating the decision.
    clearPending();

    auto pending = m_iface->asyncCall(QStringLiteral("ResolveApproval"), id, decision);
    auto* watcher = new QDBusPendingCallWatcher(pending, this);
    QObject::connect(watcher, &QDBusPendingCallWatcher::finished, this,
        [this, id, decision](QDBusPendingCallWatcher* w) {
            QDBusPendingReply<QString> reply = *w;
            if (reply.isError()) {
                qCWarning(lcPerm) << "ResolveApproval failed:" << reply.error().message();
                emit errorOccurred(reply.error().message());
            } else {
                qCInfo(lcPerm) << "Approval resolved" << id << "->" << decision
                               << "reply=" << reply.value();
            }
            w->deleteLater();
        });
}

void PermissionBridge::clearPending()
{
    if (m_pendingId.isEmpty()) return;
    m_pendingId.clear();
    m_pendingCaller.clear();
    m_pendingScope.clear();
    m_pendingAction.clear();
    emit pendingChanged();
}
