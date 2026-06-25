#include "system_stats_bridge.h"

#include <QDir>
#include <QFile>
#include <QVariantMap>

#include <algorithm>
#include <unistd.h>

namespace {
/// Read a whole /proc file into a string. Empty on any failure (non-Linux dev
/// host, permission, race) — callers treat empty as "skip this sample".
QString readAll(const QString& path)
{
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly | QIODevice::Text)) {
        return QString();
    }
    return QString::fromUtf8(f.readAll());
}
} // namespace

SystemStatsBridge::SystemStatsBridge(QObject* parent) : QObject(parent)
{
    m_cpuModel = readCpuModel();

    // One-second cadence — matches the HUD's animation feel and keeps the
    // /proc reads negligible. First tick has no deltas yet (CPU/net read 0),
    // which is correct.
    m_timer.setInterval(1000);
    QObject::connect(&m_timer, &QTimer::timeout, this, &SystemStatsBridge::tick);
    m_timer.start();
    tick();
}

void SystemStatsBridge::tick()
{
    sampleCpu();
    sampleMem();
    sampleNet();
    sampleUptime();
    sampleProcs();
    emit updated();
}

void SystemStatsBridge::sampleCpu()
{
    const QString s = readAll(QStringLiteral("/proc/stat"));
    if (s.isEmpty()) return;

    QVariantList cores;
    const QStringList lines = s.split('\n');
    for (const QString& line : lines) {
        if (!line.startsWith(QLatin1String("cpu"))) continue;
        const QStringList p = line.split(' ', Qt::SkipEmptyParts);
        if (p.size() < 5) continue;

        // Sum only the first 8 numeric fields (user..steal); guest/guest_nice
        // are already folded into user/nice, so including them double-counts.
        quint64 total = 0;
        for (int i = 1; i < p.size() && i <= 8; ++i) total += p[i].toULongLong();
        quint64 idle = p[4].toULongLong();
        if (p.size() > 5) idle += p[5].toULongLong(); // + iowait

        if (p[0] == QLatin1String("cpu")) { // aggregate line
            const quint64 dt = total - m_prevTotal;
            const quint64 di = idle - m_prevIdle;
            if (m_prevTotal != 0 && dt > 0) {
                m_cpuPercent = qBound(0, int(qRound(100.0 * double(dt - di) / double(dt))), 100);
            }
            m_prevTotal = total;
            m_prevIdle = idle;
        } else { // per-core "cpuN"
            const int idx = p[0].mid(3).toInt();
            if (idx >= m_prevCoreTotal.size()) {
                m_prevCoreTotal.resize(idx + 1);
                m_prevCoreIdle.resize(idx + 1);
            }
            const quint64 dt = total - m_prevCoreTotal[idx];
            const quint64 di = idle - m_prevCoreIdle[idx];
            int pct = 0;
            if (m_prevCoreTotal[idx] != 0 && dt > 0) {
                pct = qBound(0, int(qRound(100.0 * double(dt - di) / double(dt))), 100);
            }
            m_prevCoreTotal[idx] = total;
            m_prevCoreIdle[idx] = idle;
            cores.append(pct);
        }
    }
    if (!cores.isEmpty()) m_perCore = cores;

    m_cpuHistory.append(m_cpuPercent);
    while (m_cpuHistory.size() > kHistory) m_cpuHistory.removeFirst();
}

void SystemStatsBridge::sampleMem()
{
    const QString s = readAll(QStringLiteral("/proc/meminfo"));
    if (s.isEmpty()) return;
    const QStringList lines = s.split('\n');
    auto kb = [&lines](const QString& key) -> quint64 {
        for (const QString& line : lines) {
            if (line.startsWith(key)) {
                const QStringList p = line.split(' ', Qt::SkipEmptyParts);
                if (p.size() >= 2) return p[1].toULongLong();
            }
        }
        return 0;
    };
    const quint64 total = kb(QStringLiteral("MemTotal:"));
    const quint64 avail = kb(QStringLiteral("MemAvailable:"));
    const quint64 swapT = kb(QStringLiteral("SwapTotal:"));
    const quint64 swapF = kb(QStringLiteral("SwapFree:"));
    if (total == 0) return;
    const quint64 used = total > avail ? total - avail : 0;
    m_memTotalGiB = total / 1024.0 / 1024.0;
    m_memUsedGiB = used / 1024.0 / 1024.0;
    m_memPercent = int(qRound(100.0 * double(used) / double(total)));
    m_swapUsedGiB = (swapT > swapF ? swapT - swapF : 0) / 1024.0 / 1024.0;
}

