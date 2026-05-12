import QtQuick
import Jarvis.Greeter

/// Pill button used by every mode. Single property `label` and a
/// `clicked` signal — modes pick the copy ("UNLOCK SYSTEM" / "UNLOCK").
Rectangle {
    id: root
    property string label: qsTr("UNLOCK")
    property bool busy: false
    signal clicked()

    implicitWidth: 280
    implicitHeight: 44
    radius: 22
    color: area.containsMouse
        ? Theme.accent
        : Qt.darker(Theme.accent, 1.2)
    border.color: Theme.accent
    border.width: 1
    opacity: busy ? 0.55 : 1.0
    Behavior on color { ColorAnimation { duration: Theme.animFast } }
    Behavior on opacity { NumberAnimation { duration: Theme.animFast } }

    Text {
        anchors.centerIn: parent
        text: root.busy ? qsTr("VERIFICANDO…") : root.label
        color: Theme.text
        font.pixelSize: 13
        font.weight: Font.Bold
        font.letterSpacing: 2
    }

    MouseArea {
        id: area
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        enabled: !root.busy
        onClicked: root.clicked()
    }
}
