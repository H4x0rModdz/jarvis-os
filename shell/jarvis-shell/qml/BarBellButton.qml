import QtQuick
import Jarvis.Shell

/// Bell glyph on the bar — clicking opens the NotificationDrawer.
/// Renders the bell from a couple of rounded rectangles so we don't
/// pull a font / SVG dependency just for one button.
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

    // Bell shape — dome on top, flared rim, little clapper at the
    // bottom. Built from rectangles + radii so it stays crisp at any
    // scale without a glyph font.
    Item {
        anchors.centerIn: parent
        width: 18
        height: 18

        Rectangle {
            // Dome
            anchors.horizontalCenter: parent.horizontalCenter
            y: 2
            width: 12
            height: 11
            radius: 6
            color: "transparent"
            border.color: Theme.text
            border.width: 1.5
            opacity: area.containsMouse ? 1.0 : 0.85
        }
        Rectangle {
            // Rim
            anchors.horizontalCenter: parent.horizontalCenter
            y: 13
            width: 16
            height: 2
            radius: 1
            color: Theme.text
            opacity: area.containsMouse ? 1.0 : 0.85
        }
        Rectangle {
            // Clapper
            anchors.horizontalCenter: parent.horizontalCenter
            y: 15
            width: 3
            height: 3
            radius: 1.5
            color: Theme.text
            opacity: area.containsMouse ? 1.0 : 0.85
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
