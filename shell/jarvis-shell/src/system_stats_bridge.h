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
    Q_PROPERTY(int taskCount READ taskCount NOTIFY updated)
    Q_PROPERTY(QString cpuModel READ cpuModel CONSTANT)
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
    int taskCount() const { return m_taskCount; }
    QString cpuModel() const { return m_cpuModel; }
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
    static QString readCpuModel();

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
    int m_taskCount = 0;
    QString m_cpuModel;
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

    static constexpr int kHistory = 48;
};
