import QtQuick
import QtQuick.Controls
import Jarvis.Shell

/// One dock tile. Hover lifts + scales the icon (macOS-style magnification,
/// lite); LEFT click fires `activated`, RIGHT click opens a context menu
/// (open/focus, pin/unpin, close). The running indicator under the tile
/// distinguishes an open app (filled dot) from a minimized one (hollow ring).
Item {
    id: root
    implicitWidth: 52
    implicitHeight: 52

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

    Image {
        id: img
        anchors.centerIn: parent
        width: 44
        height: 44
        fillMode: Image.PreserveAspectFit
        smooth: true
        source: root.iconName.startsWith("/") ? "file://" + root.iconName
              : root.iconName.length           ? "image://theme/" + root.iconName
              : ""
        visible: status === Image.Ready
        scale: area.containsMouse ? 1.22 : 1.0
        y: area.containsMouse ? -6 : 0
        Behavior on scale { NumberAnimation { duration: Theme.animFast; easing.type: Easing.OutBack } }
        Behavior on y     { NumberAnimation { duration: Theme.animFast; easing.type: Easing.OutCubic } }
    }

    // Monogram fallback when the theme can't resolve the icon.
    Rectangle {
        anchors.centerIn: parent
        width: 44
        height: 44
        radius: 10
        visible: img.status !== Image.Ready
        color: Qt.rgba(1, 1, 1, 0.06)
        border.color: Theme.border
        border.width: 1
        scale: area.containsMouse ? 1.22 : 1.0
        Behavior on scale { NumberAnimation { duration: Theme.animFast; easing.type: Easing.OutBack } }

        Text {
            anchors.centerIn: parent
            text: root.label.length ? root.label.charAt(0).toUpperCase() : "?"
            color: Theme.accent
            font.pixelSize: 20
            font.weight: Font.Bold
        }
    }

    // Running indicator. Open app → a filled accent dot; minimized app →
    // a hollow accent ring (same footprint, clearly different state).
    Rectangle {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 1
        width: 5
        height: 5
        radius: 2.5
        visible: root.runStateValue > 0
        color: root.runStateValue === 1 ? Theme.accent : "transparent"
        border.color: Theme.accent
        border.width: root.runStateValue === 2 ? 1 : 0
        opacity: root.runStateValue === 1 ? 0.95 : 0.7
    }

    // Hover tooltip above the dock.
    Text {
        anchors.bottom: parent.top
        anchors.bottomMargin: 2
        anchors.horizontalCenter: parent.horizontalCenter
        text: root.label
        color: Theme.text
        font.pixelSize: 10
        visible: area.containsMouse && !contextMenu.visible
        style: Text.Outline
        styleColor: Qt.rgba(0, 0, 0, 0.6)
    }

    MouseArea {
        id: area
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        cursorShape: Qt.PointingHandCursor
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
