import QtQuick
import QtQuick3D
import QtQuick3D.Helpers
import Jarvis.Shell

/// Reusable 3D avatar viewport (ADR 0028) — the View3D + VRM/fallback with no
/// window chrome. Embedded by the floating LilithAvatar window AND the desktop
/// HUD's Lilith center (ADR 0031). Reads the drive channels itself.
///
/// IMPORTANT: this imports QtQuick3D. Always instantiate it through a Loader so
/// a missing/old qt6-qtquick3d fails *contained* (the Loader goes to Error and
/// the parent surface keeps working) instead of taking the whole window/desktop
/// down with it — same isolation lesson as the avatar in Main.qml.
Item {
    id: root

    // One state string, same priority order the dock orb uses.
    readonly property string lilithState: {
        const v = VoiceBridge.state;
        if (v === "listening") return "listening";
        if (v === "speaking")  return "speaking";
        if (LilithBridge.busy || v === "processing") return "thinking";
        return "idle";
    }
    readonly property string emotion: LilithBridge.emotion   // neutral|happy|concerned
    readonly property real mouthOpen: VoiceBridge.mouthLevel // 0..1

    readonly property color accent: {
        switch (lilithState) {
        case "listening": return "#46d6ff";
        case "thinking":  return Theme.accent;
        case "speaking":  return Theme.success;
        default:
            if (emotion === "happy")     return Theme.success;
            if (emotion === "concerned") return Theme.danger;
            return Theme.accent;
        }
    }

    View3D {
        id: view
        anchors.fill: parent

        environment: SceneEnvironment {
            backgroundMode: SceneEnvironment.Transparent
            antialiasingMode: SceneEnvironment.MSAA
            antialiasingQuality: SceneEnvironment.High
        }

        PerspectiveCamera {
            id: camera
            position: Qt.vector3d(0, 40, 260)
            eulerRotation.x: -6
        }

        DirectionalLight {
            eulerRotation.x: -25
            eulerRotation.y: -15
            brightness: 1.0
        }
        PointLight {
            position: Qt.vector3d(140, 160, 220)
            brightness: 1.4
            color: root.accent
            Behavior on color { ColorAnimation { duration: Theme.animFast } }
        }

        // Real VRM drop-in at ~/.local/share/jarvis/avatar/lilith.vrm (path
        // resolved in C++ → AvatarModelUrl). Absent → fallback below.
        RuntimeLoader {
            id: vrm
            source: AvatarModelUrl
            visible: status === RuntimeLoader.Success
        }

        // Procedural fallback head — shown until a real VRM lands. Reacts to the
        // same channels. Proportions tuned on-device.
        Node {
            id: fallback
            visible: vrm.status !== RuntimeLoader.Success

            scale: Qt.vector3d(breath, breath, breath)
            property real breath: 1.0
            eulerRotation.z: root.emotion === "happy" ? 4
                           : root.emotion === "concerned" ? -5 : 0
            eulerRotation.y: spin
            property real spin: 0

            Behavior on eulerRotation.z { NumberAnimation { duration: 220; easing.type: Easing.OutCubic } }

            SequentialAnimation on breath {
                running: root.lilithState !== "idle"
                loops: Animation.Infinite
                NumberAnimation { to: 1.04; duration: 900; easing.type: Easing.InOutSine }
                NumberAnimation { to: 1.00; duration: 900; easing.type: Easing.InOutSine }
            }
            NumberAnimation on spin {
                running: root.lilithState === "thinking"
                from: -10; to: 10; duration: 1600
                loops: Animation.Infinite; easing.type: Easing.InOutSine
            }

            Model {
                source: "#Sphere"
                scale: Qt.vector3d(1.5, 1.6, 1.4)
                materials: PrincipledMaterial {
                    baseColor: Qt.rgba(0.16, 0.15, 0.22, 1.0)
                    metalness: 0.2
                    roughness: 0.45
                    emissiveFactor: Qt.vector3d(root.accent.r * 0.12,
                                                root.accent.g * 0.12,
                                                root.accent.b * 0.12)
                }
            }

            Repeater3D {
                model: [-22, 22]
                Model {
                    source: "#Sphere"
                    position: Qt.vector3d(modelData, 18, 64)
                    scale: Qt.vector3d(0.18, root.lilithState === "thinking" ? 0.06 : 0.18, 0.18)
                    Behavior on scale { Vector3dAnimation { duration: 150 } }
                    materials: PrincipledMaterial {
                        baseColor: root.accent
                        emissiveFactor: Qt.vector3d(root.accent.r, root.accent.g, root.accent.b)
                        Behavior on baseColor { ColorAnimation { duration: Theme.animFast } }
                    }
                }
            }

            Model {
                source: "#Cube"
                position: Qt.vector3d(0, -28, 64)
                scale: Qt.vector3d(0.42, 0.06 + root.mouthOpen * 0.46, 0.10)
                Behavior on scale { Vector3dAnimation { duration: 60 } }
                materials: PrincipledMaterial {
                    baseColor: Qt.rgba(0.85, 0.4, 0.5, 1.0)
                    emissiveFactor: Qt.vector3d(0.25, 0.05, 0.08)
                }
            }
        }
    }
}
