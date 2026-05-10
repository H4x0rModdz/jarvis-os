import QtQuick
import Jarvis.Shell

/// Pill-shaped button used by the approval dialog. `filled=true` for the
/// primary action (Permitir sempre), `accent` overrides the highlight color
/// (e.g. danger red for Negar).
Rectangle {
    id: root
    implicitWidth: label.implicitWidth + 28
    implicitHeight: 34
    radius: implicitHeight / 2

    property alias text: label.text
    property color accent: Theme.text
    property bool filled: false
    signal clicked()

    color: filled
        ? Qt.rgba(root.accent.r, root.accent.g, root.accent.b, hover ? 0.95 : 0.85)
        : Qt.rgba(1, 1, 1, hover ? 0.10 : 0.04)
    border.color: filled ? "transparent" : (hover ? root.accent : Theme.border)
    border.width: 1

    property bool hover: area.containsMouse

    Behavior on color {
        ColorAnimation { duration: Theme.animFast }
    }
    Behavior on border.color {
        ColorAnimation { duration: Theme.animFast }
    }

    Text {
        id: label
        anchors.centerIn: parent
        color: root.filled ? Theme.background : root.accent
        font.pixelSize: 13
        font.weight: Font.Medium
    }

    MouseArea {
        id: area
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        hoverEnabled: true
        onClicked: root.clicked()
    }
}
