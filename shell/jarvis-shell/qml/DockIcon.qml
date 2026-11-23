import QtQuick
import QtQuick.Controls
import Jarvis.Shell

/// One dock tile. Hovering magnifies the icon from its base (it grows upward,
/// out of the dock) and shows the app's name in a small bubble above; LEFT
/// click fires `activated`, RIGHT click opens a context menu (open/focus,
/// pin/unpin, close). A dot under the tile marks a running app — solid when a
/// window is open, hollow when every window is minimized.
Item {
    id: root
    implicitWidth: Theme.dockIcon + 6
    implicitHeight: Theme.dockIcon + 10

    property string iconName: ""   // freedesktop theme name OR absolute path
    property string label: ""
    property string desktopId: ""
    /// 0 = not running, 1 = open (a visible window), 2 = running but minimized.
    property int runStateValue: 0
    /// Whether this app is pinned (drives the menu's pin/unpin label).
    property bool isPinned: true
    /// The launcher tile is special — no running state, no context menu.
    property bool isLauncher: false

    signal activated()
    signal pinToggle()
    signal quitRequested()

    readonly property bool hovered: area.containsMouse
    readonly property real zoom: area.pressed ? 1.08 : (hovered ? 1.22 : 1.0)

    // ── Icon (or its stand-in), magnifying from the bottom edge ──────────
    Item {
        id: tile
        width: Theme.dockIcon
        height: Theme.dockIcon
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        transformOrigin: Item.Bottom
        scale: root.zoom
        Behavior on scale { NumberAnimation { duration: Theme.animFast; easing.type: Easing.OutCubic } }

        Image {
            id: img
            anchors.fill: parent
            fillMode: Image.PreserveAspectFit
            sourceSize: Qt.size(Theme.dockIcon * 2, Theme.dockIcon * 2)
            smooth: true
            visible: status === Image.Ready && !root.isLauncher
            source: root.isLauncher ? ""
                  : root.iconName.startsWith("/") ? "file://" + root.iconName
                  : root.iconName.length           ? "image://theme/" + root.iconName
                  : ""
        }

        // Launcher tile: a rounded square with a 3×3 grid, drawn here so it
        // does not depend on the icon theme having an "apps" picture.
        Rectangle {
            anchors.fill: parent
            anchors.margins: 3
            visible: root.isLauncher
            radius: width * 0.24
            gradient: Gradient {
                GradientStop { position: 0.0; color: "#5a5a60" }
                GradientStop { position: 1.0; color: "#2c2c30" }
            }
            border.width: 1
            border.color: Theme.glassRim
            Grid {
                anchors.centerIn: parent
                columns: 3
                spacing: 4
                Repeater {
                    model: 9
                    Rectangle {
                        width: 7; height: 7; radius: 2
                        color: ["#ff453a", "#ff9f0a", "#ffd60a", "#30d158", "#64d2ff",
                                "#0a84ff", "#5e5ce6", "#bf5af2", "#ff375f"][index]
                    }
                }
            }
        }

        // Stand-in when the theme has no icon for this app: its initial on a
        // neutral rounded square.
        Rectangle {
            anchors.fill: parent
            anchors.margins: 3
            visible: !root.isLauncher && img.status !== Image.Ready
            radius: width * 0.24
            gradient: Gradient {
                GradientStop { position: 0.0; color: "#4a4a50" }
                GradientStop { position: 1.0; color: "#2a2a2e" }
            }
            border.width: 1
            border.color: Theme.glassRim
            Text {
                anchors.centerIn: parent
                text: root.label.length ? root.label.charAt(0).toUpperCase() : "?"
                color: Theme.text
                font.pixelSize: 22
                font.weight: Font.DemiBold
            }
        }
    }

    // ── Running indicator ────────────────────────────────────────────────
    Rectangle {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 1
        width: 4
        height: 4
        radius: 2
        visible: root.runStateValue > 0
        color: root.runStateValue === 1 ? Theme.text : "transparent"
        border.color: Theme.text
        border.width: root.runStateValue === 2 ? 1 : 0
    }

    // ── Name bubble above the dock ───────────────────────────────────────
    Rectangle {
        anchors.bottom: tile.top
        anchors.bottomMargin: 16
        anchors.horizontalCenter: parent.horizontalCenter
        width: nameLabel.implicitWidth + 18
        height: 24
        radius: 12
        color: Theme.glassSolid
        border.width: 1
        border.color: Theme.glassRim
        opacity: (root.hovered && !contextMenu.visible) ? 1.0 : 0.0
        visible: opacity > 0
        Behavior on opacity { NumberAnimation { duration: Theme.animFast } }
        Text {
            id: nameLabel
            anchors.centerIn: parent
            text: root.label
            color: Theme.text
            font.pixelSize: Theme.fontCaption + 1
        }
    }

    MouseArea {
        id: area
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        onClicked: function(mouse) {
            if (mouse.button === Qt.RightButton) {
                if (!root.isLauncher) {
                    contextMenu.popup();
                }
            } else {
                root.activated();
            }
        }
    }

    // Right-click context menu. The launcher tile never opens it.
    Menu {
        id: contextMenu
        MenuItem {
            text: root.runStateValue === 2 ? qsTr("Restaurar")
                : root.runStateValue === 1 ? qsTr("Focar")
                : qsTr("Abrir")
            onTriggered: root.activated()
        }
        MenuItem {
            text: root.isPinned ? qsTr("Desafixar do dock") : qsTr("Fixar no dock")
            onTriggered: root.pinToggle()
        }
        MenuSeparator { visible: root.runStateValue > 0 }
        MenuItem {
            text: qsTr("Fechar")
            enabled: root.runStateValue > 0
            visible: root.runStateValue > 0
            onTriggered: root.quitRequested()
        }
    }
}
