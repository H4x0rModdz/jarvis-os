#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQuickWindow>

int main(int argc, char** argv)
{
    QGuiApplication app(argc, argv);
    app.setOrganizationName(QStringLiteral("Jarvis"));
    app.setApplicationName(QStringLiteral("jarvis-greeter"));

    QQmlApplicationEngine engine;
    engine.loadFromModule("Jarvis.Greeter", "Main");
    if (engine.rootObjects().isEmpty()) return 1;

    return app.exec();
}
