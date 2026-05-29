#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QLoggingCategory>

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

    QQmlApplicationEngine engine;

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
    engine.loadFromModule("Jarvis.Shell", "Main");
    if (engine.rootObjects().isEmpty()) {
        return 1;
    }

#ifdef JARVIS_HAVE_LAYER_SHELL
    // Anchor the bar to bottom edge of every output, exclude its height
    // from the area where ordinary windows can sit, and keep it on top.
    for (QObject* obj : engine.rootObjects()) {
        if (auto* win = qobject_cast<QQuickWindow*>(obj)) {
            auto* layer = LayerShellQt::Window::get(win);
            if (!layer) continue;
            layer->setLayer(LayerShellQt::Window::LayerTop);
            // Anchors lacks Q_DECLARE_OPERATORS_FOR_FLAGS in v6.0.0, so
            // we have to wrap the OR in an explicit Anchors(...) cast.
            layer->setAnchors(LayerShellQt::Window::Anchors(
                LayerShellQt::Window::AnchorBottom
                | LayerShellQt::Window::AnchorLeft
                | LayerShellQt::Window::AnchorRight));
            layer->setExclusiveZone(win->height());
            layer->setScope(QStringLiteral("jarvis-bar"));
            layer->setKeyboardInteractivity(LayerShellQt::Window::KeyboardInteractivityOnDemand);
        }
    }
#endif

    return app.exec();
}
