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
    // Qt 6.4 places module resources at qrc:/<URI as path>/qml/<File>.
    // Qt 6.5+ moved them to qrc:/qt/qml/<URI as path>/ and gained
    // engine.loadFromModule — when we bump the minimum we'll switch.
    engine.load(QUrl(QStringLiteral("qrc:/Jarvis/Shell/qml/Main.qml")));
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
