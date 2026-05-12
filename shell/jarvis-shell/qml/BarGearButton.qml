import QtQuick
import Jarvis.Shell

/// The gear button on the right of the bar — opens the preferences
/// panel. Visual sibling of BarMenuButton.
Rectangle {
    id: root
    implicitWidth: 32
    implicitHeight: 32
    radius: 8
    color: area.containsMouse ? Qt.rgba(1, 1, 1, 0.08) : "transparent"
    border.color: area.containsMouse ? Theme.border : "transparent"
    border.width: 1
    Behavior on color { ColorAnimation { duration: Theme.animFast } }
    Behavior on border.color { ColorAnimation { duration: Theme.animFast } }

    signal clicked()

    // Cheap gear glyph — a circle with 8 teeth, drawn from rotated rects.
    Item {
        anchors.centerIn: parent
        width: 18
        height: 18

        // Outer "teeth" — 4 long rects rotated by index*45.
        Repeater {
            model: 4
            Rectangle {
                anchors.centerIn: parent
                width: 4
                height: 18
                radius: 1
                color: Theme.text
                opacity: area.containsMouse ? 1.0 : 0.7
                transform: Rotation { angle: index * 45 }
            }
        }

        // Inner hole.
        Rectangle {
            anchors.centerIn: parent
            width: 6
            height: 6
            radius: 3
            color: root.color === "transparent"
                ? Theme.surface
                : Qt.darker(root.color, 1.5)
        }
    }

    MouseArea {
        id: area
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        hoverEnabled: true
        onClicked: root.clicked()
    }
}
