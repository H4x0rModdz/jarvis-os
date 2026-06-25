#pragma once

#include <QObject>
#include <QString>
#include <QTimer>
#include <qqmlintegration.h>

/// Now-playing media for the desktop HUD (ADR 0031). Wraps `playerctl` (which
/// the image already ships) so the NETWORK panel can show the current track +
/// prev/play-pause/next controls. playerctl picks the active MPRIS player on
/// its own, so this stays trivial: poll its metadata, expose it, and run the
/// transport commands on demand.
///
/// One `updated()` signal per change; `hasPlayer` is false when nothing is
/// playing (playerctl exits non-zero) — the HUD then shows "Nada tocando".
class MediaBridge : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON

    Q_PROPERTY(bool hasPlayer READ hasPlayer NOTIFY updated)
    Q_PROPERTY(QString title READ title NOTIFY updated)
    Q_PROPERTY(QString artist READ artist NOTIFY updated)
    /// MPRIS PlaybackStatus: "Playing" | "Paused" | "Stopped".
    Q_PROPERTY(QString status READ status NOTIFY updated)

public:
    explicit MediaBridge(QObject* parent = nullptr);

    bool hasPlayer() const { return m_hasPlayer; }
    QString title() const { return m_title; }
    QString artist() const { return m_artist; }
    QString status() const { return m_status; }

    Q_INVOKABLE void previous();
    Q_INVOKABLE void playPause();
    Q_INVOKABLE void next();

signals:
    void updated();

private:
    void tick();

    QTimer m_timer;
    bool m_hasPlayer = false;
    QString m_title;
    QString m_artist;
    QString m_status;
};
