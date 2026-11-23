import QtQuick
import Jarvis.Shell

/// Lilith's presence in the dock: a luminous orb whose colour and motion
/// carry her state.
///   idle       — calm blue→violet, still
///   listening  — cyan, breathing (mic engaged)
///   thinking   — violet, breathing + a slow sweep of light
///   speaking   — green, pulsing with the voice
///   offline    — grey
///
/// Click toggles the conversation popup (ShellBus.toggleLilith); press and
/// hold engages push-to-talk (VoiceBridge.toggle).
Item {
    id: root
    implicitWidth: Theme.dockIcon + 6
    implicitHeight: Theme.dockIcon + 10

    // One state string from the two bridges. Priority: listening (mic
    // live) > speaking (TTS out) > thinking (busy/processing) > idle.
    readonly property string lilithState: {
        const v = VoiceBridge.state;
        if (v === "listening") return "listening";
        if (v === "speaking")  return "speaking";
        if (LilithBridge.busy || v === "processing") return "thinking";
        return "idle";
    }
    readonly property bool active: lilithState !== "idle"

    // Top and bottom of the orb's gradient, per state.
    readonly property color hi: {
        if (!LilithBridge.reachable && !active) return "#8e8e93";
        switch (lilithState) {
        case "listening": return "#9be7ff";
        case "speaking":  return "#8bf5a6";
        case "thinking":  return "#d0a6ff";
        default:          return "#7fd4ff";
        }
    }
    readonly property color lo: {
        if (!LilithBridge.reachable && !active) return "#48484a";
        switch (lilithState) {
        case "listening": return "#0a84ff";
        case "speaking":  return "#248a3d";
        case "thinking":  return "#5e5ce6";
        default:          return "#5e5ce6";
        }
    }

    Item {
        id: tile
        width: Theme.dockIcon
        height: Theme.dockIcon
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        transformOrigin: Item.Bottom
        scale: area.pressed ? 1.08 : (area.containsMouse ? 1.22 : 1.0)
        Behavior on scale { NumberAnimation { duration: Theme.animFast; easing.type: Easing.OutCubic } }

        Rectangle {
            id: orb
            anchors.fill: parent
            anchors.margins: 4
            radius: width / 2
            gradient: Gradient {
                GradientStop { position: 0.0; color: root.hi; Behavior on color { ColorAnimation { duration: Theme.animNormal } } }
                GradientStop { position: 1.0; color: root.lo; Behavior on color { ColorAnimation { duration: Theme.animNormal } } }
            }
            border.width: 1
            border.color: Qt.rgba(1, 1, 1, 0.35)

            // Glass highlight: a soft bright cap on the upper half.
            Rectangle {
                anchors.horizontalCenter: parent.horizontalCenter
                y: parent.height * 0.08
                width: parent.width * 0.62
                height: parent.height * 0.34
                radius: height / 2
                gradient: Gradient {
                    GradientStop { position: 0.0; color: Qt.rgba(1, 1, 1, 0.55) }
                    GradientStop { position: 1.0; color: Qt.rgba(1, 1, 1, 0.0) }
                }
            }

            // Breathing while she is doing something. Nothing animates at
            // rest — an always-running animation keeps the dock rendering.
            SequentialAnimation on scale {
                running: root.active
                loops: Animation.Infinite
                alwaysRunToEnd: true
                NumberAnimation { to: 1.07; duration: 650; easing.type: Easing.InOutSine }
                NumberAnimation { to: 1.00; duration: 650; easing.type: Easing.InOutSine }
            }
        }
    }

    // State label above the dock, in the same bubble as the app names.
    Rectangle {
        anchors.bottom: tile.top
        anchors.bottomMargin: 16
        anchors.horizontalCenter: parent.horizontalCenter
        width: stateLabel.implicitWidth + 18
        height: 24
        radius: 12
        color: Theme.glassSolid
        border.width: 1
        border.color: Theme.glassRim
        opacity: (area.containsMouse || root.active) ? 1.0 : 0.0
        visible: opacity > 0
        Behavior on opacity { NumberAnimation { duration: Theme.animFast } }
        Text {
            id: stateLabel
            anchors.centerIn: parent
            text: {
                switch (root.lilithState) {
                case "listening": return qsTr("Ouvindo…");
                case "thinking":  return qsTr("Pensando…");
                case "speaking":  return qsTr("Falando…");
                default:          return qsTr("Lilith");
                }
            }
            color: Theme.text
            font.pixelSize: Theme.fontCaption + 1
        }
    }

    MouseArea {
        id: area
        anchors.fill: parent
        hoverEnabled: true
        onClicked: ShellBus.toggleLilith()
        onPressAndHold: VoiceBridge.toggle()
    }
}
