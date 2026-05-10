import QtQuick
import Jarvis.Shell

/// The little "apps" button on the left of the bar. Three horizontal lines
/// (a stand-in for a proper logo icon) — clicking it opens the launcher.
Rectangle {
    id: root
    implicitWidth: 40
    implicitHeight: 40
    radius: 8
    color: area.containsMouse ? Qt.rgba(1, 1, 1, 0.08) : Qt.rgba(1, 1, 1, 0.04)
    border.color: area.containsMouse ? Theme.border : "transparent"
    border.width: 1
    Behavior on color { ColorAnimation { duration: Theme.animFast } }
    Behavior on border.color { ColorAnimation { duration: Theme.animFast } }

    signal clicked()

    Column {
        anchors.centerIn: parent
        spacing: 4
        Repeater {
            model: 3
            Rectangle {
                width: 18; height: 2; radius: 1
                color: Theme.text
                opacity: area.containsMouse ? 1.0 : 0.85
            }
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
