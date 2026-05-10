import QtQuick

/// A small LED dot reflecting Lilith reachability.
/// Pulses gently while idle, fades to red when offline.
Item {
    id: root
    implicitWidth: 18
    implicitHeight: 18

    property bool reachable: false
    property bool busy: false

    Rectangle {
        id: dot
        anchors.centerIn: parent
        width: 10
        height: 10
        radius: 5
        color: root.reachable
            ? (root.busy ? Theme.accent : Theme.success)
            : Theme.danger

        Behavior on color {
            ColorAnimation { duration: Theme.animNormal; easing.type: Easing.OutQuad }
        }

        // Soft pulse when idle and reachable.
        SequentialAnimation on opacity {
            running: root.reachable && !root.busy
            loops: Animation.Infinite
            NumberAnimation { from: 1.0; to: 0.55; duration: 900; easing.type: Easing.InOutSine }
            NumberAnimation { from: 0.55; to: 1.0; duration: 900; easing.type: Easing.InOutSine }
        }

        // Faster pulse while waiting on a reply.
        SequentialAnimation on scale {
            running: root.busy
            loops: Animation.Infinite
            NumberAnimation { from: 1.0; to: 1.4; duration: 350; easing.type: Easing.OutQuad }
            NumberAnimation { from: 1.4; to: 1.0; duration: 350; easing.type: Easing.InQuad }
        }
    }

    // Soft halo
    Rectangle {
        anchors.centerIn: parent
        width: 18
        height: 18
        radius: 9
        color: "transparent"
        border.color: dot.color
        border.width: 1
        opacity: 0.35
    }
}
