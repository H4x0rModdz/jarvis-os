import QtQuick
import Jarvis.Shell

/// One clickable item in the menu bar: content with a soft rounded highlight
/// on hover / while its menu is open — the menu-bar-extra look. Children are
/// laid out in a centred Row.
Item {
    id: root

    default property alias content: row.children
    /// Keep the highlight on while the popup this item opened is showing.
    property bool active: false
    property int horizontalPadding: 7

    signal clicked()

    implicitWidth: row.implicitWidth + horizontalPadding * 2
    implicitHeight: Theme.topBarHeight

    Rectangle {
        anchors.fill: parent
        anchors.topMargin: 3
        anchors.bottomMargin: 3
        radius: 6
        color: area.pressed ? Theme.fillPressed
             : (area.containsMouse || root.active) ? Theme.fillHover
             : "transparent"
        Behavior on color { ColorAnimation { duration: Theme.animFast } }
    }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 5
    }

    MouseArea {
        id: area
        anchors.fill: parent
        hoverEnabled: true
        onClicked: root.clicked()
    }
}
