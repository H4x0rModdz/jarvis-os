import QtQuick
import Jarvis.Greeter

/// Pill button for an alternate auth method (Face ID / Voice ID / PIN).
/// V1 visually present so the design lands; clicking shows a "not yet
/// implemented" hint via the parent's `picked` signal. Real handlers
/// for face/voice/PIN land in V2 when PAM hooks are in place.
Rectangle {
    id: root
    property string label: ""
    property string glyph: "●"  // simple unicode placeholder; replaced by SVGs in V2
    signal picked()

    implicitWidth: 96
    implicitHeight: 84
    radius: 12
    color: area.containsMouse
        ? Qt.rgba(1, 1, 1, 0.06)
        : Qt.rgba(1, 1, 1, 0.03)
    border.color: area.containsMouse ? Theme.accent : Theme.border
    border.width: 1
    Behavior on color { ColorAnimation { duration: Theme.animFast } }
    Behavior on border.color { ColorAnimation { duration: Theme.animFast } }

    Column {
        anchors.centerIn: parent
        spacing: 6

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: root.glyph
            color: Theme.accent
            font.pixelSize: 22
        }

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: root.label
            color: Theme.textDim
            font.pixelSize: 11
            font.weight: Font.Bold
        }
    }

    MouseArea {
        id: area
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.picked()
    }
}
