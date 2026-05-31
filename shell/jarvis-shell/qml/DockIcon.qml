import QtQuick
import Jarvis.Shell

/// One dock tile. Hover lifts + scales the icon (macOS-style
/// magnification, lite) without reflowing the row; click fires
/// `activated`. Icon resolves via the theme provider (or an absolute
/// path) with a monogram fallback — same resolution AppCell uses.
Item {
    id: root
    implicitWidth: 52
    implicitHeight: 52

    property string iconName: ""   // freedesktop theme name OR absolute path
    property string label: ""
    property bool running: false   // a window for this app is open (macOS dot)
    signal activated()

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

    // Running indicator — the macOS dot under an open app.
    Rectangle {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 1
        width: 4
        height: 4
        radius: 2
        color: Theme.accent
        visible: root.running
        opacity: 0.9
    }

    // Hover tooltip above the dock.
    Text {
        anchors.bottom: parent.top
        anchors.bottomMargin: 2
        anchors.horizontalCenter: parent.horizontalCenter
        text: root.label
        color: Theme.text
        font.pixelSize: 10
        visible: area.containsMouse
        style: Text.Outline
        styleColor: Qt.rgba(0, 0, 0, 0.6)
    }

    MouseArea {
        id: area
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.activated()
    }
}
