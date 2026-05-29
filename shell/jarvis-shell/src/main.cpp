#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QLoggingCategory>
#include <QDir>

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
    engine.loadFromModule("Jarvis.Shell", "Main");
    // The desktop icon surface (Computador / Pasta Pessoal / Lixeira) is
    // a second top-level Window; loadFromModule appends it to
    // rootObjects(). It identifies itself with objectName "jarvis-desktop"
    // so the layer-shell loop below can give it the full-screen bottom
    // anchoring the bar must not get.
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

        // Anchors lacks Q_DECLARE_OPERATORS_FOR_FLAGS in v6.0.0, so every
        // OR is wrapped in an explicit Anchors(...) cast.
        if (win->objectName() == QLatin1String("jarvis-desktop")) {
            // Desktop icons: cover the whole output on the *bottom* layer
            // (above swaybg's wallpaper, below app windows so they cover
            // it). Exclusive zone 0 = don't reserve space but DO shrink
            // around the bar's zone, so icons never hide under the bar.
            // No keyboard — clicking icons only needs pointer focus.
            layer->setLayer(LayerShellQt::Window::LayerBottom);
            layer->setAnchors(LayerShellQt::Window::Anchors(
                LayerShellQt::Window::AnchorTop
                | LayerShellQt::Window::AnchorBottom
                | LayerShellQt::Window::AnchorLeft
                | LayerShellQt::Window::AnchorRight));
            layer->setExclusiveZone(0);
            layer->setScope(QStringLiteral("jarvis-desktop"));
            layer->setKeyboardInteractivity(LayerShellQt::Window::KeyboardInteractivityNone);
        } else {
            // The bar: bottom edge of every output, height excluded from
            // the area ordinary windows can sit in, kept on top.
            layer->setLayer(LayerShellQt::Window::LayerTop);
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
