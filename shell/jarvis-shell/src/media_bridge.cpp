#include "media_bridge.h"

#include <QStringList>

namespace {
constexpr const char* kPlayerctl = "playerctl";
} // namespace

MediaBridge::MediaBridge(QObject* parent) : QObject(parent)
{
    // Event-driven, not polled. `playerctl --follow` stays running and prints
    // one line each time the active player's status or track changes (an empty
    // line when the last player goes away), so an idle desktop costs nothing.
    //
    // This used to spawn playerctl every 2 s and block on it with
    // waitForFinished() — on the GUI thread. Every fork+exec+DBus round trip
    // was a stall in which no window of the shell could paint or take input:
    // a guaranteed hitch every two seconds, for a line of text that changes a
    // few times an hour.
    m_follow.setProgram(QString::fromLatin1(kPlayerctl));
    m_follow.setArguments({QStringLiteral("metadata"),
                           QStringLiteral("--format"),
                           QStringLiteral("{{status}}\t{{title}}\t{{artist}}"),
                           QStringLiteral("--follow")});
    QObject::connect(&m_follow, &QProcess::readyReadStandardOutput, this,
                     &MediaBridge::readFollowOutput);
    QObject::connect(&m_follow, &QProcess::finished, this, [this] {
        // playerctl exited (crash, session bus restart): the last line we saw
        // no longer describes anything, and the follower needs to come back.
        setNowPlaying(false, {}, {}, {});
        m_restart.start();
    });
    // A FailedToStart error (playerctl not installed) never reaches `finished`,
    // so it is not retried; the HUD just keeps showing "Nada tocando".
    m_restart.setSingleShot(true);
    m_restart.setInterval(5000);
    QObject::connect(&m_restart, &QTimer::timeout, this, [this] { m_follow.start(); });

    m_follow.start();
}

MediaBridge::~MediaBridge()
{
    // Don't let the `finished` handler re-arm the restart timer mid-teardown.
    m_follow.disconnect(this);
    m_follow.kill();
    m_follow.waitForFinished(200);
}

void MediaBridge::readFollowOutput()
{
    // Only the newest complete line matters — a burst of changes collapses to
    // the final state.
    QByteArray last;
    bool any = false;
    while (m_follow.canReadLine()) {
        last = m_follow.readLine();
        any = true;
    }
    if (!any) return;

    // Strip the newline only: trimming all whitespace would eat the tabs of a
    // line whose status is set but whose title/artist are empty.
    while (last.endsWith('\n') || last.endsWith('\r')) last.chop(1);
    const QStringList f = QString::fromUtf8(last).split('\t');
    const QString status = f.value(0).trimmed();
    // An empty line (or empty status) is playerctl saying "no player".
    setNowPlaying(!status.isEmpty(), status, f.value(1).trimmed(), f.value(2).trimmed());
}

void MediaBridge::setNowPlaying(bool hasPlayer, const QString& status,
                                const QString& title, const QString& artist)
{
    if (m_hasPlayer == hasPlayer && m_status == status && m_title == title
        && m_artist == artist) {
        return;
    }
    m_hasPlayer = hasPlayer;
    m_status = status;
    m_title = title;
    m_artist = artist;
    emit updated();
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
