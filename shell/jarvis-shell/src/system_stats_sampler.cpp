#include "system_stats_sampler.h"

#include <QDir>
#include <QFile>
#include <QTimer>
#include <QVariantMap>

#include <algorithm>
#include <arpa/inet.h>
#include <ifaddrs.h>
#include <netinet/in.h>
#include <sys/socket.h>
#include <sys/statvfs.h>
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

SystemStatsSampler::SystemStatsSampler(QObject* parent) : QObject(parent) {}

void SystemStatsSampler::start()
{
    findTempSensors();

    // One-second cadence. First sample has no deltas yet (CPU/net read 0),
    // which is correct.
    auto* timer = new QTimer(this);
    timer->setInterval(1000);
    QObject::connect(timer, &QTimer::timeout, this, &SystemStatsSampler::sample);
    timer->start();
    sample();
}

void SystemStatsSampler::sample()
{
    StatsSample s;
    sampleCpu(s);
    sampleMem(s);
    sampleNet(s);
    sampleUptime(s);
    sampleProcs(s);
    sampleDisk(s);
    sampleTemp(s);
    emit sampled(s);
}

void SystemStatsSampler::sampleCpu(StatsSample& s)
{
    const QString stat = readAll(QStringLiteral("/proc/stat"));
    const QStringList lines = stat.split('\n');
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
            s.perCore.append(pct);
        }
    }
    s.cpuPercent = m_cpuPercent;
}

void SystemStatsSampler::sampleMem(StatsSample& s)
{
    const QString info = readAll(QStringLiteral("/proc/meminfo"));
    const QStringList lines = info.split('\n');
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
    if (total == 0) return;
    const quint64 avail = kb(QStringLiteral("MemAvailable:"));
    const quint64 swapT = kb(QStringLiteral("SwapTotal:"));
    const quint64 swapF = kb(QStringLiteral("SwapFree:"));
    const quint64 used = total > avail ? total - avail : 0;
    m_memTotalGiB = total / 1024.0 / 1024.0;
    s.memTotalGiB = m_memTotalGiB;
    s.memUsedGiB = used / 1024.0 / 1024.0;
    s.memPercent = int(qRound(100.0 * double(used) / double(total)));
    s.swapUsedGiB = (swapT > swapF ? swapT - swapF : 0) / 1024.0 / 1024.0;
}

void SystemStatsSampler::sampleNet(StatsSample& s)
{
    // ── Throughput: byte counters summed across non-loopback interfaces ──
    const QString dev = readAll(QStringLiteral("/proc/net/dev"));
    if (!dev.isEmpty()) {
        quint64 rx = 0, tx = 0;
        const QStringList lines = dev.split('\n');
        for (const QString& line : lines) {
            const int colon = line.indexOf(':');
            if (colon < 0) continue;
            if (line.left(colon).trimmed() == QLatin1String("lo")) continue;
            const QStringList p = line.mid(colon + 1).split(' ', Qt::SkipEmptyParts);
            if (p.size() < 9) continue;
            rx += p[0].toULongLong(); // receive bytes
            tx += p[8].toULongLong(); // transmit bytes
        }
        if (m_haveNetPrev) {
            const double secs = qMax(0.001, m_netClock.elapsed() / 1000.0);
            s.haveNetRates = true;
            s.netUpKBs = (tx > m_prevTx ? tx - m_prevTx : 0) / 1024.0 / secs;
            s.netDownKBs = (rx > m_prevRx ? rx - m_prevRx : 0) / 1024.0 / secs;
        }
        m_prevRx = rx;
        m_prevTx = tx;
        m_haveNetPrev = true;
        m_netClock.restart();
    }

    // ── Identity: the default-route interface, its gateway and IPv4 ──────
    // "Online" = a default route exists — interface-agnostic (wired OR wifi).
    // NetworkBridge is WiFi-only, so a wired machine would read OFFLINE.
    const QString route = readAll(QStringLiteral("/proc/net/route"));
    const QStringList rlines = route.split('\n');
    for (const QString& line : rlines) {
        const QStringList c = line.simplified().split(' ');
        // Iface Destination Gateway ...  — Destination 00000000 = default route.
        if (c.size() >= 3 && c[1] == QLatin1String("00000000")
            && c[0] != QLatin1String("lo")) {
            s.online = true;
            s.primaryIface = c[0];
            bool ok = false;
            const quint32 raw = c[2].toUInt(&ok, 16); // little-endian hex
            if (ok && raw != 0) {
                struct in_addr a;
                a.s_addr = raw; // already network byte order on LE hosts
                s.gateway = QString::fromLatin1(inet_ntoa(a));
            }
            break;
        }
    }

    if (s.primaryIface.isEmpty()) return;
    struct ifaddrs* ifap = nullptr;
    if (getifaddrs(&ifap) != 0) return;
    for (struct ifaddrs* ifa = ifap; ifa; ifa = ifa->ifa_next) {
        if (!ifa->ifa_addr || ifa->ifa_addr->sa_family != AF_INET) continue;
        if (s.primaryIface != QLatin1String(ifa->ifa_name)) continue;
        char buf[INET_ADDRSTRLEN] = {0};
        auto* sin = reinterpret_cast<struct sockaddr_in*>(ifa->ifa_addr);
        if (inet_ntop(AF_INET, &sin->sin_addr, buf, sizeof(buf)))
            s.ipAddress = QString::fromLatin1(buf);
        break;
    }
    freeifaddrs(ifap);
}

