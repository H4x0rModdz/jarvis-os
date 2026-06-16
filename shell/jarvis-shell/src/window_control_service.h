#pragma once

#include <QObject>
#include <QString>
#include <QVector>

class ForeignToplevelManager;
class ForeignToplevelHandle;

/// DBus-served window control for the labwc session (ADR 0025).
///
/// Owns its OWN wlr-foreign-toplevel client (separate from the dock's
/// RunningAppsModel) and exposes focus / minimize / maximize / close over
/// `com.jarvis.Shell` so the Action Bus — window.{focus,minimize,maximize,
/// close} — can drive windows without opening a Wayland client of its own.
///
/// Selection is by `target` string: "active"/"focused" (or empty) → the
/// focused window; otherwise an app-id match (normalised like the dock:
/// `org.mozilla.firefox` ↔ `firefox`) or a title substring. The shell is
/// the de-facto window manager today (ADR 0024), so it owns this surface;
/// when the Smithay compositor lands it registers real window.* handlers in
/// the bus Registry and this path is removed.
class WindowControlService : public QObject
{
    Q_OBJECT
    Q_CLASSINFO("D-Bus Interface", "com.jarvis.Shell.Windows")
public:
    explicit WindowControlService(QObject* parent = nullptr);

public slots:
    /// Unminimize (if needed) + activate the matched window. false = no match.
    bool Focus(const QString& target);
    bool Minimize(const QString& target);
    bool Maximize(const QString& target);
    bool Close(const QString& target);
    /// JSON array of {app_id, title, activated, minimized} for every window.
    QString List();

private slots:
    void onToplevelCreated(ForeignToplevelHandle* handle);
    void onHandleClosed(ForeignToplevelHandle* handle);

private:
    ForeignToplevelHandle* pick(const QString& target) const;
    static bool appMatches(const QString& appId, const QString& target);

    ForeignToplevelManager* m_manager = nullptr;
    QVector<ForeignToplevelHandle*> m_handles;
};
