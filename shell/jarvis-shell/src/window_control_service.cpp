#include "window_control_service.h"

#include "foreign_toplevel.h"

#include <QGuiApplication>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLoggingCategory>

#include <wayland-client.h>
#include <QtGui/qguiapplication_platform.h>

namespace {
Q_LOGGING_CATEGORY(lcWc, "jarvis.shell.windowcontrol")

/// The compositor's wl_seat, needed by `activate`. Shared from Qt's own
/// Wayland connection (same approach as foreign_toplevel.cpp) rather than
/// opening a second one.
wl_seat* sessionSeat()
{
    if (auto* app = qGuiApp) {
        if (auto* wl = app->nativeInterface<QNativeInterface::QWaylandApplication>()) {
            return wl->seat();
        }
    }
    return nullptr;
}

bool wantsActive(const QString& target)
{
    const QString t = target.trimmed().toLower();
    return t.isEmpty() || t == QLatin1String("active") || t == QLatin1String("focused")
           || t == QLatin1String("focado") || t == QLatin1String("atual")
           || t == QLatin1String("essa") || t == QLatin1String("esta");
}
} // namespace

WindowControlService::WindowControlService(QObject* parent) : QObject(parent)
{
    m_manager = new ForeignToplevelManager();
    m_manager->setParent(this);
    connect(m_manager, &ForeignToplevelManager::toplevelCreated,
            this, &WindowControlService::onToplevelCreated);
}

void WindowControlService::onToplevelCreated(ForeignToplevelHandle* handle)
{
    connect(handle, &ForeignToplevelHandle::closedNotified,
            this, &WindowControlService::onHandleClosed);
    m_handles.append(handle);
}

void WindowControlService::onHandleClosed(ForeignToplevelHandle* handle)
{
    // This service owns its own manager + handles (separate from the dock),
    // so it deletes its own copy when the compositor reports the window gone.
    m_handles.removeOne(handle);
    handle->deleteLater();
}

bool WindowControlService::appMatches(const QString& appId, const QString& target)
{
    if (appId.isEmpty() || target.isEmpty()) {
        return false;
    }
    const QString a = appId.toLower();
    const QString d = target.toLower();
    if (a == d) {
        return true;
    }
    // org.mozilla.firefox <-> firefox, dev.zed.Zed <-> zed (same rule the
    // dock uses in RunningAppsModel::matches).
    return a.section(QLatin1Char('.'), -1) == d.section(QLatin1Char('.'), -1);
}

ForeignToplevelHandle* WindowControlService::pick(const QString& target) const
{
    if (wantsActive(target)) {
        for (ForeignToplevelHandle* h : m_handles) {
            if (h->isActivated()) {
                return h;
            }
        }
        // Nothing reports activated (e.g. focus on a layer-shell surface) —
        // fall back to the most recently mapped window.
        return m_handles.isEmpty() ? nullptr : m_handles.last();
    }

    const QString t = target.trimmed();
    // App-id match first (normalised), then title substring.
    for (ForeignToplevelHandle* h : m_handles) {
        if (appMatches(h->appId(), t)) {
            return h;
        }
    }
    for (ForeignToplevelHandle* h : m_handles) {
        if (!t.isEmpty() && h->title().contains(t, Qt::CaseInsensitive)) {
            return h;
        }
    }
    return nullptr;
}

bool WindowControlService::Focus(const QString& target)
{
    ForeignToplevelHandle* h = pick(target);
    if (!h) {
        return false;
    }
    if (h->isMinimized()) {
        h->unset_minimized();
    }
    wl_seat* seat = sessionSeat();
    if (!seat) {
        qCWarning(lcWc) << "no wl_seat — cannot focus" << target;
        return false;
    }
    h->activate(seat);
    return true;
}

bool WindowControlService::Minimize(const QString& target)
{
    ForeignToplevelHandle* h = pick(target);
    if (!h) {
        return false;
    }
    h->set_minimized();
    return true;
}

bool WindowControlService::Maximize(const QString& target)
{
    ForeignToplevelHandle* h = pick(target);
    if (!h) {
        return false;
    }
    h->set_maximized();
    return true;
}

bool WindowControlService::Close(const QString& target)
{
    ForeignToplevelHandle* h = pick(target);
    if (!h) {
        return false;
    }
    h->close();
    return true;
}

QString WindowControlService::List()
{
    QJsonArray arr;
    for (const ForeignToplevelHandle* h : m_handles) {
        QJsonObject o;
        o[QStringLiteral("app_id")] = h->appId();
        o[QStringLiteral("title")] = h->title();
        o[QStringLiteral("activated")] = h->isActivated();
        o[QStringLiteral("minimized")] = h->isMinimized();
        arr.append(o);
    }
    return QString::fromUtf8(QJsonDocument(arr).toJson(QJsonDocument::Compact));
}