void SystemStatsSampler::sampleUptime(StatsSample& s)
{
    const QString up = readAll(QStringLiteral("/proc/uptime"));
    if (up.isEmpty()) return;
    qint64 t = qint64(up.split(' ', Qt::SkipEmptyParts).value(0).toDouble());
    const qint64 d = t / 86400;
    t %= 86400;
    const qint64 h = t / 3600;
    t %= 3600;
    const qint64 m = t / 60;
    s.uptimeText = QStringLiteral("%1d %2:%3")
                       .arg(d)
                       .arg(h, 2, 10, QChar('0'))
                       .arg(m, 2, 10, QChar('0'));
}

void SystemStatsSampler::sampleProcs(StatsSample& s)
{
    // Task count comes from /proc/loadavg's "running/total" field — one read,
    // every sample.
    const QString load = readAll(QStringLiteral("/proc/loadavg"));
    const QStringList lf = load.split(' ', Qt::SkipEmptyParts);
    if (lf.size() >= 4) {
        const int slash = lf[3].indexOf('/');
        if (slash >= 0) s.taskCount = lf[3].mid(slash + 1).toInt();
    }

    // The top-5-by-memory table: two file reads per process, so ~800
    // open/read/close per pass on a desktop with ~400 tasks. It is a glance
    // widget, not a profiler — every few seconds is plenty.
    if (++m_procScanTick < kProcScanEvery) return;
    m_procScanTick = 0;

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
    for (const QString& e : entries) {
        bool ok = false;
        const int pid = e.toInt(&ok);
        if (!ok) continue; // only numeric pid dirs
        const QString statm = readAll(QStringLiteral("/proc/%1/statm").arg(e));
        if (statm.isEmpty()) continue;
        const QStringList sp = statm.split(' ', Qt::SkipEmptyParts);
        if (sp.size() < 2) continue;
        const double rssBytes = double(sp[1].toULongLong()) * double(pageSize);
        const double memPct = memTotalBytes > 0.0 ? 100.0 * rssBytes / memTotalBytes : 0.0;
        procs.append({pid, QString(), memPct});
    }

    // Only the five that make the table need a name — not all ~400.
    const int top = int(qMin(qsizetype(5), procs.size()));
    std::partial_sort(procs.begin(), procs.begin() + top, procs.end(),
                      [](const Proc& a, const Proc& b) { return a.mem > b.mem; });

    s.haveTopProcesses = true;
    for (int i = 0; i < top; ++i) {
        QVariantMap m;
        m[QStringLiteral("pid")] = procs[i].pid;
        m[QStringLiteral("name")] =
            readAll(QStringLiteral("/proc/%1/comm").arg(procs[i].pid)).trimmed();
        m[QStringLiteral("mem")] = procs[i].mem;
        s.topProcesses.append(m);
    }
}

void SystemStatsSampler::sampleDisk(StatsSample& s)
{
    // df-style: used = total - free(incl. root-reserved); percent is used over
    // used+available-to-us, matching what the user sees in `df`.
    auto usage = [&s](const char* path) {
        struct statvfs vfs;
        if (statvfs(path, &vfs) != 0) return;
        const double block = double(vfs.f_frsize);
        const double totalB = double(vfs.f_blocks) * block;
        const double freeB = double(vfs.f_bfree) * block;   // incl. reserved
        const double availB = double(vfs.f_bavail) * block;  // available to us
        if (totalB <= 0.0) return;
        const double usedB = totalB - freeB;
        const double denom = usedB + availB;
        s.diskTotalGiB = totalB / 1024.0 / 1024.0 / 1024.0;
        s.diskUsedGiB = usedB / 1024.0 / 1024.0 / 1024.0;
        s.diskPercent = denom > 0.0 ? int(qRound(100.0 * usedB / denom)) : 0;
    };
    // "/" on bootc is a tiny read-only composefs overlay (statvfs reports ~0),
    // which is useless as a "disk" gauge. The real writable store — where OTA
    // images, ollama models and logs live and where ENOSPC actually bites — is
    // /var. Fall back to "/" if /var isn't a separate mount (non-bootc host).
    usage("/var");
    if (s.diskTotalGiB <= 0.0) usage("/");
}

void SystemStatsSampler::findTempSensors()
{
    // Prefer a CPU-named hwmon (k10temp/coretemp/…); fall back to every
    // sensor there is, of which sampleTemp() reports the hottest.
    QDir hwmon(QStringLiteral("/sys/class/hwmon"));
    const QStringList mons = hwmon.entryList(QDir::Dirs | QDir::NoDotAndDotDot);
    QStringList cpu;
    QStringList any;
    for (const QString& m : mons) {
        QDir d(hwmon.filePath(m));
        const QString name = readAll(d.filePath(QStringLiteral("name"))).trimmed();
        const bool isCpu = name == QLatin1String("k10temp")
            || name == QLatin1String("coretemp") || name == QLatin1String("zenpower")
            || name == QLatin1String("cpu_thermal");
        const QStringList inputs =
            d.entryList(QStringList() << QStringLiteral("temp*_input"), QDir::Files);
        for (const QString& in : inputs) {
            any.append(d.filePath(in));
            if (isCpu) cpu.append(d.filePath(in));
        }
    }
    m_tempInputs = cpu.isEmpty() ? any : cpu;
}

void SystemStatsSampler::sampleTemp(StatsSample& s)
{
    if (m_tempInputs.isEmpty()) {
        if (++m_tempRescanTick < kTempRescanEvery) return;
        m_tempRescanTick = 0;
        findTempSensors();
    }
    // Values are millidegrees. Stays 0 when nothing is readable (a VM).
    int hottest = 0;
    for (const QString& path : std::as_const(m_tempInputs)) {
        bool ok = false;
        const int milli = readAll(path).trimmed().toInt(&ok);
        if (ok && milli > hottest) hottest = milli;
    }
    s.cpuTempC = hottest > 0 ? int(qRound(hottest / 1000.0)) : 0;
}
