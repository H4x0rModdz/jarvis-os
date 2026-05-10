#pragma once

#include <QObject>
#include <QString>
#include <QDBusInterface>
#include <qqmlintegration.h>

/// Bridge between QML and `com.jarvis.PermissionSystem`.
///
/// Subscribes to the `ApprovalRequested` signal and exposes the current
/// pending request as Q_PROPERTYs so QML can bind a dialog to it. When the
/// user clicks a button, the bridge calls `ResolveApproval` and clears the
/// pending state.
///
/// Phase 1 holds a single pending request — if a second signal arrives
/// while the dialog is up, the new one overwrites the current pending
/// (the old one will hit the daemon's 30 s timeout and auto-deny). A
/// queue can replace this later if multiple concurrent prompts become a
/// real-world scenario.
class PermissionBridge : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    Q_PROPERTY(bool hasPending READ hasPending NOTIFY pendingChanged)
    Q_PROPERTY(QString pendingRequestId READ pendingRequestId NOTIFY pendingChanged)
    Q_PROPERTY(QString pendingCaller READ pendingCaller NOTIFY pendingChanged)
    Q_PROPERTY(QString pendingScope READ pendingScope NOTIFY pendingChanged)
    Q_PROPERTY(QString pendingAction READ pendingAction NOTIFY pendingChanged)

public:
    explicit PermissionBridge(QObject* parent = nullptr);

    bool hasPending() const { return !m_pendingId.isEmpty(); }
    QString pendingRequestId() const { return m_pendingId; }
    QString pendingCaller() const { return m_pendingCaller; }
    QString pendingScope() const { return m_pendingScope; }
    QString pendingAction() const { return m_pendingAction; }

    Q_INVOKABLE void approveOnce();
    Q_INVOKABLE void approvePersistent();
    Q_INVOKABLE void deny();

signals:
    void pendingChanged();
    void errorOccurred(const QString& message);

private slots:
    void onApprovalRequested(const QString& requestId,
                             const QString& caller,
                             const QString& scope,
                             const QString& action);

private:
    void resolve(const QString& decision);
    void clearPending();

    QDBusInterface* m_iface = nullptr;
    QString m_pendingId;
    QString m_pendingCaller;
    QString m_pendingScope;
    QString m_pendingAction;
};
