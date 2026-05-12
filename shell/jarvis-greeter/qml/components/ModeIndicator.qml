import QtQuick
import Jarvis.Greeter

/// Three-dot row showing which mode is active. Click a dot to jump
/// directly to that mode; the parent owns the binding to currentIndex.
Row {
    id: root
    property int count: 3
    property int currentIndex: 0
    signal indexChosen(int index)

    spacing: 14

    Repeater {
        model: root.count

        Rectangle {
            width: 10
            height: 10
            radius: 5
            color: index === root.currentIndex
                ? Theme.accent
                : Qt.rgba(1, 1, 1, 0.18)
            border.color: index === root.currentIndex
                ? Theme.accent
                : Qt.rgba(1, 1, 1, 0.25)
            border.width: 1

            scale: index === root.currentIndex ? 1.2 : 1.0
            Behavior on scale {
                NumberAnimation { duration: 180; easing.type: Easing.OutCubic }
            }
            Behavior on color {
                ColorAnimation { duration: 180 }
            }

            MouseArea {
                anchors.fill: parent
                anchors.margins: -6
                cursorShape: Qt.PointingHandCursor
                onClicked: root.indexChosen(index)
            }
        }
    }
}
