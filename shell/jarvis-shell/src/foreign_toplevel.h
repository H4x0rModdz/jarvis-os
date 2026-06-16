#pragma once

#include <QAbstractListModel>
#include <QHash>
#include <QObject>
#include <QString>
#include <QStringList>
#include <QVector>
#include <qqmlintegration.h>

#include <QtWaylandClient/QWaylandClientExtension>

#include "qwayland-wlr-foreign-toplevel-management-unstable-v1.h"

struct wl_array;
struct wl_seat;

/// One running toplevel window, as reported by the compositor through
/// `zwlr_foreign_toplevel_handle_v1`. Accumulates the latest title /
/// app_id / state and emits `changed()` once the compositor signals
/// `done` (so multi-event updates are seen atomically).
///
/// Powers the dock's running-app awareness (Phase B, ADR 0024). The
/// data flows compositor -> here -> RunningAppsModel -> Dock.qml.
class ForeignToplevelHandle : public QObject,
                              public QtWayland::zwlr_foreign_toplevel_handle_v1
{
    Q_OBJECT
public:
    explicit ForeignToplevelHandle(::zwlr_foreign_toplevel_handle_v1* object);
    ~ForeignToplevelHandle() override;

    QString appId() const { return m_appId; }
    QString title() const { return m_title; }
    bool isActivated() const { return m_activated; }
    bool isMinimized() const { return m_minimized; }

signals:
    /// Title / app_id / state settled (emitted on the protocol `done`).
    void changed();
    /// The compositor destroyed this toplevel. `self` lets the model find
    /// the row without a separate sender() lookup.
    void closedNotified(ForeignToplevelHandle* self);

protected:
    void zwlr_foreign_toplevel_handle_v1_title(const QString& title) override;
    void zwlr_foreign_toplevel_handle_v1_app_id(const QString& app_id) override;
    void zwlr_foreign_toplevel_handle_v1_state(wl_array* state) override;
    void zwlr_foreign_toplevel_handle_v1_done() override;
    void zwlr_foreign_toplevel_handle_v1_closed() override;

private:
    // Committed values (visible after `done`).
    QString m_appId;
    QString m_title;
    bool m_activated = false;
    bool m_minimized = false;
    // Pending values accumulated between `done` events.
    QString m_pendingAppId;
    QString m_pendingTitle;
    bool m_pendingActivated = false;
    bool m_pendingMinimized = false;
};

/// Binds the compositor's `zwlr_foreign_toplevel_manager_v1` global and
/// emits `toplevelCreated` for every window. Auto-binds when the global
/// is advertised — labwc implements it, so does any compositor we target.
class ForeignToplevelManager
    : public QWaylandClientExtensionTemplate<ForeignToplevelManager>,
      public QtWayland::zwlr_foreign_toplevel_manager_v1
{
    Q_OBJECT
public:
    ForeignToplevelManager();

signals:
    void toplevelCreated(ForeignToplevelHandle* handle);

protected:
    void zwlr_foreign_toplevel_manager_v1_toplevel(
        ::zwlr_foreign_toplevel_handle_v1* toplevel) override;
};

/// QML-facing model of running windows. The dock binds to it for the
/// running-indicator dot on pinned tiles and click-to-focus /
/// restore-minimized. Exposed to QML as `RunningAppsModel`.
class RunningAppsModel : public QAbstractListModel
{
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(int count READ count NOTIFY changed)
    /// Bumped on any toplevel add / remove / state change. QML bindings
    /// that call isRunning() reference `revision` so they re-evaluate as
    /// windows open and close (isRunning is a method, not a property).
    Q_PROPERTY(int revision READ revision NOTIFY changed)

public:
    enum Roles {
        AppIdRole = Qt::UserRole + 1,
        TitleRole,
        ActivatedRole,
        MinimizedRole,
    };

    explicit RunningAppsModel(QObject* parent = nullptr);

    int rowCount(const QModelIndex& parent = QModelIndex()) const override;
    QVariant data(const QModelIndex& index, int role = Qt::DisplayRole) const override;
    QHash<int, QByteArray> roleNames() const override;

    int count() const { return m_handles.size(); }
    int revision() const { return m_revision; }

    /// Does any running window correspond to this pinned desktop id?
    /// Normalised match: `org.mozilla.firefox` <-> `firefox`,
    /// `foot` <-> `foot`, `dev.zed.Zed` <-> `zed`.
    Q_INVOKABLE bool isRunning(const QString& desktopId) const;

    /// Focus (and unminimize) the first window matching this desktop id.
    Q_INVOKABLE void activateApp(const QString& desktopId);

    /// Open/minimized state for a pinned dock tile:
    ///   0 = no window for this app, 1 = at least one visible window,
    ///   2 = running but every matching window is minimized.
    /// Drives the dock's open-vs-minimized indicator.
    Q_INVOKABLE int runState(const QString& desktopId) const;

    /// Distinct app_ids of every running toplevel, so the dock can surface
    /// running apps that aren't pinned.
    Q_INVOKABLE QStringList runningAppIds() const;

signals:
    void changed();

private slots:
    void onToplevelCreated(ForeignToplevelHandle* handle);
    void onHandleChanged();
    void onHandleClosed(ForeignToplevelHandle* handle);

private:
    static bool matches(const QString& appId, const QString& desktopId);
    void activateHandle(ForeignToplevelHandle* handle);
    void bump();

    ForeignToplevelManager* m_manager = nullptr;
    QVector<ForeignToplevelHandle*> m_handles;
    int m_revision = 0;
};
