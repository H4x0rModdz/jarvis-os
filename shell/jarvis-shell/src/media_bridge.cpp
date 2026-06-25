#include "media_bridge.h"

#include <QProcess>
#include <QStringList>

namespace {
constexpr const char* kPlayerctl = "playerctl";
} // namespace

MediaBridge::MediaBridge(QObject* parent) : QObject(parent)
{
    // 2 s is plenty for a now-playing line; the transport commands refresh on
    // the next tick. playerctl is fast (<50 ms), so a synchronous poll on the
    // GUI thread is fine.
    m_timer.setInterval(2000);
    QObject::connect(&m_timer, &QTimer::timeout, this, &MediaBridge::tick);
    m_timer.start();
    tick();
}

void MediaBridge::tick()
{
    QProcess p;
    // One call yields status + title + artist, tab-separated.
    p.start(QString::fromLatin1(kPlayerctl),
            {QStringLiteral("metadata"),
             QStringLiteral("--format"),
             QStringLiteral("{{status}}\t{{title}}\t{{artist}}")});
    const bool ok = p.waitForFinished(800)
        && p.exitStatus() == QProcess::NormalExit
        && p.exitCode() == 0;

    bool changed = false;
    if (ok) {
        const QString out = QString::fromUtf8(p.readAllStandardOutput()).trimmed();
        const QStringList f = out.split('\t');
        const QString st = f.value(0);
        const QString ti = f.value(1);
        const QString ar = f.value(2);
        if (!m_hasPlayer || m_status != st || m_title != ti || m_artist != ar) {
            m_hasPlayer = true;
            m_status = st;
            m_title = ti;
            m_artist = ar;
            changed = true;
        }
    } else if (m_hasPlayer) {
        // No active player (playerctl exits non-zero) or it isn't installed.
        m_hasPlayer = false;
        m_status.clear();
        m_title.clear();
        m_artist.clear();
        changed = true;
    }
    if (changed) emit updated();
}

void MediaBridge::previous()
{
    QProcess::startDetached(QString::fromLatin1(kPlayerctl), {QStringLiteral("previous")});
}

void MediaBridge::playPause()
{
    QProcess::startDetached(QString::fromLatin1(kPlayerctl), {QStringLiteral("play-pause")});
}

void MediaBridge::next()
{
    QProcess::startDetached(QString::fromLatin1(kPlayerctl), {QStringLiteral("next")});
}
