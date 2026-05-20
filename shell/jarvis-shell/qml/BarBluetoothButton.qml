import QtQuick
import Jarvis.Shell

/// Bluetooth indicator in the bar. Stylized "B" rune that we draw
/// from rectangles + diagonals — recognizable as the Bluetooth
/// glyph at a glance without pulling in an icon font.
///
/// Visual states:
///   - radio off          → glyph dim
///   - on, nothing paired → glyph normal
///   - device connected   → glyph accent + status dot
///
/// Click signals up to Main.qml which opens the BluetoothPanel.
Rectangle {
    id: root
    implicitWidth: 32
    implicitHeight: 32
    radius: 8
    color: area.containsMouse ? Qt.rgba(1, 1, 1, 0.08) : "transparent"
    border.color: area.containsMouse ? Theme.border : "transparent"
    border.width: 1
    Behavior on color { ColorAnimation { duration: Theme.animFast } }
    Behavior on border.color { ColorAnimation { duration: Theme.animFast } }

    signal clicked()

    /// Any paired device currently connected? Powers the accent
    /// color + status dot.
    readonly property bool anyConnected: {
        for (const dev of BluetoothBridge.pairedDevices) {
            if (dev.connected) return true;
        }
        return false;
    }

    Item {
        anchors.centerIn: parent
        width: 14
        height: 18
        opacity: BluetoothBridge.poweredOn
            ? (area.containsMouse ? 1.0 : 0.95)
            : 0.30
        Behavior on opacity { NumberAnimation { duration: Theme.animFast } }

        readonly property color stroke: root.anyConnected
            ? Theme.accent
            : Theme.text

        // Bluetooth rune is roughly a vertical bar with two
        // back-to-back triangles meeting at the centre. We draw it
        // as: vertical line + two diagonal lines for each triangle.

        // Vertical bar (the spine of the rune).
        Rectangle {
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            width: 1.5
            color: parent.stroke
            radius: 0.75
        }

        // Top diagonal: from top-centre to right-middle.
        Rectangle {
            x: parent.width / 2 - 1
            y: 1
            width: 6
            height: 1.5
            radius: 0.75
            color: parent.stroke
            transform: Rotation { origin.x: 0; origin.y: 0.75; angle: 45 }
        }

        // Bottom diagonal: from bottom-centre to right-middle.
        Rectangle {
            x: parent.width / 2 - 1
            y: parent.height - 1
            width: 6
            height: 1.5
            radius: 0.75
            color: parent.stroke
            transform: Rotation { origin.x: 0; origin.y: 0.75; angle: -45 }
        }

        // Inner verticals that complete the two triangles. These
        // are the lines from the right-middle endpoints back to the
        // centre vertical at quarter heights.
        Rectangle {
            x: parent.width / 2 - 1
            y: 1
            width: 6
            height: 1.5
            radius: 0.75
            color: parent.stroke
            transform: Rotation { origin.x: 6; origin.y: 0.75; angle: -45 }
        }
        Rectangle {
            x: parent.width / 2 - 1
            y: parent.height - 1
            width: 6
            height: 1.5
            radius: 0.75
            color: parent.stroke
            transform: Rotation { origin.x: 6; origin.y: 0.75; angle: 45 }
        }
    }

    // Tiny connection-status dot bottom-right.
    Rectangle {
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.rightMargin: 4
        anchors.bottomMargin: 4
        width: 4
        height: 4
        radius: 2
        color: Theme.accent
        visible: root.anyConnected
    }

    MouseArea {
        id: area
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        hoverEnabled: true
        onClicked: root.clicked()
    }
}
