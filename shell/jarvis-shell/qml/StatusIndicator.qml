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

        // No idle pulse. This used to breathe forever while `reachable &&
        // !busy` — i.e. during the normal, healthy, nothing-is-happening state.
        // An infinite animation there means the top bar repaints at 60 fps for
        // the entire life of the session and neither Qt's render loop nor the
        // compositor ever goes idle, which costs real frames on a wide display
        // and burns power for no information: the dot's COLOUR already says
        // reachable / unreachable / busy. Motion is reserved for the transient
        // state below, per the house rule that animation must be purposeful.

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
