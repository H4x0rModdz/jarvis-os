#include "system_stats_bridge.h"

#include <QDir>
#include <QFile>
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

SystemStatsBridge::SystemStatsBridge(QObject* parent) : QObject(parent)
{
    m_cpuModel = readCpuModel();
    m_osRelease = readOsRelease();

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
    sampleDisk();
    sampleTemp();
    sampleNetIdentity();
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

    // "Online" = a default route exists — interface-agnostic (wired OR wifi).
    // The HUD used to read NetworkBridge's WiFi-only activeConnection, so a
    // wired VM (VMware NAT) always showed OFFLINE.
    bool online = false;
    const QString route = readAll(QStringLiteral("/proc/net/route"));
    const QStringList rlines = route.split('\n');
    for (const QString& line : rlines) {
        const QStringList c = line.simplified().split(' ');
        // Columns: Iface Destination Gateway ... — Destination 00000000 = default.
        if (c.size() >= 2 && c[1] == QLatin1String("00000000")
            && c[0] != QLatin1String("lo")) {
            online = true;
            break;
        }
    }
    m_online = online;
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

void SystemStatsBridge::sampleDisk()
{
    // df-style: used = total - free(incl. root-reserved); percent is used over
    // used+available-to-us, matching what the user sees in `df`.
    auto usage = [](const QString& path, double& usedGiB, double& totalGiB, int& pct) {
        struct statvfs vfs;
        if (statvfs(path.toLocal8Bit().constData(), &vfs) != 0) return;
        const double block = double(vfs.f_frsize);
        const double totalB = double(vfs.f_blocks) * block;
        const double freeB = double(vfs.f_bfree) * block;   // incl. reserved
        const double availB = double(vfs.f_bavail) * block;  // available to us
        if (totalB <= 0.0) return;
        const double usedB = totalB - freeB;
        const double denom = usedB + availB;
        totalGiB = totalB / 1024.0 / 1024.0 / 1024.0;
        usedGiB = usedB / 1024.0 / 1024.0 / 1024.0;
        pct = denom > 0.0 ? int(qRound(100.0 * usedB / denom)) : 0;
    };
    // "/" on bootc is a tiny read-only composefs overlay (statvfs reports ~0),
    // which is useless as a "disk" gauge. The real writable store — where OTA
    // images, ollama models and logs live and where ENOSPC actually bites — is
    // /var. Fall back to "/" if /var isn't a separate mount (non-bootc host).
    usage(QStringLiteral("/var"), m_diskUsedGiB, m_diskTotalGiB, m_diskPercent);
    if (m_diskTotalGiB <= 0.0)
        usage(QStringLiteral("/"), m_diskUsedGiB, m_diskTotalGiB, m_diskPercent);
    usage(QDir::homePath(), m_homeUsedGiB, m_homeTotalGiB, m_homePercent);
}

void SystemStatsBridge::sampleTemp()
{
    // Prefer a CPU-named hwmon (k10temp/coretemp/…); fall back to the hottest of
    // any sensor. Values are millidegrees. 0 when nothing is readable (VM).
    QDir hwmon(QStringLiteral("/sys/class/hwmon"));
    const QStringList mons = hwmon.entryList(QDir::Dirs | QDir::NoDotAndDotDot);
    int cpuMilli = 0;
    int anyMilli = 0;
    for (const QString& m : mons) {
        QDir d(hwmon.filePath(m));
        const QString name = readAll(d.filePath(QStringLiteral("name"))).trimmed();
        const bool isCpu = name == QLatin1String("k10temp")
            || name == QLatin1String("coretemp") || name == QLatin1String("zenpower")
            || name == QLatin1String("cpu_thermal");
        const QStringList inputs =
            d.entryList(QStringList() << QStringLiteral("temp*_input"), QDir::Files);
        for (const QString& in : inputs) {
            bool ok = false;
            const int milli = readAll(d.filePath(in)).trimmed().toInt(&ok);
            if (!ok) continue;
            if (milli > anyMilli) anyMilli = milli;
            if (isCpu && milli > cpuMilli) cpuMilli = milli;
        }
    }
    const int milli = cpuMilli > 0 ? cpuMilli : anyMilli;
    m_cpuTempC = milli > 0 ? int(qRound(milli / 1000.0)) : 0;
}

void SystemStatsBridge::sampleNetIdentity()
{
    // The default-route interface + its gateway (from /proc/net/route) and the
    // IPv4 bound to it (from getifaddrs). Interface-agnostic, like `online`.
    QString iface;
    QString gw;
    const QString route = readAll(QStringLiteral("/proc/net/route"));
    for (const QString& line : route.split('\n')) {
        const QStringList c = line.simplified().split(' ');
        // Iface Destination Gateway ...  — Destination 00000000 = default route.
        if (c.size() >= 3 && c[1] == QLatin1String("00000000")
            && c[0] != QLatin1String("lo")) {
            iface = c[0];
            bool ok = false;
            const quint32 raw = c[2].toUInt(&ok, 16); // little-endian hex
            if (ok && raw != 0) {
                struct in_addr a;
                a.s_addr = raw; // already network byte order on LE hosts
                gw = QString::fromLatin1(inet_ntoa(a));
            }
            break;
        }
    }

    QString ip;
    if (!iface.isEmpty()) {
        struct ifaddrs* ifap = nullptr;
        if (getifaddrs(&ifap) == 0) {
            for (struct ifaddrs* ifa = ifap; ifa; ifa = ifa->ifa_next) {
                if (!ifa->ifa_addr || ifa->ifa_addr->sa_family != AF_INET) continue;
                if (iface != QLatin1String(ifa->ifa_name)) continue;
                char buf[INET_ADDRSTRLEN] = {0};
                auto* sin = reinterpret_cast<struct sockaddr_in*>(ifa->ifa_addr);
                if (inet_ntop(AF_INET, &sin->sin_addr, buf, sizeof(buf)))
                    ip = QString::fromLatin1(buf);
                break;
            }
            freeifaddrs(ifap);
        }
    }

    m_primaryIface = iface;
    m_gateway = gw;
    m_ipAddress = ip;
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
