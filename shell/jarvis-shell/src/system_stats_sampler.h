#pragma once

#include <QElapsedTimer>
#include <QList>
#include <QMetaType>
#include <QObject>
#include <QString>
#include <QStringList>
#include <QVariantList>
#include <QVector>

/// One reading of everything the desktop HUD shows. A plain value type so it
/// can cross from the sampler thread to the GUI thread in a queued signal.
struct StatsSample
{
    int cpuPercent = 0;
    QList<int> perCore;

    int memPercent = 0;
    double memUsedGiB = 0.0;
    double memTotalGiB = 0.0;
    double swapUsedGiB = 0.0;

    /// False on the very first sample — a rate needs two readings.
    bool haveNetRates = false;
    double netUpKBs = 0.0;
    double netDownKBs = 0.0;

    bool online = false;
    QString primaryIface;
    QString ipAddress;
    QString gateway;

    double diskUsedGiB = 0.0;
    double diskTotalGiB = 0.0;
    int diskPercent = 0;

    int cpuTempC = 0;
    int taskCount = 0;
    QString uptimeText;

    /// The process table is only rescanned every few samples; false means
    /// "keep showing the previous one".
    bool haveTopProcesses = false;
    QVariantList topProcesses;
};
Q_DECLARE_METATYPE(StatsSample)

/// Reads /proc and /sys once a second and emits a StatsSample. Lives on its
/// own thread (owned by SystemStatsBridge) so that none of this I/O — least of
/// all the walk over every /proc/<pid> — can ever stall the GUI thread, which
/// is shared by every window of the shell.
class SystemStatsSampler : public QObject
{
    Q_OBJECT

public:
    explicit SystemStatsSampler(QObject* parent = nullptr);

public slots:
    /// Begin sampling. Must run on the sampler's own thread (connect it to
    /// QThread::started) so the timer it creates lives there.
    void start();

signals:
    void sampled(const StatsSample& sample);

private:
    void sample();
    void sampleCpu(StatsSample& s);
    void sampleMem(StatsSample& s);
    void sampleNet(StatsSample& s);
    void sampleUptime(StatsSample& s);
    void sampleProcs(StatsSample& s);
    void sampleDisk(StatsSample& s);
    void sampleTemp(StatsSample& s);
    void findTempSensors();

    // Previous CPU jiffies (aggregate + per-core) for delta computation.
    quint64 m_prevTotal = 0;
    quint64 m_prevIdle = 0;
    QVector<quint64> m_prevCoreTotal;
    QVector<quint64> m_prevCoreIdle;
    int m_cpuPercent = 0;

    // Previous network byte totals (summed across non-loopback interfaces) and
    // when they were read — the timer is not exact, so rates divide by the
    // real elapsed time rather than assuming one second.
    quint64 m_prevRx = 0;
    quint64 m_prevTx = 0;
    bool m_haveNetPrev = false;
    QElapsedTimer m_netClock;

    double m_memTotalGiB = 0.0;

    // The top-processes table walks every /proc/<pid> and reads two files per
    // process — the one genuinely expensive sample — so it runs once every
    // kProcScanEvery samples instead of every sample.
    static constexpr int kProcScanEvery = 5;
    int m_procScanTick = kProcScanEvery; // scan on the very first sample

    // hwmon temp*_input files, found once: the sensor set doesn't change while
    // running, so there is no reason to re-walk /sys/class/hwmon every second.
    // Retried occasionally while empty (a VM has none; a module may load late).
    QStringList m_tempInputs;
    static constexpr int kTempRescanEvery = 60;
    int m_tempRescanTick = 0;
};
