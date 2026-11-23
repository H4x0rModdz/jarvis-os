#include "system_stats_bridge.h"

#include <QFile>

namespace {
QString readAll(const QString& path)
{
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly | QIODevice::Text)) {
        return QString();
    }
    return QString::fromUtf8(f.readAll());
}

/// True when two readings would print differently at one decimal place —
/// the finest precision the HUD shows. Anything smaller is noise, and
/// notifying on noise is what makes an idle desktop repaint.
bool differs(double a, double b)
{
    return qRound64(a * 10.0) != qRound64(b * 10.0);
}
} // namespace

SystemStatsBridge::SystemStatsBridge(QObject* parent) : QObject(parent)
{
    m_cpuModel = readCpuModel();
    m_osRelease = readOsRelease();

    qRegisterMetaType<StatsSample>();

    // The sampler has no parent so it can move to its thread; it is deleted
    // there when the thread finishes.
    auto* sampler = new SystemStatsSampler;
    sampler->moveToThread(&m_samplerThread);
    QObject::connect(&m_samplerThread, &QThread::started, sampler, &SystemStatsSampler::start);
    QObject::connect(&m_samplerThread, &QThread::finished, sampler, &QObject::deleteLater);
    // Queued across threads: apply() runs here, on the GUI thread.
    QObject::connect(sampler, &SystemStatsSampler::sampled, this, &SystemStatsBridge::apply);

    m_samplerThread.setObjectName(QStringLiteral("jarvis-stats"));
    m_samplerThread.start(QThread::LowPriority);
}

SystemStatsBridge::~SystemStatsBridge()
{
    m_samplerThread.quit();
    m_samplerThread.wait();
}

void SystemStatsBridge::apply(const StatsSample& s)
{
    // ── CPU ──────────────────────────────────────────────────────────────
    // The history graph scrolls by one point per sample, so this group
    // changes every second by definition.
    const bool coreCountDiffers = m_perCore.size() != s.perCore.size();
    m_cpuPercent = s.cpuPercent;
    QVariantList cores;
    cores.reserve(s.perCore.size());
    for (int pct : s.perCore) cores.append(pct);
    m_perCore = cores;
    m_cpuHistory.append(s.cpuPercent);
    while (m_cpuHistory.size() > kHistory) m_cpuHistory.removeFirst();
    if (coreCountDiffers) emit coreCountChanged();
    emit cpuChanged();

    // ── Memory ───────────────────────────────────────────────────────────
    if (m_memPercent != s.memPercent || differs(m_memUsedGiB, s.memUsedGiB)
        || differs(m_memTotalGiB, s.memTotalGiB) || differs(m_swapUsedGiB, s.swapUsedGiB)) {
        m_memPercent = s.memPercent;
        m_memUsedGiB = s.memUsedGiB;
        m_memTotalGiB = s.memTotalGiB;
        m_swapUsedGiB = s.swapUsedGiB;
        emit memChanged();
    }

    // ── Network throughput (scrolling graphs, like CPU) ──────────────────
    if (s.haveNetRates) {
        m_netUpKBs = s.netUpKBs;
        m_netDownKBs = s.netDownKBs;
        m_netUpHistory.append(s.netUpKBs);
        m_netDownHistory.append(s.netDownKBs);
        while (m_netUpHistory.size() > kHistory) m_netUpHistory.removeFirst();
        while (m_netDownHistory.size() > kHistory) m_netDownHistory.removeFirst();
        emit netRateChanged();
    }

    // ── Network identity ─────────────────────────────────────────────────
    if (m_online != s.online || m_primaryIface != s.primaryIface
        || m_ipAddress != s.ipAddress || m_gateway != s.gateway) {
        m_online = s.online;
        m_primaryIface = s.primaryIface;
        m_ipAddress = s.ipAddress;
        m_gateway = s.gateway;
        emit netIdentityChanged();
    }

    // ── Disk ─────────────────────────────────────────────────────────────
    if (m_diskPercent != s.diskPercent || differs(m_diskUsedGiB, s.diskUsedGiB)
        || differs(m_diskTotalGiB, s.diskTotalGiB)) {
        m_diskPercent = s.diskPercent;
        m_diskUsedGiB = s.diskUsedGiB;
        m_diskTotalGiB = s.diskTotalGiB;
        emit diskChanged();
    }

    // ── Singles ──────────────────────────────────────────────────────────
    if (m_cpuTempC != s.cpuTempC) {
        m_cpuTempC = s.cpuTempC;
        emit cpuTempChanged();
    }
    if (m_taskCount != s.taskCount) {
        m_taskCount = s.taskCount;
        emit taskCountChanged();
    }
    if (m_uptimeText != s.uptimeText) { // minute resolution
        m_uptimeText = s.uptimeText;
        emit uptimeChanged();
    }

    // ── Top processes ────────────────────────────────────────────────────
    if (s.haveTopProcesses && m_topProcesses != s.topProcesses) {
        m_topProcesses = s.topProcesses;
        emit topProcessesChanged();
    }
}

QString SystemStatsBridge::readOsRelease()
{
    // NAME + VERSION from /etc/os-release → e.g. "LilithOS 42.20260713". Values
    // may be quoted; strip a single pair of surrounding quotes.
    const QString s = readAll(QStringLiteral("/etc/os-release"));
    auto val = [&s](const QString& key) -> QString {
        for (const QString& line : s.split('\n')) {
            if (line.startsWith(key)) {
                QString v = line.mid(key.size()).trimmed();
                if (v.size() >= 2 && v.startsWith('"') && v.endsWith('"'))
                    v = v.mid(1, v.size() - 2);
                return v;
            }
        }
        return QString();
    };
    QString name = val(QStringLiteral("NAME="));
    if (name.isEmpty()) name = QStringLiteral("LilithOS");
    QString version = val(QStringLiteral("VERSION="));
    if (version.isEmpty()) version = val(QStringLiteral("VERSION_ID="));
    return version.isEmpty() ? name : (name + QStringLiteral(" ") + version);
}

QString SystemStatsBridge::readCpuModel()
{
    const QString s = readAll(QStringLiteral("/proc/cpuinfo"));
    for (const QString& line : s.split('\n')) {
        if (line.startsWith(QLatin1String("model name"))) {
            const int colon = line.indexOf(':');
            if (colon >= 0) return line.mid(colon + 1).trimmed();
        }
    }
    return QStringLiteral("CPU");
}
