#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QLoggingCategory>
#include <QDir>
#include <QIcon>
#include <QMargins>
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
    engine.loadFromModule("Jarvis.Shell", "Main");
    engine.loadFromModule("Jarvis.Shell", "Dock");
    engine.loadFromModule("Jarvis.Shell", "Desktop");
    if (engine.rootObjects().isEmpty()) {
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
            // centers it at its own width. Top layer, exclusive zone 0 —
            // maximized windows float *under* it, exactly like macOS. A
            // small bottom margin lifts it off the screen edge.
            layer->setLayer(LayerShellQt::Window::LayerTop);
            layer->setAnchors(LayerShellQt::Window::Anchors(
                LayerShellQt::Window::AnchorBottom));
            layer->setExclusiveZone(0);
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

    // Window control service (ADR 0025). Owns a wlr-foreign-toplevel client
    // and serves com.jarvis.Shell so the Action Bus can focus/minimize/
    // maximize/close windows on labwc. Best-effort: if the name is already
    // taken (a second shell instance) we log and carry on — the desktop UI
    // doesn't depend on owning it.
    auto* windowControl = new WindowControlService(&app);
    QDBusConnection bus = QDBusConnection::sessionBus();
    if (!bus.registerService(QStringLiteral("com.jarvis.Shell"))) {
        qWarning("jarvis-shell: could not own com.jarvis.Shell — window control disabled (%s)",
                 qPrintable(bus.lastError().message()));
    } else if (!bus.registerObject(QStringLiteral("/com/jarvis/Shell"), windowControl,
                                   QDBusConnection::ExportAllSlots)) {
        qWarning("jarvis-shell: could not export /com/jarvis/Shell — window control disabled");
    }

    return app.exec();
}
