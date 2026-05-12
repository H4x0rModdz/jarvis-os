import QtQuick
import Jarvis.Greeter

/// Reusable password input — pill, focus-accent border, optional
/// echoMode override (so the same component drives PIN entry).
Rectangle {
    id: root
    property alias text: input.text
    property string placeholder: qsTr("Senha")
    property bool secret: true
    signal accepted()

    implicitWidth: 280
    implicitHeight: 44
    radius: 22
    color: Qt.rgba(1, 1, 1, 0.04)
    border.color: input.activeFocus ? Theme.accent : Theme.border
    border.width: 1
    Behavior on border.color { ColorAnimation { duration: Theme.animFast } }

    TextInput {
        id: input
        anchors.fill: parent
        anchors.leftMargin: 18
        anchors.rightMargin: 18
        verticalAlignment: TextInput.AlignVCenter
        color: Theme.text
        selectionColor: Theme.accent
        selectedTextColor: Theme.text
        font.pixelSize: 15
        clip: true
        echoMode: root.secret ? TextInput.Password : TextInput.Normal
        onAccepted: root.accepted()
    }

    Text {
        anchors.fill: input
        verticalAlignment: Text.AlignVCenter
        text: root.placeholder
        color: Theme.textDim
        font.pixelSize: 15
        visible: input.text.length === 0 && !input.activeFocus
    }
}
