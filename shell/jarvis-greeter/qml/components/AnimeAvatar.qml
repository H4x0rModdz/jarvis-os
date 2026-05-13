import QtQuick

/// Lilith's avatar slot. State-driven sprite swap with a procedural
/// fallback for V1 — until the real Lilith character art lands the
/// avatar renders a glow column that pulses with `state`. The instant
/// the right PNG files exist under `qrc:/avatar/`, the sprite path
/// engages automatically.
///
/// Asset hand-off contract (V2):
///   qrc:/avatar/lilith-idle.png       — neutral pose
///   qrc:/avatar/lilith-talking.png    — mid-speech pose
///   qrc:/avatar/lilith-listening.png  — listening / leaning-in pose
/// 256 × 360 px portrait, transparent background, single subject
/// centered. PNG-8 with alpha is fine — the avatar is rendered at
/// 140 × 200 and below, so 512×720 is wasted bytes.
///
/// While the assets are absent the procedural fallback keeps the
/// composition believable: same proportions as the real art will
/// fill, same accent color, breathing animation tuned to the state.
Item {
    id: root

    /// `"idle" | "talking" | "listening"`. Drives both the procedural
    /// pulse and which sprite is shown when the assets are present.
    property string state: "idle"

    implicitWidth: 140
    implicitHeight: 200

    // Real sprite path. Image.status falls back to Image.Null when
    // the resource isn't compiled in, so we use that as the
    // "use procedural" switch — no manual check needed.
    Image {
        id: sprite
        anchors.fill: parent
        fillMode: Image.PreserveAspectFit
        smooth: true
        cache: true
        asynchronous: true
        source: {
            switch (root.state) {
                case "talking":   return "qrc:/avatar/lilith-talking.png";
                case "listening": return "qrc:/avatar/lilith-listening.png";
                default:          return "qrc:/avatar/lilith-idle.png";
            }
        }
        visible: status === Image.Ready
        opacity: visible ? 1.0 : 0.0
        Behavior on opacity {
            NumberAnimation { duration: 200 }
        }
    }

    // Procedural fallback — runs whenever the Image above isn't
    // Ready. Same composition (vertical figure) so the rest of the
    // layout doesn't re-flow when real assets arrive.
    Item {
        id: procedural
        anchors.fill: parent
        visible: sprite.status !== Image.Ready

        Rectangle {
            id: column
            anchors.centerIn: parent
            width: parent.width * 0.7
            height: parent.height
            radius: width / 2
            color: Qt.rgba(0.49, 0.36, 1.0, 0.18)
            border.color: "#7c5cff"
            border.width: 1

            // Breathing animation that intensifies with state. The
            // numbers are deliberate: idle is barely visible (so the
            // greeter doesn't feel like it's nagging you to speak),
            // talking is brisk, listening sits in between.
            SequentialAnimation on opacity {
                loops: Animation.Infinite
                NumberAnimation {
                    to: {
                        switch (root.state) {
                            case "talking":   return 1.0;
                            case "listening": return 0.85;
                            default:          return 0.7;
                        }
                    }
                    duration: {
                        switch (root.state) {
                            case "talking":   return 500;
                            case "listening": return 800;
                            default:          return 1500;
                        }
                    }
                    easing.type: Easing.InOutSine
                }
                NumberAnimation {
                    to: {
                        switch (root.state) {
                            case "talking":   return 0.6;
                            case "listening": return 0.5;
                            default:          return 0.4;
                        }
                    }
                    duration: {
                        switch (root.state) {
                            case "talking":   return 500;
                            case "listening": return 800;
                            default:          return 1500;
                        }
                    }
                    easing.type: Easing.InOutSine
                }
            }
        }

        Text {
            anchors.centerIn: parent
            text: "Lilith"
            color: "#e8eaed"
            font.pixelSize: 20
            font.weight: Font.Bold
            font.italic: true
            opacity: 0.85
        }
    }
}
