#pragma once

#include <QDBusInterface>
#include <QObject>
#include <QString>
#include <QVariantMap>
#include <qqmlintegration.h>

/// Bridge to UPower for laptop battery state. Listens on the
/// SYSTEM bus (UPower runs as root) and tracks the DisplayDevice —
/// UPower's synthesised aggregate of every battery on the machine,
/// which is what almost every desktop wants to surface.
///
/// Exposes:
///   - `hasBattery`   — true when DisplayDevice exists and reports
///                      a real battery. Bar hides the indicator
///                      when false (desktop / VM without battery).
///   - `percentage`   — 0..100 double.
///   - `state`        — "charging" | "discharging" | "full" | "empty"
///                      | "pending-charge" | "pending-discharge"
///                      | "unknown".
///   - `charging`     — convenience bool derived from `state`.
///   - `timeRemaining` — seconds. 0 when UPower hasn't decided yet
///                       (cold boot, plug/unplug transient).
///   - `lastWarning`  — which threshold the bridge has already
///                       toasted for the current discharge session;
///                       reset on charge or full. Q_PROPERTY so the
///                       bar can sanity-check without poking the
///                       battery threshold tables.
///
/// The bridge fires a single notify-send via the existing
/// notifications daemon when the battery crosses a low threshold
/// while discharging (15% / 5%). Debounced so a percentage that
/// fluctuates across the line doesn't spam.
class PowerBridge : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    Q_PROPERTY(bool hasBattery READ hasBattery NOTIFY stateChanged)
    Q_PROPERTY(double percentage READ percentage NOTIFY stateChanged)
    Q_PROPERTY(QString state READ state NOTIFY stateChanged)
    Q_PROPERTY(bool charging READ charging NOTIFY stateChanged)
    Q_PROPERTY(int timeRemaining READ timeRemaining NOTIFY stateChanged)
    Q_PROPERTY(QString iconName READ iconName NOTIFY stateChanged)

public:
    explicit PowerBridge(QObject* parent = nullptr);

    bool hasBattery() const { return m_hasBattery; }
    double percentage() const { return m_percentage; }
    QString state() const { return m_state; }
    bool charging() const { return m_state == QStringLiteral("charging"); }
    int timeRemaining() const { return m_timeRemaining; }
    QString iconName() const { return m_iconName; }

    /// Reboot the machine via logind (org.freedesktop.login1.Manager.Reboot).
    /// An active local session is permitted by the default login1 polkit
    /// policy, so no agent prompt is needed. Used by the updater splash's
    /// "Reiniciar agora" button to finish an OS upgrade.
    Q_INVOKABLE void reboot();

    /// Set the login password for the fixed `jarvis` account (first-boot
    /// wizard). Pipes `password` on stdin to
    /// `pkexec /usr/libexec/jarvis-set-password`, authorised without a prompt
    /// by 50-jarvis-setpw.rules. Blocks briefly until chpasswd returns.
    /// Returns false on empty input / failure (autologin means no lockout).
    Q_INVOKABLE bool setLoginPassword(const QString& password);

signals:
    void stateChanged();

private slots:
    void onPropertiesChanged(const QString& iface,
                             const QVariantMap& changed,
                             const QStringList& invalidated);

private:
    void refreshAll();
    void evaluateLowBattery();
    void notifyLow(int threshold);

    QDBusInterface* m_iface = nullptr;
    bool m_hasBattery = false;
    double m_percentage = 0.0;
    QString m_state = QStringLiteral("unknown");
    int m_timeRemaining = 0;
    QString m_iconName;
    // Highest threshold we've already warned for in this discharge
    // session (15 or 5). Reset to 100 when plugging back in so a
    // future discharge can warn again.
    int m_warnedAt = 100;
};
