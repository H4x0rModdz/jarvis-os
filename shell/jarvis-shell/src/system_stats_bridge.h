#pragma once

#include <QObject>
#include <QString>
#include <QTimer>
#include <QVariantList>
#include <QVector>
#include <qqmlintegration.h>

/// Live system telemetry for the eDEX-style desktop HUD (the "command
/// center" panels on the home surface — see Desktop.qml). Polls /proc once
/// a second and exposes CPU / memory / network / uptime / top-processes as
/// QML properties. One `updated()` signal fires per tick; QML re-reads all
/// bindings off it (same pattern as PowerBridge's stateChanged).
///
/// Pure read-only telemetry from /proc — no subprocess, no privilege. On a
/// non-Linux dev host the /proc reads just yield zeros, which renders an
/// empty-but-valid HUD rather than crashing.
class SystemStatsBridge : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON

    Q_PROPERTY(int cpuPercent READ cpuPercent NOTIFY updated)
    Q_PROPERTY(QVariantList perCore READ perCore NOTIFY updated)
    Q_PROPERTY(QVariantList cpuHistory READ cpuHistory NOTIFY updated)
    Q_PROPERTY(int memPercent READ memPercent NOTIFY updated)
    Q_PROPERTY(double memUsedGiB READ memUsedGiB NOTIFY updated)
    Q_PROPERTY(double memTotalGiB READ memTotalGiB NOTIFY updated)
    Q_PROPERTY(double swapUsedGiB READ swapUsedGiB NOTIFY updated)
    Q_PROPERTY(QString uptimeText READ uptimeText NOTIFY updated)
    Q_PROPERTY(double netUpKBs READ netUpKBs NOTIFY updated)
    Q_PROPERTY(double netDownKBs READ netDownKBs NOTIFY updated)
    Q_PROPERTY(QVariantList netUpHistory READ netUpHistory NOTIFY updated)
    Q_PROPERTY(QVariantList netDownHistory READ netDownHistory NOTIFY updated)
    Q_PROPERTY(bool online READ online NOTIFY updated)
    // Network identity of the default-route interface (wired or wifi) — empty
    // when offline. NetworkBridge is WiFi-only and exposes no IP/gateway.
    Q_PROPERTY(QString ipAddress READ ipAddress NOTIFY updated)
    Q_PROPERTY(QString gateway READ gateway NOTIFY updated)
    Q_PROPERTY(QString primaryIface READ primaryIface NOTIFY updated)
    // Disk usage for the root filesystem ("/") and the user's home. bootc keeps
    // /usr read-only and /home a separate subvolume, so the two fill up for very
    // different reasons — worth showing both. `*Percent` is 0..100.
    Q_PROPERTY(double diskUsedGiB READ diskUsedGiB NOTIFY updated)
    Q_PROPERTY(double diskTotalGiB READ diskTotalGiB NOTIFY updated)
    Q_PROPERTY(int diskPercent READ diskPercent NOTIFY updated)
    Q_PROPERTY(double homeUsedGiB READ homeUsedGiB NOTIFY updated)
    Q_PROPERTY(double homeTotalGiB READ homeTotalGiB NOTIFY updated)
    Q_PROPERTY(int homePercent READ homePercent NOTIFY updated)
    // Hottest CPU core in °C from /sys/class/hwmon, or 0 when no sensor is
    // readable (common in a VM) — the UI hides the row when 0.
    Q_PROPERTY(int cpuTempC READ cpuTempC NOTIFY updated)
    Q_PROPERTY(int taskCount READ taskCount NOTIFY updated)
    Q_PROPERTY(QString cpuModel READ cpuModel CONSTANT)
    // "LilithOS <version>" from /etc/os-release, read once — the currently
    // *booted* build. (UpdaterBridge only knows the *available* image.)
    Q_PROPERTY(QString osRelease READ osRelease CONSTANT)
    /// [{ pid:int, name:string, mem:double (percent) }], top 5 by RSS.
    Q_PROPERTY(QVariantList topProcesses READ topProcesses NOTIFY updated)

public:
    explicit SystemStatsBridge(QObject* parent = nullptr);

    int cpuPercent() const { return m_cpuPercent; }
    QVariantList perCore() const { return m_perCore; }
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
    double homeUsedGiB() const { return m_homeUsedGiB; }
    double homeTotalGiB() const { return m_homeTotalGiB; }
    int homePercent() const { return m_homePercent; }
    int cpuTempC() const { return m_cpuTempC; }
    int taskCount() const { return m_taskCount; }
    QString cpuModel() const { return m_cpuModel; }
    QString osRelease() const { return m_osRelease; }
    QVariantList topProcesses() const { return m_topProcesses; }

signals:
    void updated();

private:
    void tick();
    void sampleCpu();
    void sampleMem();
    void sampleNet();
    void sampleUptime();
    void sampleProcs();
    void sampleDisk();
    void sampleTemp();
    void sampleNetIdentity();
    static QString readCpuModel();
    static QString readOsRelease();

    QTimer m_timer;

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
    double m_homeUsedGiB = 0.0;
    double m_homeTotalGiB = 0.0;
    int m_homePercent = 0;
    int m_cpuTempC = 0;
    int m_taskCount = 0;
    QString m_cpuModel;
    QString m_osRelease;
    QVariantList m_topProcesses;

    // Previous CPU jiffies (aggregate + per-core) for delta computation.
    quint64 m_prevTotal = 0;
    quint64 m_prevIdle = 0;
    QVector<quint64> m_prevCoreTotal;
    QVector<quint64> m_prevCoreIdle;
    // Previous network byte totals (summed across non-loopback interfaces).
    quint64 m_prevRx = 0;
    quint64 m_prevTx = 0;
    bool m_haveNetPrev = false;

    // The top-processes table walks every /proc/<pid> and reads two files per
    // process — the one genuinely expensive sample, and it runs on the GUI
    // thread. Everything else is a handful of reads, so only this one is
    // throttled: once every kProcScanEvery ticks instead of every tick.
    static constexpr int kProcScanEvery = 5;
    int m_procScanTick = kProcScanEvery; // scan on the very first tick

    static constexpr int kHistory = 48;
};
