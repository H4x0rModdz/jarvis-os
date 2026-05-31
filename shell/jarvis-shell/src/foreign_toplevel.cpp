#include "foreign_toplevel.h"

#include <QGuiApplication>
#include <QLoggingCategory>

#include <wayland-client.h>
#include <QtGui/qguiapplication_platform.h>

namespace {
Q_LOGGING_CATEGORY(lcFt, "jarvis.shell.foreigntoplevel")

/// The compositor's wl_seat, needed by `activate`. Pulled from Qt's own
/// Wayland connection so we share the session's seat rather than opening
/// a second one.
wl_seat* sessionSeat()
{
    if (auto* app = qGuiApp) {
        if (auto* wl = app->nativeInterface<QNativeInterface::QWaylandApplication>()) {
            return wl->seat();
        }
    }
    return nullptr;
}
}

// ─────────────────────────── ForeignToplevelHandle ───────────────────────────

ForeignToplevelHandle::ForeignToplevelHandle(::zwlr_foreign_toplevel_handle_v1* object)
    : QtWayland::zwlr_foreign_toplevel_handle_v1(object)
{
}

ForeignToplevelHandle::~ForeignToplevelHandle() = default;

void ForeignToplevelHandle::zwlr_foreign_toplevel_handle_v1_title(const QString& title)
{
    m_pendingTitle = title;
}

void ForeignToplevelHandle::zwlr_foreign_toplevel_handle_v1_app_id(const QString& app_id)
{
    m_pendingAppId = app_id;
}

void ForeignToplevelHandle::zwlr_foreign_toplevel_handle_v1_state(wl_array* state)
{
    // The state event always carries the full set, so reset and rebuild.
    m_pendingActivated = false;
    m_pendingMinimized = false;
    if (!state || !state->data) {
        return;
    }
    const auto* values = static_cast<const uint32_t*>(state->data);
    const size_t n = state->size / sizeof(uint32_t);
    for (size_t i = 0; i < n; ++i) {
        switch (values[i]) {
        case QtWayland::zwlr_foreign_toplevel_handle_v1::state_activated:
            m_pendingActivated = true;
            break;
        case QtWayland::zwlr_foreign_toplevel_handle_v1::state_minimized:
            m_pendingMinimized = true;
            break;
        default:
            break;
        }
    }
}

void ForeignToplevelHandle::zwlr_foreign_toplevel_handle_v1_done()
{
    const bool dirty = m_pendingTitle != m_title || m_pendingAppId != m_appId
                       || m_pendingActivated != m_activated
                       || m_pendingMinimized != m_minimized;
    m_title = m_pendingTitle;
    m_appId = m_pendingAppId;
    m_activated = m_pendingActivated;
    m_minimized = m_pendingMinimized;
    if (dirty) {
        emit changed();
    }
}

void ForeignToplevelHandle::zwlr_foreign_toplevel_handle_v1_closed()
{
    emit closedNotified(this);
}

// ─────────────────────────── ForeignToplevelManager ──────────────────────────

ForeignToplevelManager::ForeignToplevelManager()
    : QWaylandClientExtensionTemplate<ForeignToplevelManager>(3)
{
}

void ForeignToplevelManager::zwlr_foreign_toplevel_manager_v1_toplevel(
    ::zwlr_foreign_toplevel_handle_v1* toplevel)
{
    emit toplevelCreated(new ForeignToplevelHandle(toplevel));
}

// ─────────────────────────────── RunningAppsModel ────────────────────────────

RunningAppsModel::RunningAppsModel(QObject* parent) : QAbstractListModel(parent)
{
    m_manager = new ForeignToplevelManager();
    m_manager->setParent(this);
    connect(m_manager, &ForeignToplevelManager::toplevelCreated,
            this, &RunningAppsModel::onToplevelCreated);
}

int RunningAppsModel::rowCount(const QModelIndex& parent) const
{
    return parent.isValid() ? 0 : m_handles.size();
}

QVariant RunningAppsModel::data(const QModelIndex& index, int role) const
{
    if (!index.isValid() || index.row() < 0 || index.row() >= m_handles.size()) {
        return {};
    }
    const ForeignToplevelHandle* h = m_handles.at(index.row());
    switch (role) {
    case AppIdRole:
        return h->appId();
    case TitleRole:
        return h->title();
    case ActivatedRole:
        return h->isActivated();
    case MinimizedRole:
        return h->isMinimized();
    default:
        return {};
    }
}

QHash<int, QByteArray> RunningAppsModel::roleNames() const
{
    return {
        { AppIdRole, "appId" },
        { TitleRole, "title" },
        { ActivatedRole, "activated" },
        { MinimizedRole, "minimized" },
    };
}

bool RunningAppsModel::isRunning(const QString& desktopId) const
{
    for (const ForeignToplevelHandle* h : m_handles) {
        if (matches(h->appId(), desktopId)) {
            return true;
        }
    }
    return false;
}

void RunningAppsModel::activateApp(const QString& desktopId)
{
    for (ForeignToplevelHandle* h : m_handles) {
        if (matches(h->appId(), desktopId)) {
            activateHandle(h);
            return;
        }
    }
}

void RunningAppsModel::onToplevelCreated(ForeignToplevelHandle* handle)
{
    connect(handle, &ForeignToplevelHandle::changed,
            this, &RunningAppsModel::onHandleChanged);
    connect(handle, &ForeignToplevelHandle::closedNotified,
            this, &RunningAppsModel::onHandleClosed);

    beginInsertRows(QModelIndex(), m_handles.size(), m_handles.size());
    m_handles.append(handle);
    endInsertRows();
    bump();
}

void RunningAppsModel::onHandleChanged()
{
    auto* handle = qobject_cast<ForeignToplevelHandle*>(sender());
    if (!handle) {
        return;
    }
    const int row = m_handles.indexOf(handle);
    if (row >= 0) {
        const QModelIndex idx = index(row);
        emit dataChanged(idx, idx);
    }
    bump();
}

void RunningAppsModel::onHandleClosed(ForeignToplevelHandle* handle)
{
    const int row = m_handles.indexOf(handle);
    if (row < 0) {
        return;
    }
    beginRemoveRows(QModelIndex(), row, row);
    m_handles.remove(row);
    endRemoveRows();
    handle->deleteLater();
    bump();
}

bool RunningAppsModel::matches(const QString& appId, const QString& desktopId)
{
    if (appId.isEmpty() || desktopId.isEmpty()) {
        return false;
    }
    const QString a = appId.toLower();
    const QString d = desktopId.toLower();
    if (a == d) {
        return true;
    }
    // org.mozilla.firefox <-> firefox, dev.zed.Zed <-> zed, etc.
    return a.section(QLatin1Char('.'), -1) == d.section(QLatin1Char('.'), -1);
}

void RunningAppsModel::activateHandle(ForeignToplevelHandle* handle)
{
    if (handle->isMinimized()) {
        handle->unset_minimized();
    }
    if (wl_seat* seat = sessionSeat()) {
        handle->activate(seat);
    } else {
        qCWarning(lcFt) << "No wl_seat available — cannot activate" << handle->appId();
    }
}

void RunningAppsModel::bump()
{
    ++m_revision;
    emit changed();
}
