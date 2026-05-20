import QtQuick
import Jarvis.Shell

/// Wi-Fi indicator on the bar. Four vertical bars of increasing
/// height — cellular-signal-style. The number lit reflects the
/// active connection's signal strength.
///
/// Why bars instead of curved Wi-Fi arcs: arcs need
/// `QtQuick.Shapes` which is a heavier module + needs a path
/// definition. Bars are crisp at any DPI from plain Rectangles.
///
/// States:
///   - radio off          → all bars dim
///   - on, disconnected   → first bar lit dim, rest dim
///   - signal 1..33       → 1 bar lit, accent color
///   - signal 34..66      → 2 bars lit
///   - signal 67..100     → 3-4 bars lit (4 if signal >= 85)
///
/// Click opens the ConnectivityPanel.
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

    readonly property int signalLevel: {
        if (!NetworkBridge.wifiEnabled) return -1;
        const sig = NetworkBridge.activeConnection.signal;
        if (sig === undefined || sig === null) return 0;
        return sig;
    }
    readonly property bool connected: NetworkBridge.activeConnection.ssid !== undefined
    readonly property int litBars: {
        if (!NetworkBridge.wifiEnabled) return 0;
        if (!connected) return 0;
        if (signalLevel >= 85) return 4;
        if (signalLevel >= 67) return 3;
        if (signalLevel >= 34) return 2;
        if (signalLevel >= 1)  return 1;
        return 0;
    }

    Row {
        anchors.centerIn: parent
        spacing: 2

        Repeater {
            model: 4
            delegate: Rectangle {
                readonly property int idx: index
                readonly property bool lit: idx < root.litBars
                readonly property real h: 4 + idx * 3   // 4, 7, 10, 13 px

                width: 3
                height: h
                radius: 1
                anchors.verticalCenter: parent.verticalCenter
                color: lit
                    ? Theme.accent
                    : Theme.text
                opacity: lit
                    ? (area.containsMouse ? 1.0 : 0.95)
                    : (NetworkBridge.wifiEnabled ? 0.30 : 0.18)
                Behavior on opacity { NumberAnimation { duration: Theme.animFast } }
            }
        }
    }

    MouseArea {
        id: area
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        hoverEnabled: true
        onClicked: root.clicked()
    }
}
