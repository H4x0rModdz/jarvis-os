import QtQuick
import Jarvis.Shell

/// Push-to-talk button next to the Lilith input. Renders the current
/// VoiceBridge state — idle (outline), listening (purple pulse),
/// processing (rotating arc), speaking (waveform). Click toggles
/// between idle and listening; click during processing/speaking is
/// ignored by the daemon.
///
/// V1 surface: the daemon emits StateChanged but STT/TTS aren't wired
/// yet. The button still cycles through states so the visual paths are
/// validated against the bridge contract before V2/V3 light them up.
Rectangle {
    id: root
    implicitWidth: 40
    implicitHeight: 40
    radius: 8

    readonly property string voiceState: VoiceBridge.state
    readonly property bool active: voiceState !== "idle"
    readonly property bool unreachable: !VoiceBridge.reachable

    color: area.containsMouse
        ? Qt.rgba(1, 1, 1, 0.08)
        : (active ? Qt.rgba(0.49, 0.36, 1.0, 0.18) : Qt.rgba(1, 1, 1, 0.04))
    border.color: {
        if (unreachable) return Theme.border;
        if (voiceState === "listening") return Theme.accent;
        if (voiceState === "processing") return Theme.accent;
        if (voiceState === "speaking") return Theme.success;
        return area.containsMouse ? Theme.border : "transparent";
    }
    border.width: 1
    Behavior on color { ColorAnimation { duration: Theme.animFast } }
    Behavior on border.color { ColorAnimation { duration: Theme.animFast } }

    opacity: unreachable ? 0.45 : 1.0
    Behavior on opacity { NumberAnimation { duration: Theme.animFast } }

    // ── Mic glyph (custom paint with Canvas-like rectangles, no fonts) ──
    // Capsule body
    Rectangle {
        anchors.horizontalCenter: parent.horizontalCenter
        y: 9
        width: 10
        height: 14
        radius: 5
        color: {
            if (voiceState === "listening") return Theme.accent;
            if (voiceState === "speaking") return Theme.success;
            return Theme.text;
        }
        opacity: area.containsMouse || root.active ? 1.0 : 0.85

        Behavior on color { ColorAnimation { duration: Theme.animFast } }
    }

    // Stand / base
    Rectangle {
        anchors.horizontalCenter: parent.horizontalCenter
        y: 24
        width: 14
        height: 2
        radius: 1
        color: Theme.text
        opacity: area.containsMouse || root.active ? 1.0 : 0.6
    }
    Rectangle {
        anchors.horizontalCenter: parent.horizontalCenter
        y: 26
        width: 2
        height: 4
        radius: 1
        color: Theme.text
        opacity: area.containsMouse || root.active ? 1.0 : 0.6
    }

    // ── State-dependent overlays ──

    // Listening: pulsing ring.
    Rectangle {
        anchors.fill: parent
        radius: parent.radius
        color: "transparent"
        border.color: Theme.accent
        border.width: 2
        visible: voiceState === "listening"

        SequentialAnimation on opacity {
            running: parent.visible
            loops: Animation.Infinite
            NumberAnimation { from: 0.4; to: 1.0; duration: 600; easing.type: Easing.InOutSine }
            NumberAnimation { from: 1.0; to: 0.4; duration: 600; easing.type: Easing.InOutSine }
        }
    }

    // Processing: rotating dashed arc (cheap approximation via 3 dots
    // orbiting). Communicates "working on it" without a real spinner.
    Item {
        anchors.fill: parent
        visible: voiceState === "processing"
        RotationAnimator on rotation {
            from: 0
            to: 360
            duration: 1400
            loops: Animation.Infinite
            running: parent.visible
        }
        Repeater {
            model: 3
            Rectangle {
                width: 3; height: 3; radius: 1.5
                color: Theme.accent
                x: parent.width / 2 - width / 2
                y: 2
                transform: Rotation {
                    origin.x: width / 2
                    origin.y: parent.height / 2 - 2
                    angle: index * 120
                }
            }
        }
    }

    MouseArea {
        id: area
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        hoverEnabled: true
        enabled: !root.unreachable
        onClicked: VoiceBridge.toggle()
    }
}
