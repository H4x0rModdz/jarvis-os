#include "display_bridge.h"

#include <QLoggingCategory>
#include <QProcess>
#include <QRegularExpression>
#include <QVariantMap>

namespace {
Q_LOGGING_CATEGORY(lcDisplay, "jarvis.shell.display")
} // namespace

DisplayBridge::DisplayBridge(QObject* parent) : QObject(parent)
{
    refresh();
}

void DisplayBridge::refresh()
{
    runWlrRandr({}, [this](int code, const QString& out, const QString& err) {
        if (code != 0) {
            setError(err.trimmed());
            return;
        }
        parseList(out);
    });
}

void DisplayBridge::parseList(const QString& output)
{
    // wlr-randr output is indented: each top-level line begins a
    // new monitor block (e.g. "HDMI-A-1 \"Dell U2720Q\""), then
    // sub-keys are indented two spaces, mode lines four. Track the
    // current monitor as we walk; flush on each new top-level.
    QVariantList parsed;
    QVariantMap current;
    QVariantList modes;
    auto flush = [&]() {
        if (current.isEmpty()) return;
        current.insert(QStringLiteral("modes"), modes);
        parsed.append(current);
        current.clear();
        modes.clear();
    };
    static const QRegularExpression modeRe(
        QStringLiteral("^(\\d+)x(\\d+)\\s+px,\\s+([\\d.]+)\\s+Hz(.*)$"));
    for (const QString& raw : output.split('\n', Qt::KeepEmptyParts)) {
        const QString line = raw;
        if (line.isEmpty()) continue;
        const QString trimmed = line.trimmed();
        if (!line.startsWith(QChar(' ')) && !line.startsWith(QChar('\t'))) {
            // Top-level: "<name> [\"<description>\"]"
            flush();
            const int firstSpace = trimmed.indexOf(QChar(' '));
            const QString name = (firstSpace > 0)
                ? trimmed.left(firstSpace) : trimmed;
            QString desc;
            if (firstSpace > 0) {
                desc = trimmed.mid(firstSpace + 1).trimmed();
                if (desc.startsWith(QChar('"')) && desc.endsWith(QChar('"'))) {
                    desc = desc.mid(1, desc.size() - 2);
                }
            }
            current.insert(QStringLiteral("name"), name);
            current.insert(QStringLiteral("description"), desc);
            current.insert(QStringLiteral("enabled"), true);
            current.insert(QStringLiteral("scale"), 1.0);
            current.insert(QStringLiteral("position"),
                QVariantMap{{"x", 0}, {"y", 0}});
            continue;
        }
        // Indented line — a key or a mode.
        if (trimmed.startsWith(QStringLiteral("Enabled:"))) {
            current.insert(QStringLiteral("enabled"),
                trimmed.section(':', 1).trimmed() == QStringLiteral("yes"));
            continue;
        }
        if (trimmed.startsWith(QStringLiteral("Scale:"))) {
            current.insert(QStringLiteral("scale"),
                trimmed.section(':', 1).trimmed().toDouble());
            continue;
        }
        if (trimmed.startsWith(QStringLiteral("Position:"))) {
            const QString posStr = trimmed.section(':', 1).trimmed();
            const QStringList parts = posStr.split(QChar(','));
            if (parts.size() == 2) {
                current.insert(QStringLiteral("position"),
                    QVariantMap{
                        {"x", parts[0].trimmed().toInt()},
                        {"y", parts[1].trimmed().toInt()},
                    });
            }
            continue;
        }
        // Mode lines: "1920x1080 px, 60.000000 Hz (preferred, current)"
        const auto m = modeRe.match(trimmed);
        if (m.hasMatch()) {
            QVariantMap mode;
            const QString modeStr = QString("%1x%2@%3")
                .arg(m.captured(1))
                .arg(m.captured(2))
                .arg(static_cast<int>(m.captured(3).toDouble()));
            mode.insert(QStringLiteral("mode"), modeStr);
            const QString flags = m.captured(4).trimmed();
            const bool isCurrent = flags.contains(QStringLiteral("current"));
            const bool isPreferred = flags.contains(QStringLiteral("preferred"));
            mode.insert(QStringLiteral("current"), isCurrent);
            mode.insert(QStringLiteral("preferred"), isPreferred);
            modes.append(mode);
            if (isCurrent) {
                current.insert(QStringLiteral("currentMode"), modeStr);
            }
            continue;
        }
    }
    flush();

    if (parsed != m_outputs) {
        m_outputs = parsed;
        emit outputsChanged();
    }
}

void DisplayBridge::setEnabled(const QString& output, bool enabled)
{
    setBusy(true);
    runWlrRandr({"--output", output, enabled ? "--on" : "--off"},
        [this](int code, const QString&, const QString& err) {
            setBusy(false);
            if (code != 0) setError(err.trimmed());
            refresh();
        });
}

void DisplayBridge::setMode(const QString& output, const QString& mode)
{
    setBusy(true);
    runWlrRandr({"--output", output, "--mode", mode},
        [this](int code, const QString&, const QString& err) {
            setBusy(false);
            if (code != 0) setError(err.trimmed());
            refresh();
        });
}

void DisplayBridge::setScale(const QString& output, double scale)
{
    setBusy(true);
    runWlrRandr({"--output", output, "--scale",
                 QString::number(scale, 'f', 2)},
        [this](int code, const QString&, const QString& err) {
            setBusy(false);
            if (code != 0) setError(err.trimmed());
            refresh();
        });
}

void DisplayBridge::setPosition(const QString& output, int x, int y)
{
    setBusy(true);
    runWlrRandr({"--output", output, "--pos",
                 QString("%1,%2").arg(x).arg(y)},
        [this](int code, const QString&, const QString& err) {
            setBusy(false);
            if (code != 0) setError(err.trimmed());
            refresh();
        });
}

void DisplayBridge::runWlrRandr(const QStringList& args,
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
    proc->start(QStringLiteral("wlr-randr"), args);
}

void DisplayBridge::setError(const QString& msg)
{
    if (msg == m_lastError) return;
    m_lastError = msg;
    emit lastErrorChanged();
    if (!msg.isEmpty()) {
        qCWarning(lcDisplay) << "wlr-randr error:" << msg;
    }
}

void DisplayBridge::setBusy(bool v)
{
    if (m_busy == v) return;
    m_busy = v;
    emit busyChanged();
}
