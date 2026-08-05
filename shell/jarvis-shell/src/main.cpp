#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QLoggingCategory>
#include <QDir>
#include <QFile>
#include <QIcon>
#include <QMargins>
#include <QStandardPaths>
#include <QUrl>
#include <QDBusConnection>
#include <QDBusError>

#include "escape_closer.h"
#include "icon_image_provider.h"
#include "window_control_service.h"

#ifdef JARVIS_HAVE_LAYER_SHELL
#  include <LayerShellQt/Shell>
#  include <LayerShellQt/Window>
#endif

int main(int argc, char** argv)
{
#ifdef JARVIS_HAVE_LAYER_SHELL
    // Switch Qt's Wayland shell integration to wlr-layer-shell BEFORE the
    // QGuiApplication is constructed (the integration is selected during
    // QPA platform init). On non-wlroots compositors the plugin will fail
    // to load and Qt falls back to xdg-shell — the bar still appears, just
    // as a regular floating window.
    LayerShellQt::Shell::useLayerShell();
#endif

    // Bottom-of-screen shell wants smooth animations and a tablet-y feel.
    QQuickStyle::setStyle(QStringLiteral("Material"));
    qputenv("QT_QUICK_CONTROLS_MATERIAL_THEME", "Dark");

    QGuiApplication app(argc, argv);
    app.setOrganizationName(QStringLiteral("Jarvis"));
    app.setApplicationName(QStringLiteral("jarvis-shell"));

    // Escape closes the focused/visible shell panel everywhere. Per-window
    // Esc handlers don't fire for panels that never grab keyboard focus under
    // labwc, so we catch it once at the application level. See escape_closer.h.
    app.installEventFilter(new EscapeCloser(&app));

    // Single instance, enforced BEFORE any window exists.
    //
    // `com.jarvis.Shell` doubles as the lock: DBus grants the name to exactly
    // one connection and releases it automatically when that process dies, so
    // there is no stale-lockfile failure mode. Owning it late — after the QML
    // engine had already created the top bar, dock and desktop — meant a second
    // instance merely logged "window control disabled" and then kept running:
    // two full shells, each painting a fullscreen desktop layer, fighting over
    // the same layer-shell slots ("already has a shell integration") and
    // doubling every timer and repaint. On a wide display that alone is enough
    // to make the session feel broken, and it is easy to hit by launching the
    // shell from a terminal while the session's own instance is up.
    //
    // Losing the race is not an error: the desktop is already running.
    QDBusConnection bus = QDBusConnection::sessionBus();
    if (!bus.registerService(QStringLiteral("com.jarvis.Shell"))) {
        qInfo("jarvis-shell: another instance already owns com.jarvis.Shell (%s) — exiting",
              qPrintable(bus.lastError().message()));
        return 0;
    }

    // App icons. The shell is a pure Qt/Wayland app with no GTK platform
    // theme, so QIcon has no active icon theme by default and every
    // `image://theme/<name>` lookup would miss — leaving the dock/launcher on
    // their monogram fallback. Pin the theme the image actually ships
    // (WhiteSur, see iso/assets/theme/install-whitesur.sh + gtk-settings.ini)
    // and let hicolor catch the strays. Only set a name if nothing upstream
    // already did, so a future platform-theme integration wins.
    if (QIcon::themeName().isEmpty()) {
        QIcon::setThemeName(QStringLiteral("WhiteSur"));
    }
    QIcon::setFallbackThemeName(QStringLiteral("hicolor"));

    QQmlApplicationEngine engine;

    // Serve `image://theme/<name>` from the active icon theme. Registered
    // before any QML loads so the very first dock paint resolves icons.
    engine.addImageProvider(QStringLiteral("theme"), new IconImageProvider);

    // Real home dir for the desktop icons (Desktop.qml's "Pasta Pessoal"
    // opens HomePath). Resolved in C++ because xdg-open never expands a
    // shell `~`, and QML has no first-class home-path accessor without
    // pulling in Qt.labs.platform. One justified context property.
    engine.rootContext()->setContextProperty(
        QStringLiteral("HomePath"), QDir::homePath());

    // Avatar model URL for the embodied Lilith (ADR 0028). The VRM is a
    // drop-in at <data>/jarvis/avatar/lilith.vrm; resolved in C++ via
    // QStandardPaths (GenericDataLocation = ~/.local/share) for the same
    // reason as HomePath above — QML has no first-class data-dir accessor
    // without pulling in QtCore/Qt.labs.platform, and RuntimeLoader needs a
    // real file URL.
    //
    // AvatarModelPresent gates the whole 3D view: with no VRM there is nothing
    // to show, so we don't instantiate QtQuick3D at all. That matters twice
    // over — we shipped no stand-in model (the avatar is the user's own file),
    // and an idle View3D still renders every frame, which on a machine without
    // GPU acceleration is pure CPU cost for an empty scene.
    {
        const QString dataDir =
            QStandardPaths::writableLocation(QStandardPaths::GenericDataLocation);
        const QString vrmPath = dataDir + QStringLiteral("/jarvis/avatar/lilith.vrm");
        engine.rootContext()->setContextProperty(
            QStringLiteral("AvatarModelUrl"), QUrl::fromLocalFile(vrmPath).toString());
        engine.rootContext()->setContextProperty(
            QStringLiteral("AvatarModelPresent"), QFile::exists(vrmPath));
    }

    // Dev loop: set JARVIS_QML_PATH to a directory that contains the
    // built `Jarvis/Shell/` module tree (the cmake build dir works —
    // qt_add_qml_module drops the qmldir + .qml files there). With it
    // set we prepend that dir to the engine's import paths so the
    // on-disk QML shadows the qrc-compiled copies. Edit a .qml,
    // restart jarvis-shell, see the change — no C++/cmake rebuild.
    // Unset (production) → falls straight through to the embedded
    // module. tools/dev-deploy.sh wires this automatically.
    const QByteArray devQmlPath = qgetenv("JARVIS_QML_PATH");
    if (!devQmlPath.isEmpty()) {
        engine.addImportPath(QString::fromLocal8Bit(devQmlPath));
        qInfo("jarvis-shell: dev QML path active: %s", devQmlPath.constData());
    }

    // Qt 6.5+ idiom: resolve the entry point through the QML module so the
    // engine reads the generated qmldir (including `singleton Theme`) and
    // registers types with their correct semantics. Loading via a raw
    // qrc:/… URL in Qt 6.10 silently drops the singleton flag on
    // pragma-Singleton QML files — every Theme.* reference then errors with
    // "was a singleton at compile time, but is not a singleton anymore."
    // Three layer-shell roots, all rendered by this one engine. Each
    // loadFromModule appends a top-level Window to rootObjects(); the
    // loop below configures each by objectName:
    //   Main    → "jarvis-topbar"  (top menu bar)
    //   Dock    → "jarvis-dock"    (floating bottom dock + Lilith orb)
    //   Desktop → "jarvis-desktop" (desktop icons, bottom layer)
    // JARVIS_SHELL_SURFACES isolates surfaces for performance diagnosis.
    // Multiple QQuickWindows in one process share frame pacing, and both Qt's
    // own docs (threaded loop is smooth "as long as there is exactly one
    // window") and QTBUG-52372 make several windows a suspect for the
    // whole-shell stutter seen on real hardware. Running e.g.
    // `JARVIS_SHELL_SURFACES=topbar jarvis-shell` answers "is one window
    // smooth where three are not?" without a special build. Unset → all three.
    const QString surfaces = qEnvironmentVariable("JARVIS_SHELL_SURFACES",
                                                  QStringLiteral("topbar,dock,desktop"));
    if (surfaces.contains(QLatin1String("topbar")))
        engine.loadFromModule("Jarvis.Shell", "Main");
    if (surfaces.contains(QLatin1String("dock")))
        engine.loadFromModule("Jarvis.Shell", "Dock");
    if (surfaces.contains(QLatin1String("desktop")))
        engine.loadFromModule("Jarvis.Shell", "Desktop");
    if (engine.rootObjects().isEmpty()) {
        qWarning("jarvis-shell: no surfaces loaded (JARVIS_SHELL_SURFACES=%s)",
                 qPrintable(surfaces));
        return 1;
    }

#ifdef JARVIS_HAVE_LAYER_SHELL
    for (QObject* obj : engine.rootObjects()) {
        auto* win = qobject_cast<QQuickWindow*>(obj);
        if (!win) continue;
        auto* layer = LayerShellQt::Window::get(win);
        if (!layer) continue;

        const QString name = win->objectName();
        // Anchors lacks Q_DECLARE_OPERATORS_FOR_FLAGS in v6.0.0, so every
        // OR is wrapped in an explicit Anchors(...) cast.
        if (name == QLatin1String("jarvis-desktop")) {
            // Desktop icons: cover the whole output on the *bottom* layer
            // (above swaybg's wallpaper, below app windows so they cover
            // it). Exclusive zone 0 = don't reserve space but DO shrink
            // around the bars' zones. No keyboard — clicks only.
            layer->setLayer(LayerShellQt::Window::LayerBottom);
            layer->setAnchors(LayerShellQt::Window::Anchors(
                LayerShellQt::Window::AnchorTop
                | LayerShellQt::Window::AnchorBottom
                | LayerShellQt::Window::AnchorLeft
                | LayerShellQt::Window::AnchorRight));
            layer->setExclusiveZone(0);
            layer->setScope(QStringLiteral("jarvis-desktop"));
            layer->setKeyboardInteractivity(LayerShellQt::Window::KeyboardInteractivityNone);
        } else if (name == QLatin1String("jarvis-dock")) {
            // Dock: anchored to the bottom edge only, so the compositor
            // centers it at its own width. Top layer, with an exclusive zone
            // equal to its full footprint (height + the 8 px lift margin) so
            // maximized windows stop ABOVE it instead of being covered — the
            // macOS default (dock always visible, never overlapping content).
            layer->setLayer(LayerShellQt::Window::LayerTop);
            layer->setAnchors(LayerShellQt::Window::Anchors(
                LayerShellQt::Window::AnchorBottom));
            layer->setExclusiveZone(win->height() + 8);
            layer->setMargins(QMargins(0, 0, 0, 8));
            layer->setScope(QStringLiteral("jarvis-dock"));
            layer->setKeyboardInteractivity(LayerShellQt::Window::KeyboardInteractivityNone);
        } else {
            // Top menu bar: top edge of every output, its height excluded
            // from the area ordinary windows can sit in, kept on top.
            layer->setLayer(LayerShellQt::Window::LayerTop);
            layer->setAnchors(LayerShellQt::Window::Anchors(
                LayerShellQt::Window::AnchorTop
                | LayerShellQt::Window::AnchorLeft
                | LayerShellQt::Window::AnchorRight));
            layer->setExclusiveZone(win->height());
            layer->setScope(QStringLiteral("jarvis-topbar"));
            layer->setKeyboardInteractivity(LayerShellQt::Window::KeyboardInteractivityOnDemand);
        }
    }
#endif

    // Window control service (ADR 0025). Owns a wlr-foreign-toplevel client and
    // serves com.jarvis.Shell so the Action Bus can focus/minimize/maximize/
    // close windows on labwc. The name itself was already claimed up top as the
    // single-instance lock, so only the object still needs exporting here.
    auto* windowControl = new WindowControlService(&app);
    if (!bus.registerObject(QStringLiteral("/com/jarvis/Shell"), windowControl,
                            QDBusConnection::ExportAllSlots)) {
        qWarning("jarvis-shell: could not export /com/jarvis/Shell — window control disabled");
    }

    return app.exec();
}
