#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQuickStyle>
#include <QLoggingCategory>

int main(int argc, char** argv)
{
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
    return app.exec();
}
