import QtQuick
import QtQuick.Window
import QtQuick3D
import QtQuick3D.Helpers
import Jarvis.Shell

/// Lilith's embodied presence — a floating, draggable 3D companion window
/// (ADR 0028). Frameless + transparent + always-on-top, sized to a small
/// corner tile so it doesn't block the desktop behind it.
///
/// Three drive channels feed the avatar, exactly as the ADR lays out:
///   - **state**   (VoiceBridge.state + LilithBridge.busy) → idle / listening /
///                 thinking / speaking → pose, motion, accent glow.
///   - **emotion** (LilithBridge.emotion) → neutral / happy / concerned →
///                 expression (here: accent colour + a small head tilt).
///   - **mouth**   (VoiceBridge.mouthLevel, 0–1) → how open the mouth is while
///                 she speaks (amplitude lip-flap; viseme timing is a later phase).
///
/// Rendering: a real VRM model is loaded at runtime from
/// `~/.local/share/jarvis/avatar/lilith.vrm` when present; until one is dropped
/// in, a procedural fallback head (Quick3D primitives) renders and animates off
/// the same channels so the whole pipeline is visible with no art asset.
/// Note: driving the *VRM's* morph targets (visemes/expressions) is Phase 2 —
/// for now the loaded model shows statically and the fallback is the animated one.
Window {
    id: root
    objectName: "jarvis-lilith-avatar"
    visible: true
    width: 260
    height: 340
    color: "transparent"
    // Qt.Dialog (not Tool): same reason as LilithPopup — Tool windows are
    // non-activatable under labwc. Frameless + stays-on-top = a companion overlay.
    flags: Qt.Dialog | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
    title: qsTr("Lilith")

    // First show: park bottom-right, clear of the floating dock (~90px tall).
    Component.onCompleted: {
        if (Qt.application.screens.length > 0) {
            const s = Qt.application.screens[0];
            x = s.virtualX + s.width - width - 32;
            y = s.virtualY + s.height - height - 120;
        }
    }

    // ── Drive channels ────────────────────────────────────────────────
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

    // Accent colour: state wins while she's active; emotion tints the idle face.
    readonly property color accent: {
        switch (lilithState) {
        case "listening": return "#46d6ff";          // cyan
        case "thinking":  return Theme.accent;        // purple
        case "speaking":  return Theme.success;       // green
        default:
            if (emotion === "happy")     return Theme.success;
            if (emotion === "concerned") return Theme.danger;
            return Theme.accent;
        }
    }

    // ── 3D viewport ───────────────────────────────────────────────────
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
        // Accent-coloured fill light so the whole avatar shifts hue with state.
        PointLight {
            position: Qt.vector3d(140, 160, 220)
            brightness: 1.4
            color: root.accent
            Behavior on color { ColorAnimation { duration: Theme.animFast } }
        }

        // Real VRM model — drop-in at ~/.local/share/jarvis/avatar/lilith.vrm.
        // Resolves the user data dir via QtCore.StandardPaths; if the file is
        // absent/unreadable, status != Success and the fallback below shows.
        RuntimeLoader {
            id: vrm
            // ~/.local/share/jarvis/avatar/lilith.vrm — resolved in C++ and
            // injected as a context property (see main.cpp). Absent → status
            // != Success → the procedural fallback below shows.
            source: AvatarModelUrl
            visible: status === RuntimeLoader.Success
        }

        // ── Procedural fallback head ──────────────────────────────────
        // Shown until a real VRM lands. A stylized head + eyes + mouth that
        // react to the same channels. Proportions are eyeballed and meant to
        // be tuned on-device (no GPU/preview on the build host — ADR 0028).
        Node {
            id: fallback
            visible: vrm.status !== RuntimeLoader.Success

            // Idle breathing + a small tilt that leans into the emotion.
            scale: Qt.vector3d(breath, breath, breath)
            property real breath: 1.0
            eulerRotation.z: root.emotion === "happy" ? 4
                           : root.emotion === "concerned" ? -5 : 0
            eulerRotation.y: spin
            property real spin: 0

            Behavior on eulerRotation.z { NumberAnimation { duration: 220; easing.type: Easing.OutCubic } }

            // Gentle breathing while active; still when idle+silent.
            SequentialAnimation on breath {
                running: root.lilithState !== "idle"
                loops: Animation.Infinite
                NumberAnimation { to: 1.04; duration: 900; easing.type: Easing.InOutSine }
                NumberAnimation { to: 1.00; duration: 900; easing.type: Easing.InOutSine }
            }
            // Slow turn while thinking, to read as "working".
            NumberAnimation on spin {
                running: root.lilithState === "thinking"
                from: -10; to: 10; duration: 1600
                loops: Animation.Infinite; easing.type: Easing.InOutSine
            }

            // Head.
            Model {
                source: "#Sphere"
                scale: Qt.vector3d(1.5, 1.6, 1.4)
                materials: PrincipledMaterial {
                    baseColor: Qt.rgba(0.16, 0.15, 0.22, 1.0)
                    metalness: 0.2
                    roughness: 0.45
                    // Faint self-glow in the accent so she reads as "alive".
                    emissiveFactor: Qt.vector3d(root.accent.r * 0.12,
                                                root.accent.g * 0.12,
                                                root.accent.b * 0.12)
                }
            }

            // Eyes — two glowing accent dots near the front of the head.
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

            // Mouth — a bar whose height tracks the speech amplitude. Closed
            // (thin) at rest; opens with mouthOpen while she talks.
            Model {
                source: "#Cube"
                position: Qt.vector3d(0, -28, 64)
                // Width fixed; height = base + amplitude; small depth.
                scale: Qt.vector3d(0.42, 0.06 + root.mouthOpen * 0.46, 0.10)
                Behavior on scale { Vector3dAnimation { duration: 60 } }
                materials: PrincipledMaterial {
                    baseColor: Qt.rgba(0.85, 0.4, 0.5, 1.0)
                    emissiveFactor: Qt.vector3d(0.25, 0.05, 0.08)
                }
            }
        }
    }

    // ── Drag to reposition; click (no drag) opens the conversation ─────
    MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        property point pressPos
        property bool dragging: false
        onPressed: (mouse) => { pressPos = Qt.point(mouse.x, mouse.y); dragging = false; }
        onPositionChanged: (mouse) => {
            const dx = mouse.x - pressPos.x;
            const dy = mouse.y - pressPos.y;
            if (!dragging && Math.hypot(dx, dy) > 6) dragging = true;
            if (dragging) {
                root.x += dx;
                root.y += dy;
            }
        }
        onReleased: { if (!dragging) ShellBus.toggleLilith(); }
    }
}
