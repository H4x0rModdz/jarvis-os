#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <QDir>
#include <QFile>
#include <QFont>
#include <QIcon>
#include <QStandardPaths>
#include <QUrl>
#include <QDBusConnection>
#include <QDBusError>

#include "escape_closer.h"
#include "icon_image_provider.h"
#include "wallpaper_backdrop_provider.h"
#include "window_control_service.h"

#ifdef JARVIS_HAVE_LAYER_SHELL
#  include <LayerShellQt/Shell>
#endif

int main(int argc, char** argv)
{
#ifdef JARVIS_HAVE_LAYER_SHELL
    // Switch Qt's Wayland shell integration to wlr-layer-shell BEFORE the
    // QGuiApplication is constructed (the integration is selected during
    // QPA platform init). Every shell window is then a layer surface; each
    // one declares its own layer, anchors, margins and namespace in QML
    // through the `org.kde.layershell` attached properties.
    //
    // That configuration used to be done here, in a loop over the root
    // objects after the engine had loaded them. By then the windows were
    // already visible, so the surface existed and LayerShellQt refused the
    // late namespace ("already has a shell integration"): all three surfaces
    // reached the compositor with the default namespace "window". Compositor
    // rules — blur, animations, anything matched by namespace — could never
    // tell the dock from the desktop. Declared in QML, the values are in
    // place before the surface is created.
    LayerShellQt::Shell::useLayerShell();
#endif

    QQuickStyle::setStyle(QStringLiteral("Material"));
    qputenv("QT_QUICK_CONTROLS_MATERIAL_THEME", "Dark");

    QGuiApplication app(argc, argv);
    app.setOrganizationName(QStringLiteral("Jarvis"));
    app.setApplicationName(QStringLiteral("jarvis-shell"));

    // One UI typeface for the whole shell (Theme.fontFamily lists the same
    // fallbacks for the places that set a family explicitly).
    QFont uiFont(QStringLiteral("Inter"));
    uiFont.setStyleHint(QFont::SansSerif);
    app.setFont(uiFont);

    // Escape closes the focused/visible shell panel everywhere. Per-window
    // Esc handlers don't fire for panels that never grab keyboard focus under
    // labwc, so we catch it once at the application level. See escape_closer.h.
    app.installEventFilter(new EscapeCloser(&app));

    // Single instance, enforced BEFORE any window exists.
    //
    // `com.jarvis.Shell` doubles as the lock: DBus grants the name to exactly
    // one connection and releases it automatically when that process dies, so
    // there is no stale-lockfile failure mode. A second instance would paint a
    // second bar, dock and desktop into the same layer-shell slots and double
    // every timer and repaint.
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
    // `image://theme/<name>` lookup would miss. Pin the theme the image
    // ships (WhiteSur, see iso/assets/theme/install-whitesur.sh) and let
    // hicolor catch the strays. JARVIS_ICON_THEME overrides it — the preview
    // container has no WhiteSur.
    const QString iconTheme = qEnvironmentVariable("JARVIS_ICON_THEME");
    if (!iconTheme.isEmpty()) {
        QIcon::setThemeName(iconTheme);
    } else if (QIcon::themeName().isEmpty()) {
        QIcon::setThemeName(QStringLiteral("WhiteSur"));
    }
    QIcon::setFallbackThemeName(QStringLiteral("hicolor"));

    QQmlApplicationEngine engine;

    // `image://theme/<name>` — freedesktop icons. `image://backdrop/…` — the
    // blurred wallpaper that glass surfaces show through (see GlassPanel).
    // Registered before any QML loads so the first paint resolves both.
    engine.addImageProvider(QStringLiteral("theme"), new IconImageProvider);
    engine.addImageProvider(QStringLiteral("backdrop"), new WallpaperBackdropProvider);

    // Real home dir, resolved in C++ because xdg-open never expands a shell
    // `~` and QML has no first-class home-path accessor.
    engine.rootContext()->setContextProperty(
        QStringLiteral("HomePath"), QDir::homePath());

    // Dev switch: don't open the first-boot wizard. It gates on a Settings
    // daemon key, so in an environment with no daemons (tools/dev/
    // shell-preview.sh) it would always cover the desktop.
    engine.rootContext()->setContextProperty(
        QStringLiteral("SkipFirstBoot"), qEnvironmentVariableIsSet("JARVIS_SKIP_FIRST_BOOT"));
    // Does the compositor blur what is behind our translucent surfaces?
    // Hyprland does (layer rules in hyprland.lua); labwc cannot. When it
    // does, GlassPanel paints only its tint and lets the compositor frost
    // the real content behind it — windows included — instead of drawing
    // the pre-blurred wallpaper. JARVIS_COMPOSITOR_BLUR=0/1 overrides.
    {
        bool blur = qEnvironmentVariable("XDG_CURRENT_DESKTOP")
                        .contains(QLatin1String("Hyprland"), Qt::CaseInsensitive);
        if (qEnvironmentVariableIsSet("JARVIS_COMPOSITOR_BLUR"))
            blur = qEnvironmentVariableIntValue("JARVIS_COMPOSITOR_BLUR") != 0;
        engine.rootContext()->setContextProperty(QStringLiteral("CompositorBlur"), blur);
    }

    // Dev switch: show the command-centre HUD regardless of the setting.
    engine.rootContext()->setContextProperty(
        QStringLiteral("HudForced"), qEnvironmentVariableIsSet("JARVIS_HUD"));

    // Avatar model URL for the embodied Lilith (ADR 0028). The VRM is a
    // drop-in at <data>/jarvis/avatar/lilith.vrm. AvatarModelPresent gates the
    // whole 3D view: with no VRM there is nothing to show, so we don't
    // instantiate QtQuick3D at all — an idle View3D still renders every frame.
    {
        const QString dataDir =
            QStandardPaths::writableLocation(QStandardPaths::GenericDataLocation);
        const QString vrmPath = dataDir + QStringLiteral("/jarvis/avatar/lilith.vrm");
        engine.rootContext()->setContextProperty(
            QStringLiteral("AvatarModelUrl"), QUrl::fromLocalFile(vrmPath).toString());
        engine.rootContext()->setContextProperty(
            QStringLiteral("AvatarModelPresent"), QFile::exists(vrmPath));
    }

    // Dev loop: set JARVIS_QML_PATH to a directory that contains the built
    // `Jarvis/Shell/` module tree to shadow the qrc-compiled QML. Edit a
    // .qml, restart jarvis-shell, see the change — no rebuild.
    const QByteArray devQmlPath = qgetenv("JARVIS_QML_PATH");
    if (!devQmlPath.isEmpty()) {
        engine.addImportPath(QString::fromLocal8Bit(devQmlPath));
        qInfo("jarvis-shell: dev QML path active: %s", devQmlPath.constData());
    }

    // Three layer-shell roots, all rendered by this one engine. Loaded
    // through the QML module (not a raw qrc URL) so the generated qmldir —
    // including `singleton Theme` — is honoured.
    //   Main    → "jarvis-topbar"  (top menu bar + every popup)
    //   Dock    → "jarvis-dock"    (floating bottom dock + Lilith orb)
    //   Desktop → "jarvis-desktop" (optional command-centre HUD, bottom layer)
    //
    // JARVIS_SHELL_SURFACES picks which roots load, for telling which surface
    // a performance problem belongs to. Unset → all three.
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
