#pragma once

#include <QObject>
#include <QString>
#include <QTimer>
#include <QVariantList>
#include <qqmlintegration.h>

/// Bridge to PulseAudio / PipeWire via `pactl`. Lets the user pick
/// which output device receives audio without dropping to a CLI —
/// the laptop case where you plug headphones and want the sound to
/// follow them.
///
/// V1: sinks only (outputs). Sources (mic input) reachable through
/// the existing mic-mute hardware key + voice daemon — no panel.
/// V2 candidate: per-app stream routing.
///
/// Exposes:
///   - `sinks` — `[{name, description, volume, mute, isDefault}, …]`
///   - `defaultSink` — short name of the current default
///   - `busy` — pactl call in flight
///   - `lastError`
class AudioBridge : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    Q_PROPERTY(QVariantList sinks READ sinks NOTIFY sinksChanged)
    Q_PROPERTY(QString defaultSink READ defaultSink NOTIFY sinksChanged)
    Q_PROPERTY(bool busy READ busy NOTIFY busyChanged)
    Q_PROPERTY(QString lastError READ lastError NOTIFY lastErrorChanged)

public:
    explicit AudioBridge(QObject* parent = nullptr);

    QVariantList sinks() const { return m_sinks; }
    QString defaultSink() const { return m_defaultSink; }
    bool busy() const { return m_busy; }
    QString lastError() const { return m_lastError; }

    /// Switch the default sink and migrate every running stream to
    /// it (so currently playing audio actually follows the change).
    Q_INVOKABLE void setDefaultSink(const QString& sinkName);

    /// 0..100 volume on a specific sink.
    Q_INVOKABLE void setVolume(const QString& sinkName, int percent);

    Q_INVOKABLE void setMute(const QString& sinkName, bool muted);

    /// Manual refresh — settings panel calls this on open.
    Q_INVOKABLE void refresh();

signals:
    void sinksChanged();
    void busyChanged();
    void lastErrorChanged();

private slots:
    void poll();

private:
    void runPactl(const QStringList& args,
                  std::function<void(int, const QString&, const QString&)> on_done);
    void parseSinks(const QString& list, const QString& defaultName);
    void setError(const QString& msg);
    void setBusy(bool v);

    QTimer m_pollTimer;
    QVariantList m_sinks;
    QString m_defaultSink;
    bool m_busy = false;
    QString m_lastError;
};
