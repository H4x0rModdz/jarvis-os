import QtQuick
import Jarvis.Greeter

/// Floating toast at the bottom-centre. Shows the most recent
/// error/info message and fades out after 4s. Replaces the
/// per-mode inline error text from V1.
Rectangle {
    id: root
    property string message: ""
    property bool error: false

    width: Math.min(implicitContentWidth + 36, 720)
    height: 36
    radius: 18
    color: root.error
        ? Qt.rgba(1.0, 0.36, 0.49, 0.20)
        : Qt.rgba(0.49, 0.36, 1.0, 0.18)
    border.color: root.error ? Theme.danger : Theme.accent
    border.width: 1

    opacity: message.length > 0 ? 1.0 : 0.0
    visible: opacity > 0.01
    Behavior on opacity { NumberAnimation { duration: Theme.animNormal; easing.type: Easing.OutQuad } }

    readonly property real implicitContentWidth: msgText.implicitWidth

    Text {
        id: msgText
        anchors.centerIn: parent
        text: root.message
        color: root.error ? Theme.danger : Theme.text
        font.pixelSize: 12
        font.weight: Font.Bold
    }

    Timer {
        id: hide
        interval: 4000
        onTriggered: root.message = ""
    }

    onMessageChanged: {
        if (message.length > 0) hide.restart();
    }
}
