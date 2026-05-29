import QtQuick
import QtQuick.Layouts
import Jarvis.Shell

/// The top menu bar content. Left: the Jarvis logo (opens the Jarvis
/// menu). Right: the reused indicator buttons + clock. Replaces the
/// retired bottom Bar.qml; main.cpp anchors the host window to the top
/// edge of every output.
Rectangle {
    id: root
    color: Theme.surface

    // A single hairline along the bottom edge reads cleaner for a top
    // bar than a full border box.
    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: 1
        color: Theme.border
    }

    signal jarvisMenuRequested()
    signal networksRequested()
    signal bluetoothRequested()
    signal notificationsRequested()
    signal settingsRequested()

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 10
        anchors.rightMargin: 12
        spacing: 8

        // ── Jarvis logo (Apple-menu analogue) ──────────────────────
        Item {
            Layout.alignment: Qt.AlignVCenter
            Layout.preferredWidth: 26
            Layout.fillHeight: true

            Text {
                anchors.centerIn: parent
                text: "◈"
                color: logoArea.containsMouse ? Theme.accent : Theme.text
                font.pixelSize: 16
                Behavior on color { ColorAnimation { duration: Theme.animFast } }
            }
            MouseArea {
                id: logoArea
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.jarvisMenuRequested()
            }
        }

        // Spacer pushes the indicators to the right edge.
        Item { Layout.fillWidth: true }

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

        Rectangle {
            Layout.preferredWidth: 1
            Layout.preferredHeight: parent.height * 0.55
            Layout.alignment: Qt.AlignVCenter
            color: Theme.border
        }

        Clock {
            Layout.alignment: Qt.AlignVCenter
            Layout.preferredWidth: 96
        }
    }
}
