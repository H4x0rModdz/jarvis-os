#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQuickWindow>

#ifdef JARVIS_HAVE_LAYER_SHELL
#  include <LayerShellQt/Shell>
#  include <LayerShellQt/Window>
#endif

int main(int argc, char** argv)
{
#ifdef JARVIS_HAVE_LAYER_SHELL
    LayerShellQt::Shell::useLayerShell();
#endif

    QGuiApplication app(argc, argv);
    app.setOrganizationName(QStringLiteral("Jarvis"));
    app.setApplicationName(QStringLiteral("jarvis-lock-window"));

    QQmlApplicationEngine engine;
    engine.loadFromModule("Jarvis.Lock", "Main");
    if (engine.rootObjects().isEmpty()) return 1;

#ifdef JARVIS_HAVE_LAYER_SHELL
    // Push the lock window to the Overlay layer above every other
    // surface on the output, anchor to every edge so it fills the
    // screen, and grab keyboard exclusively so Alt+Tab + shortcuts
    // don't leak past us. VT switch still escapes — that's the V1
    // limitation documented in ADR 0014.
    for (QObject* obj : engine.rootObjects()) {
        if (auto* win = qobject_cast<QQuickWindow*>(obj)) {
            auto* layer = LayerShellQt::Window::get(win);
            if (!layer) continue;
            layer->setLayer(LayerShellQt::Window::LayerOverlay);
            layer->setAnchors(LayerShellQt::Window::Anchors(
                LayerShellQt::Window::AnchorTop
                | LayerShellQt::Window::AnchorBottom
                | LayerShellQt::Window::AnchorLeft
                | LayerShellQt::Window::AnchorRight));
            layer->setExclusiveZone(-1);
            layer->setScope(QStringLiteral("jarvis-lock"));
            layer->setKeyboardInteractivity(
                LayerShellQt::Window::KeyboardInteractivityExclusive);
        }
    }
#endif

    return app.exec();
}
