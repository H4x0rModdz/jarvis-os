import QtQuick
import QtQuick.Layouts
import Jarvis.Shell

/// The translucent bar — Phase 1a uses a regular toplevel rectangle.
/// Phase 1b swaps the host Window for a wlr-layer-shell surface and anchors
/// this same component to the bottom edge of every output.
Rectangle {
    id: root
    color: Theme.surface
    border.color: Theme.border
    border.width: 1
    radius: Theme.radius

    // Subtle top highlight — fakes a soft inner light source.
    Rectangle {
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.margins: 1
        height: 1
        color: Qt.rgba(1, 1, 1, 0.06)
        radius: Theme.radius
    }

    signal launcherRequested()
    signal settingsRequested()
    signal notificationsRequested()
    signal networksRequested()
    signal bluetoothRequested()
    /// Fired when the LilithInput gains focus — Main.qml routes this
    /// to the LilithPopup so the empty-state suggestions get a chance
    /// to be seen.
    signal lilithFocused()

    function focusInput() {
        input.focusInput();
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Theme.pad
        anchors.rightMargin: Theme.pad
        spacing: Theme.gap

        BarMenuButton {
            Layout.alignment: Qt.AlignVCenter
            onClicked: root.launcherRequested()
        }

        Clock {
            Layout.alignment: Qt.AlignVCenter
            Layout.preferredWidth: 110
        }

        // Vertical divider between clock and input
        Rectangle {
            Layout.preferredWidth: 1
            Layout.preferredHeight: parent.height * 0.5
            Layout.alignment: Qt.AlignVCenter
            color: Theme.border
        }

        MicButton {
            Layout.alignment: Qt.AlignVCenter
        }

        LilithInput {
            id: input
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignVCenter

            onAccepted: function(text) {
                LilithBridge.send(text);
                root.lastUserText = text;
            }
            onInputFocused: root.lilithFocused()
        }

        // Hotword pipeline. When the daemon matches a wake-word the
        // bridge splits the transcript into (wake-word, remainder).
        // Remainder present  → dispatch straight to Lilith (one-shot
        //                       "oi lilith abre o navegador").
        // Remainder empty    → engage the mic so the user can speak
        //                       the command body — same path as the
        //                       MicButton click. The TranscriptionFinal
        //                       handler in LilithBridge wires the
        //                       second leg.
        Connections {
            target: VoiceBridge
            function onWakeWordTriggered(fullTranscript, remainder) {
                if (remainder.length > 0) {
                    LilithBridge.send(remainder);
                    root.lastUserText = remainder;
                } else {
                    VoiceBridge.toggle();
                }
            }
            // Pipe push-to-talk transcripts into Lilith automatically
            // when they originated from a hotword cycle. The original
            // user-typed flow already runs through onAccepted above.
            function onLastTranscriptChanged() {
                const t = VoiceBridge.lastTranscript.trim();
                if (t.length === 0) return;
                LilithBridge.send(t);
                root.lastUserText = t;
            }
        }

        StatusIndicator {
            Layout.alignment: Qt.AlignVCenter
            reachable: LilithBridge.reachable
            busy: LilithBridge.busy
        }

        BarWifiButton {
            Layout.alignment: Qt.AlignVCenter
            onClicked: root.networksRequested()
        }

        BarBluetoothButton {
            Layout.alignment: Qt.AlignVCenter
            onClicked: root.bluetoothRequested()
        }

        BarBatteryIndicator {
            Layout.alignment: Qt.AlignVCenter
        }

        BarBellButton {
            Layout.alignment: Qt.AlignVCenter
            onClicked: root.notificationsRequested()
        }

        BarGearButton {
            Layout.alignment: Qt.AlignVCenter
            onClicked: root.settingsRequested()
        }
    }

    property string lastUserText: ""
}
