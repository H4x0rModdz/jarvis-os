import QtQuick
import QtQuick3D
// RuntimeLoader (the VRM/glTF drop-in below) lives in AssetUtils, NOT in
// Helpers. Importing Helpers made the whole component fail to compile with
// "RuntimeLoader is not a type", which the Loader reported as QtQuick3D being
// unavailable — so the avatar silently never rendered even with qt6-qtquick3d
// installed. Helpers is not used here at all.
import QtQuick3D.AssetUtils
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

        // The user's VRM, dropped at ~/.local/share/jarvis/avatar/lilith.vrm
        // (path resolved in C++ → AvatarModelUrl). This view is only ever
        // instantiated when that file exists — see AvatarModelPresent — so
        // there is no placeholder to fall back to and nothing renders when the
        // user hasn't supplied a model.
        RuntimeLoader {
            id: vrm
            source: AvatarModelUrl
            visible: status === RuntimeLoader.Success
        }
    }
}
