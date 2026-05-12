import QtQuick
import Jarvis.Greeter

/// Stylised Jarvis ring logo — two concentric rings with a small
/// orbital sphere that drifts around the outer ring. Drawn with
/// rectangles + transforms so we don't need a Canvas or shader for
/// V1; replacement with a proper Shape comes when we move to GLES2
/// rendering (Phase 3 compositor work).
Item {
    id: root
    property real size: 110
    property color accent: Theme.accent

    implicitWidth: size
    implicitHeight: size

    // Outer ring (the disc/halo).
    Rectangle {
        anchors.centerIn: parent
        width: parent.size
        height: parent.size
        radius: width / 2
        color: "transparent"
        border.color: root.accent
        border.width: 2
        opacity: 0.85
    }

    // Inner ring — slightly smaller, slightly dimmer.
    Rectangle {
        anchors.centerIn: parent
        width: parent.size * 0.62
        height: parent.size * 0.62
        radius: width / 2
        color: "transparent"
        border.color: root.accent
        border.width: 1
        opacity: 0.55
    }

    // Central glow dot (the "eye"). A bright disc + a faint halo.
    Rectangle {
        anchors.centerIn: parent
        width: parent.size * 0.18
        height: parent.size * 0.18
        radius: width / 2
        color: root.accent
        opacity: 0.95
    }
    Rectangle {
        anchors.centerIn: parent
        width: parent.size * 0.34
        height: parent.size * 0.34
        radius: width / 2
        color: "transparent"
        border.color: root.accent
        border.width: 1
        opacity: 0.4
    }

    // Orbital sphere — tiny dot riding the outer ring at the top,
    // rotating slowly. The transform's origin sits at the centre of
    // the logo so the dot traces the circle.
    Item {
        anchors.fill: parent
        RotationAnimator on rotation {
            from: 0
            to: 360
            duration: 9000
            loops: Animation.Infinite
            running: true
        }
        Rectangle {
            x: parent.width / 2 - width / 2
            y: 0
            width: 8
            height: 8
            radius: 4
            color: root.accent
        }
    }
}
