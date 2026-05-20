#include "audio_bridge.h"

#include <QLoggingCategory>
#include <QProcess>
#include <QRegularExpression>
#include <QVariantMap>

namespace {
Q_LOGGING_CATEGORY(lcAudio, "jarvis.shell.audio")

/// Extract the first percentage (e.g. "75%") from a pactl volume
/// line. pactl reports per-channel volumes — we take the first one
/// because the slider is mono anyway.
int firstPercent(const QString& s)
{
    static const QRegularExpression re(QStringLiteral("(\\d+)%"));
    const auto m = re.match(s);
    return m.hasMatch() ? m.captured(1).toInt() : 0;
}
} // namespace

AudioBridge::AudioBridge(QObject* parent) : QObject(parent)
{
    // 3 s refresh — sink list changes when the user plugs / unplugs
    // headphones; not worth polling more aggressively.
    m_pollTimer.setInterval(3000);
    QObject::connect(&m_pollTimer, &QTimer::timeout, this, &AudioBridge::poll);
    m_pollTimer.start();
    refresh();
}

void AudioBridge::refresh()
{
    // Step 1: get the default sink name.
    runPactl({"get-default-sink"},
        [this](int code, const QString& out, const QString&) {
            const QString defaultName = (code == 0) ? out.trimmed() : QString();
            // Step 2: get the full sink list.
            runPactl({"list", "sinks"},
                [this, defaultName](int code, const QString& list, const QString&) {
                    if (code != 0) return;
                    parseSinks(list, defaultName);
                });
        });
}

void AudioBridge::poll()
{
    refresh();
}

void AudioBridge::parseSinks(const QString& list, const QString& defaultName)
{
    // `pactl list sinks` prints blocks separated by blank lines.
    // Each block has lines like:
    //   Sink #42
    //     Name: alsa_output.pci-...
    //     Description: Built-in Audio
    //     Mute: no
    //     Volume: front-left: 65536 / 100% / 0.00 dB, …
    QVariantList parsed;
    QVariantMap current;
    auto flush = [&]() {
        if (!current.isEmpty()) {
            current.insert(
                QStringLiteral("isDefault"),
                current.value(QStringLiteral("name")).toString() == defaultName);
            parsed.append(current);
            current.clear();
        }
    };
    for (const QString& raw : list.split('\n', Qt::KeepEmptyParts)) {
        const QString line = raw.trimmed();
        if (line.isEmpty()) continue;
        if (line.startsWith(QStringLiteral("Sink #"))) {
            flush();
            current.clear();
        } else if (line.startsWith(QStringLiteral("Name:"))) {
            current.insert(QStringLiteral("name"),
                line.section(':', 1).trimmed());
        } else if (line.startsWith(QStringLiteral("Description:"))) {
            current.insert(QStringLiteral("description"),
                line.section(':', 1).trimmed());
        } else if (line.startsWith(QStringLiteral("Mute:"))) {
            current.insert(QStringLiteral("mute"),
                line.section(':', 1).trimmed() == QStringLiteral("yes"));
        } else if (line.startsWith(QStringLiteral("Volume:"))) {
            current.insert(QStringLiteral("volume"), firstPercent(line));
        }
    }
    flush();

    bool changed = parsed != m_sinks || defaultName != m_defaultSink;
    if (changed) {
        m_sinks = parsed;
        m_defaultSink = defaultName;
        emit sinksChanged();
    }
}

void AudioBridge::setDefaultSink(const QString& sinkName)
{
    setBusy(true);
    runPactl({"set-default-sink", sinkName},
        [this, sinkName](int code, const QString&, const QString& err) {
            if (code != 0) {
                setError(err.trimmed());
                setBusy(false);
                return;
            }
            // Move every running stream to the new default so
            // currently-playing audio actually follows the switch.
            // pactl list short sink-inputs prints "id ..." rows.
            runPactl({"list", "short", "sink-inputs"},
                [this, sinkName](int code, const QString& list, const QString&) {
                    if (code == 0) {
                        for (const QString& row : list.split('\n', Qt::SkipEmptyParts)) {
                            const QString id = row.section('\t', 0, 0).trimmed();
                            if (id.isEmpty()) continue;
                            runPactl({"move-sink-input", id, sinkName},
                                [](int, const QString&, const QString&) {});
                        }
                    }
                    setBusy(false);
                    refresh();
                });
        });
}

void AudioBridge::setVolume(const QString& sinkName, int percent)
{
    const int clamped = std::clamp(percent, 0, 150);
    runPactl({"set-sink-volume", sinkName, QString::number(clamped) + "%"},
        [this](int code, const QString&, const QString& err) {
            if (code != 0) setError(err.trimmed());
            refresh();
        });
}

void AudioBridge::setMute(const QString& sinkName, bool muted)
{
    runPactl({"set-sink-mute", sinkName, muted ? "1" : "0"},
        [this](int code, const QString&, const QString& err) {
            if (code != 0) setError(err.trimmed());
            refresh();
        });
}

void AudioBridge::runPactl(const QStringList& args,
                           std::function<void(int, const QString&, const QString&)> on_done)
{
    auto* proc = new QProcess(this);
    QObject::connect(proc,
        QOverload<int, QProcess::ExitStatus>::of(&QProcess::finished),
        this,
        [proc, on_done](int code, QProcess::ExitStatus) {
            const QString out = QString::fromUtf8(proc->readAllStandardOutput());
            const QString err = QString::fromUtf8(proc->readAllStandardError());
            on_done(code, out, err);
            proc->deleteLater();
        });
    QObject::connect(proc, &QProcess::errorOccurred, this,
        [proc, on_done](QProcess::ProcessError) {
            on_done(-1, {}, proc->errorString());
            proc->deleteLater();
        });
    proc->start(QStringLiteral("pactl"), args);
}

void AudioBridge::setError(const QString& msg)
{
    if (msg == m_lastError) return;
    m_lastError = msg;
    emit lastErrorChanged();
    if (!msg.isEmpty()) {
        qCWarning(lcAudio) << "pactl error:" << msg;
    }
}

void AudioBridge::setBusy(bool v)
{
    if (m_busy == v) return;
    m_busy = v;
    emit busyChanged();
}
