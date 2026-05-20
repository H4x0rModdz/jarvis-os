#pragma once

#include <QObject>
#include <QString>
#include <QTimer>
#include <QVariantList>
#include <qqmlintegration.h>

/// Bridge to wlr-randr for output configuration on wlroots-based
/// compositors (labwc / the future Jarvis compositor). Lets the
/// user enable/disable monitors, pick resolution, set scale —
/// without dropping to a terminal.
///
/// V1: query + simple toggles. Multi-monitor positioning ("right
/// of HDMI-1" semantics) is exposed via raw `setPosition(x, y)`
/// — a drag-to-arrange UI is V2.
///
/// Exposes:
///   - `outputs` — `[{name, description, enabled, modes,
///                   currentMode, scale, position, isPrimary}, …]`
///                 `modes` is `["1920x1080@60", …]`.
///   - `busy`
///   - `lastError`
class DisplayBridge : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    Q_PROPERTY(QVariantList outputs READ outputs NOTIFY outputsChanged)
    Q_PROPERTY(bool busy READ busy NOTIFY busyChanged)
    Q_PROPERTY(QString lastError READ lastError NOTIFY lastErrorChanged)

public:
    explicit DisplayBridge(QObject* parent = nullptr);

    QVariantList outputs() const { return m_outputs; }
    bool busy() const { return m_busy; }
    QString lastError() const { return m_lastError; }

    Q_INVOKABLE void setEnabled(const QString& output, bool enabled);
    Q_INVOKABLE void setMode(const QString& output, const QString& mode);
    Q_INVOKABLE void setScale(const QString& output, double scale);
    Q_INVOKABLE void setPosition(const QString& output, int x, int y);
    Q_INVOKABLE void refresh();

signals:
    void outputsChanged();
    void busyChanged();
    void lastErrorChanged();

private:
    void runWlrRandr(const QStringList& args,
                     std::function<void(int, const QString&, const QString&)> on_done);
    void parseList(const QString& output);
    void setError(const QString& msg);
    void setBusy(bool v);

    QVariantList m_outputs;
    bool m_busy = false;
    QString m_lastError;
};
