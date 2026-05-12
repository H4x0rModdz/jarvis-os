import QtQuick
import Jarvis.Greeter

/// Pill containing a quick-action label. V1 click is decorative —
/// real conversational unlock requires a per-prompt PAM bridge that
/// translates "Open Developer Workspace" into a session-command
/// hint (Phase 3.5+). For now it surfaces the affordance.
Rectangle {
    id: root
    property string label: ""
    signal picked()

    implicitWidth: labelText.implicitWidth + 28
    implicitHeight: 32
    radius: 16
    color: area.containsMouse
        ? Qt.rgba(0.49, 0.36, 1.0, 0.22)
        : Qt.rgba(1, 1, 1, 0.05)
    border.color: area.containsMouse ? Theme.accent : Theme.border
    border.width: 1
    Behavior on color { ColorAnimation { duration: Theme.animFast } }
    Behavior on border.color { ColorAnimation { duration: Theme.animFast } }

    Text {
        id: labelText
        anchors.centerIn: parent
        text: root.label
        color: Theme.text
        font.pixelSize: 12
    }

    MouseArea {
        id: area
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.picked()
    }
}
