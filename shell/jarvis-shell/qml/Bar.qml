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
        }

        StatusIndicator {
            Layout.alignment: Qt.AlignVCenter
            reachable: LilithBridge.reachable
            busy: LilithBridge.busy
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
