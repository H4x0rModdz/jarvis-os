import QtQuick
import QtQuick.Window
import QtQuick.Layouts
import Jarvis.Shell

/// The macOS-style dock: a centered glass pill of pinned app tiles plus,
/// after a divider, the Lilith orb. A second top-level layer-shell root
/// (objectName "jarvis-dock"); main.cpp anchors it to the bottom edge,
/// centered, on the Top layer with no exclusive zone — so maximized
/// windows float under it like macOS.
///
/// App tiles dispatch app.open via the Action Bus. The Launchpad tile
/// and the orb route through ShellBus to the popups Main.qml owns.
Window {
    id: root
    objectName: "jarvis-dock"
    visible: true
    color: "transparent"
    flags: Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint

    // Size to the pill (+ breathing room for hover lift / shadow). Only
    // the bottom edge is anchored, so the compositor centers us at this
    // self-determined width.
    width: pill.implicitWidth + 32
    height: pill.implicitHeight + 28

    // Pinned apps. `desktopId` is what app.open resolves (Flatpak ids /
    // binary names); the `launcher` tile is special — it opens the full
    // app grid instead of launching one app.
    readonly property var pinned: [
        { label: qsTr("Apps"),     icon: "view-app-grid-symbolic",  launcher: true },
        { label: "Firefox",        icon: "firefox",                 desktopId: "org.mozilla.firefox" },
        { label: qsTr("Arquivos"), icon: "system-file-manager",     desktopId: "org.kde.dolphin" },
        { label: "Zed",            icon: "dev.zed.Zed",             desktopId: "dev.zed.Zed" },
        { label: qsTr("Terminal"), icon: "utilities-terminal",      desktopId: "foot" }
    ]

    // Running-window awareness (Phase B, ADR 0024). Fed by the compositor
    // via wlr-foreign-toplevel; drives the running dot under each tile and
    // click-to-focus instead of relaunching.
    RunningAppsModel { id: running }

    GlassPanel {
        id: pill
        anchors.centerIn: parent
        implicitWidth: dockRow.implicitWidth + 28
        implicitHeight: 64

        RowLayout {
            id: dockRow
            anchors.centerIn: parent
            spacing: 8

            Repeater {
                model: root.pinned
                delegate: DockIcon {
                    iconName: modelData.icon
                    label: modelData.label
                    // Referencing running.revision makes this re-evaluate as
                    // windows open/close — isRunning() is a method, not a
                    // notifying property.
                    running: modelData.launcher === true
                             ? false
                             : (running.revision >= 0 && running.isRunning(modelData.desktopId))
                    onActivated: {
                        if (modelData.launcher === true) {
                            ShellBus.openLauncher();
                        } else if (running.isRunning(modelData.desktopId)) {
                            // Already open — focus / restore it instead of
                            // launching a second copy (macOS dock behaviour).
                            running.activateApp(modelData.desktopId);
                        } else {
                            ActionBusBridge.dispatch(
                                "app.open", JSON.stringify({ "app": modelData.desktopId }));
                        }
                    }
                }
            }

            // Divider between apps and Lilith — macOS separates apps from
            // the Trash with the same vertical rule.
            Rectangle {
                Layout.preferredWidth: 1
                Layout.preferredHeight: 36
                Layout.alignment: Qt.AlignVCenter
                color: Theme.border
            }

            LilithOrb {
                Layout.alignment: Qt.AlignVCenter
            }
        }
    }
}