void SystemStatsBridge::sampleNet()
{
    const QString s = readAll(QStringLiteral("/proc/net/dev"));
    if (s.isEmpty()) return;
    quint64 rx = 0, tx = 0;
    const QStringList lines = s.split('\n');
    for (const QString& line : lines) {
        const int colon = line.indexOf(':');
        if (colon < 0) continue;
        const QString iface = line.left(colon).trimmed();
        if (iface == QLatin1String("lo")) continue;
        const QStringList p = line.mid(colon + 1).split(' ', Qt::SkipEmptyParts);
        if (p.size() < 9) continue;
        rx += p[0].toULongLong(); // receive bytes
        tx += p[8].toULongLong(); // transmit bytes
    }
    if (m_haveNetPrev) {
        const double up = (tx > m_prevTx ? tx - m_prevTx : 0) / 1024.0;   // KB/s @1s
        const double down = (rx > m_prevRx ? rx - m_prevRx : 0) / 1024.0; // KB/s @1s
        m_netUpKBs = up;
        m_netDownKBs = down;
        m_netUpHistory.append(up);
        m_netDownHistory.append(down);
        while (m_netUpHistory.size() > kHistory) m_netUpHistory.removeFirst();
        while (m_netDownHistory.size() > kHistory) m_netDownHistory.removeFirst();
    }
    m_prevRx = rx;
    m_prevTx = tx;
    m_haveNetPrev = true;
}

void SystemStatsBridge::sampleUptime()
{
    const QString s = readAll(QStringLiteral("/proc/uptime"));
    if (s.isEmpty()) return;
    const double secs = s.split(' ', Qt::SkipEmptyParts).value(0).toDouble();
    qint64 t = qint64(secs);
    const qint64 d = t / 86400;
    t %= 86400;
    const qint64 h = t / 3600;
    t %= 3600;
    const qint64 m = t / 60;
    m_uptimeText = QStringLiteral("%1d %2:%3")
                       .arg(d)
                       .arg(h, 2, 10, QChar('0'))
                       .arg(m, 2, 10, QChar('0'));
}

void SystemStatsBridge::sampleProcs()
{
    QDir proc(QStringLiteral("/proc"));
    const QStringList entries = proc.entryList(QDir::Dirs | QDir::NoDotAndDotDot);
    const long pageSize = sysconf(_SC_PAGESIZE);
    const double memTotalBytes = m_memTotalGiB * 1024.0 * 1024.0 * 1024.0;

    struct Proc {
        int pid;
        QString name;
        double mem;
    };
    QVector<Proc> procs;
    int count = 0;
    for (const QString& e : entries) {
        bool ok = false;
        const int pid = e.toInt(&ok);
        if (!ok) continue; // only numeric pid dirs
        ++count;
        const QString statm = readAll(QStringLiteral("/proc/%1/statm").arg(e));
        if (statm.isEmpty()) continue;
        const QStringList sp = statm.split(' ', Qt::SkipEmptyParts);
        if (sp.size() < 2) continue;
        const double rssBytes = double(sp[1].toULongLong()) * double(pageSize);
        const double memPct = memTotalBytes > 0.0 ? 100.0 * rssBytes / memTotalBytes : 0.0;
        QString name = readAll(QStringLiteral("/proc/%1/comm").arg(e)).trimmed();
        procs.append({pid, name, memPct});
    }
    m_taskCount = count;

    std::sort(procs.begin(), procs.end(),
              [](const Proc& a, const Proc& b) { return a.mem > b.mem; });

    QVariantList out;
    for (int i = 0; i < procs.size() && i < 5; ++i) {
        QVariantMap m;
        m[QStringLiteral("pid")] = procs[i].pid;
        m[QStringLiteral("name")] = procs[i].name;
        m[QStringLiteral("mem")] = procs[i].mem;
        out.append(m);
    }
    m_topProcesses = out;
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
