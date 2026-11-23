#pragma once

#include <QObject>
#include <QString>
#include <QThread>
#include <QVariantList>
#include <qqmlintegration.h>

#include "system_stats_sampler.h"

/// Live system telemetry for the eDEX-style desktop HUD (the "command
/// center" panels on the home surface — see Desktop.qml): CPU / memory /
/// network / disk / uptime / top-processes as QML properties.
///
/// The reading happens on a worker thread (SystemStatsSampler); this object
/// only receives finished samples and republishes them. Each group of
/// properties has its own change signal, emitted only when a value the HUD
/// would draw differently has actually changed — so a quiet machine triggers
/// almost no binding re-evaluation, no relayout and no repaint. (One blanket
/// `updated()` per second used to re-evaluate every binding on the desktop
/// and rebuild both Repeaters' delegates, changed or not.)
///
/// Pure read-only telemetry from /proc — no subprocess, no privilege. On a
/// non-Linux dev host the /proc reads just yield zeros, which renders an
/// empty-but-valid HUD rather than crashing.
class SystemStatsBridge : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON

    Q_PROPERTY(int cpuPercent READ cpuPercent NOTIFY cpuChanged)
    /// Per-core load, 0..100, indexed by core. Drive a Repeater with
    /// `coreCount` (stable) and read `perCore[index]` inside the delegate —
    /// using this list as the model would rebuild every delegate each second.
    Q_PROPERTY(QVariantList perCore READ perCore NOTIFY cpuChanged)
    Q_PROPERTY(int coreCount READ coreCount NOTIFY coreCountChanged)
    Q_PROPERTY(QVariantList cpuHistory READ cpuHistory NOTIFY cpuChanged)
    Q_PROPERTY(int memPercent READ memPercent NOTIFY memChanged)
    Q_PROPERTY(double memUsedGiB READ memUsedGiB NOTIFY memChanged)
    Q_PROPERTY(double memTotalGiB READ memTotalGiB NOTIFY memChanged)
    Q_PROPERTY(double swapUsedGiB READ swapUsedGiB NOTIFY memChanged)
    Q_PROPERTY(QString uptimeText READ uptimeText NOTIFY uptimeChanged)
    Q_PROPERTY(double netUpKBs READ netUpKBs NOTIFY netRateChanged)
    Q_PROPERTY(double netDownKBs READ netDownKBs NOTIFY netRateChanged)
    Q_PROPERTY(QVariantList netUpHistory READ netUpHistory NOTIFY netRateChanged)
    Q_PROPERTY(QVariantList netDownHistory READ netDownHistory NOTIFY netRateChanged)
    // Network identity of the default-route interface (wired or wifi) — empty
    // when offline. NetworkBridge is WiFi-only and exposes no IP/gateway.
    Q_PROPERTY(bool online READ online NOTIFY netIdentityChanged)
    Q_PROPERTY(QString ipAddress READ ipAddress NOTIFY netIdentityChanged)
    Q_PROPERTY(QString gateway READ gateway NOTIFY netIdentityChanged)
    Q_PROPERTY(QString primaryIface READ primaryIface NOTIFY netIdentityChanged)
    // Usage of the writable store (/var on bootc; "/" elsewhere). `diskPercent`
    // is 0..100.
    Q_PROPERTY(double diskUsedGiB READ diskUsedGiB NOTIFY diskChanged)
    Q_PROPERTY(double diskTotalGiB READ diskTotalGiB NOTIFY diskChanged)
    Q_PROPERTY(int diskPercent READ diskPercent NOTIFY diskChanged)
    // Hottest CPU core in °C from /sys/class/hwmon, or 0 when no sensor is
    // readable (common in a VM) — the UI hides the row when 0.
    Q_PROPERTY(int cpuTempC READ cpuTempC NOTIFY cpuTempChanged)
    Q_PROPERTY(int taskCount READ taskCount NOTIFY taskCountChanged)
    Q_PROPERTY(QString cpuModel READ cpuModel CONSTANT)
    // "LilithOS <version>" from /etc/os-release, read once — the currently
    // *booted* build. (UpdaterBridge only knows the *available* image.)
    Q_PROPERTY(QString osRelease READ osRelease CONSTANT)
    /// [{ pid:int, name:string, mem:double (percent) }], top 5 by RSS.
    Q_PROPERTY(QVariantList topProcesses READ topProcesses NOTIFY topProcessesChanged)

public:
    explicit SystemStatsBridge(QObject* parent = nullptr);
    ~SystemStatsBridge() override;

    int cpuPercent() const { return m_cpuPercent; }
    QVariantList perCore() const { return m_perCore; }
    int coreCount() const { return int(m_perCore.size()); }
    QVariantList cpuHistory() const { return m_cpuHistory; }
    int memPercent() const { return m_memPercent; }
    double memUsedGiB() const { return m_memUsedGiB; }
    double memTotalGiB() const { return m_memTotalGiB; }
    double swapUsedGiB() const { return m_swapUsedGiB; }
    QString uptimeText() const { return m_uptimeText; }
    double netUpKBs() const { return m_netUpKBs; }
    double netDownKBs() const { return m_netDownKBs; }
    QVariantList netUpHistory() const { return m_netUpHistory; }
    QVariantList netDownHistory() const { return m_netDownHistory; }
    bool online() const { return m_online; }
    QString ipAddress() const { return m_ipAddress; }
    QString gateway() const { return m_gateway; }
    QString primaryIface() const { return m_primaryIface; }
    double diskUsedGiB() const { return m_diskUsedGiB; }
    double diskTotalGiB() const { return m_diskTotalGiB; }
    int diskPercent() const { return m_diskPercent; }
    int cpuTempC() const { return m_cpuTempC; }
    int taskCount() const { return m_taskCount; }
    QString cpuModel() const { return m_cpuModel; }
    QString osRelease() const { return m_osRelease; }
    QVariantList topProcesses() const { return m_topProcesses; }

signals:
    void cpuChanged();
    void coreCountChanged();
    void memChanged();
    void uptimeChanged();
    void netRateChanged();
    void netIdentityChanged();
    void diskChanged();
    void cpuTempChanged();
    void taskCountChanged();
    void topProcessesChanged();

private:
    void apply(const StatsSample& s);
    static QString readCpuModel();
    static QString readOsRelease();

    /// Runs the SystemStatsSampler.
    QThread m_samplerThread;

    int m_cpuPercent = 0;
    QVariantList m_perCore;
    QVariantList m_cpuHistory;
    int m_memPercent = 0;
    double m_memUsedGiB = 0.0;
    double m_memTotalGiB = 0.0;
    double m_swapUsedGiB = 0.0;
    QString m_uptimeText;
    double m_netUpKBs = 0.0;
    double m_netDownKBs = 0.0;
    QVariantList m_netUpHistory;
    QVariantList m_netDownHistory;
    bool m_online = false;
    QString m_ipAddress;
    QString m_gateway;
    QString m_primaryIface;
    double m_diskUsedGiB = 0.0;
    double m_diskTotalGiB = 0.0;
    int m_diskPercent = 0;
    int m_cpuTempC = 0;
    int m_taskCount = 0;
    QString m_cpuModel;
    QString m_osRelease;
    QVariantList m_topProcesses;

    /// Points kept in each history graph.
    static constexpr int kHistory = 48;
};
