import QtQuick
import Jarvis.Greeter

/// Translucent rounded card with subtle border + top highlight. The
/// V1 doesn't pull real backdrop-blur (that's a compositor-side
/// shader pass we deliberately defer to Phase 3.5). The opacity +
/// border combo reads as "glass" against the dark background until
/// we own the blur pipeline.
Rectangle {
    id: root
    radius: 18
    color: Theme.surfaceBright
    border.color: Theme.border
    border.width: 1

    // Soft inner-top highlight to fake the lens edge.
    Rectangle {
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.margins: 1
        height: 1
        color: Qt.rgba(1, 1, 1, 0.07)
    }
}
