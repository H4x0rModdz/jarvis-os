#pragma once

#include <QDBusInterface>
#include <QObject>
#include <QString>
#include <qqmlintegration.h>

/// Bridge between QML and `com.jarvis.Updater`.
///
/// Two state slices live here:
///
///   - **Active operation** (model pull / OS upgrade) — `active`, `stage`,
///     `percent`, `message`, `failed`. Bound by `UpdaterSplash` to render
///     a progress bar; auto-clears on Completed(success=true).
///   - **OS update advisory** — `osUpdateAvailable`, `osVersion`. Set when
///     the daemon's startup probe finds a staged bootc image. Independent
///     of the active operation — the splash shows both at once when the
///     user is in the middle of a model pull AND there's an OS update
///     waiting.
class UpdaterBridge : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    Q_PROPERTY(bool active READ active NOTIFY stateChanged)
    Q_PROPERTY(QString stage READ stage NOTIFY stateChanged)
    Q_PROPERTY(int percent READ percent NOTIFY stateChanged)
    Q_PROPERTY(QString message READ message NOTIFY stateChanged)
    Q_PROPERTY(bool failed READ failed NOTIFY stateChanged)

    Q_PROPERTY(bool osUpdateAvailable READ osUpdateAvailable NOTIFY osUpdateChanged)
    Q_PROPERTY(QString osVersion READ osVersion NOTIFY osUpdateChanged)
    Q_PROPERTY(bool requiresReboot READ requiresReboot NOTIFY stateChanged)

public:
    explicit UpdaterBridge(QObject* parent = nullptr);

    bool active() const { return m_active; }
    QString stage() const { return m_stage; }
    int percent() const { return m_percent; }
    QString message() const { return m_message; }
    bool failed() const { return m_failed; }

    bool osUpdateAvailable() const { return m_osUpdateAvailable; }
    QString osVersion() const { return m_osVersion; }
    /// True after a successful OS upgrade — splash should prompt reboot.
    bool requiresReboot() const { return m_requiresReboot; }

    /// Ask the daemon to apply the pending bootc OS upgrade.
    Q_INVOKABLE void applyOSUpgrade();

    /// User-initiated update check (the Jarvis-menu "Atualização do
    /// sistema" item). Calls the daemon's Check(); when an OS update is
    /// staged it flips `osUpdateAvailable` so the splash surfaces the
    /// install prompt, otherwise it emits `upToDate` so the UI can tell
    /// the user nothing is pending. `checkFailed` fires if the daemon is
    /// unreachable. Without this the menu item dispatched a fire-and-
    /// forget action and the user saw no response at all.
    Q_INVOKABLE void checkNow();

signals:
    void stateChanged();
    void osUpdateChanged();
    void upToDate();
    void checkFailed(const QString& message);

private slots:
    void onProgress(const QString& stage, int percent, const QString& message);
    void onCompleted(bool success, const QString& message);
    void onOSUpdateAvailable(const QString& version);

private:
    void setState(bool active, const QString& stage, int percent,
                  const QString& message, bool failed, bool requiresReboot);

    QDBusInterface* m_iface = nullptr;
    bool m_active = false;
    bool m_failed = false;
    bool m_requiresReboot = false;
    QString m_stage;
    int m_percent = -1;
    QString m_message;

    bool m_osUpdateAvailable = false;
    QString m_osVersion;
};
