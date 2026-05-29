import QtQuick
import Jarvis.Shell

/// Lilith's presence in the dock. A glyph orb whose symbol encodes
/// state — the set the user asked for:
///   ◉    idle       — solid, calm
///   ◎    listening   — hollow ring (mic engaged)
///   ◌◌◌  thinking    — dotted, animated (Lilith busy / voice processing)
///   ◉◉◉  speaking    — triple solid, pulsing (TTS playing)
///
/// Folds in the retired bar's status LED (color = reachability/state),
/// mic button (press-and-hold = push-to-talk) and the popup trigger
/// (click → ShellBus.toggleLilith, which Main.qml routes to LilithPopup).
Item {
    id: root
    implicitWidth: 56
    implicitHeight: 56

    // One state string from the two bridges. Priority: listening (mic
    // live) > speaking (TTS out) > thinking (busy/processing) > idle.
    // VoiceBridge.state ∈ {idle, listening, processing, speaking}.
    readonly property string lilithState: {
        const v = VoiceBridge.state;
        if (v === "listening") return "listening";
        if (v === "speaking")  return "speaking";
        if (LilithBridge.busy || v === "processing") return "thinking";
        return "idle";
    }

    readonly property color stateColor: {
        switch (lilithState) {
        case "listening": return "#46d6ff";                        // cyan
        case "thinking":  return Theme.accent;                     // purple
        case "speaking":  return Theme.success;                    // green
        default:          return LilithBridge.reachable ? Theme.accent : Theme.textDim;
        }
    }

    readonly property string glyph: {
        switch (lilithState) {
        case "listening": return "◎";
        case "thinking":  return "◌◌◌";
        case "speaking":  return "◉◉◉";
        default:          return "◉";
        }
    }

    // Circular glass backplate so the orb reads as a dock tile.
    Rectangle {
        id: plate
        anchors.centerIn: parent
        width: 48
        height: 48
        radius: 24
        color: area.containsMouse ? Qt.rgba(1, 1, 1, 0.10) : Qt.rgba(1, 1, 1, 0.05)
        border.width: 1
        border.color: Qt.rgba(root.stateColor.r, root.stateColor.g, root.stateColor.b, 0.55)
        Behavior on border.color { ColorAnimation { duration: Theme.animFast } }

        // Breathing while active (not idle).
        SequentialAnimation on scale {
            running: root.lilithState !== "idle"
            loops: Animation.Infinite
            NumberAnimation { to: 1.06; duration: 700; easing.type: Easing.InOutSine }
            NumberAnimation { to: 1.00; duration: 700; easing.type: Easing.InOutSine }
        }
    }

    Text {
        anchors.centerIn: parent
        text: root.glyph
        color: root.stateColor
        // Single-glyph states want a big symbol; the triple-glyph states
        // (◌◌◌ / ◉◉◉) want a smaller size so they fit the plate.
        font.pixelSize: (root.lilithState === "idle" || root.lilithState === "listening") ? 24 : 13
        font.letterSpacing: 1
        Behavior on color { ColorAnimation { duration: Theme.animFast } }

        // Pulse the multi-dot glyphs so they read as "working".
        SequentialAnimation on opacity {
            running: root.lilithState === "thinking" || root.lilithState === "speaking"
            loops: Animation.Infinite
            NumberAnimation { to: 0.45; duration: 500; easing.type: Easing.InOutSine }
            NumberAnimation { to: 1.00; duration: 500; easing.type: Easing.InOutSine }
        }
    }

    // Hover / active label above the orb.
    Text {
        anchors.bottom: parent.top
        anchors.horizontalCenter: parent.horizontalCenter
        text: {
            switch (root.lilithState) {
            case "listening": return qsTr("Ouvindo…");
            case "thinking":  return qsTr("Pensando…");
            case "speaking":  return qsTr("Falando…");
            default:          return qsTr("Lilith");
            }
        }
        color: Theme.text
        font.pixelSize: 10
        opacity: (area.containsMouse || root.lilithState !== "idle") ? 0.9 : 0.0
        Behavior on opacity { NumberAnimation { duration: Theme.animFast } }
        style: Text.Outline
        styleColor: Qt.rgba(0, 0, 0, 0.6)
    }

    MouseArea {
        id: area
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        // Click toggles the conversation popup; press-and-hold engages
        // push-to-talk (same VoiceBridge.toggle the old MicButton fired).
        onClicked: ShellBus.toggleLilith()
        onPressAndHold: VoiceBridge.toggle()
    }
}
