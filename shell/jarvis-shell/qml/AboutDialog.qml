import QtQuick
import QtQuick.Window
import Jarvis.Shell

/// "Sobre este PC" — a small centered card opened from the Jarvis menu.
/// Static product info; no live system probing in V1 (the proactive
/// daemon already owns real telemetry, and a dialog doesn't need it).
Window {
    id: root
    width: 360
    height: 240
    color: "transparent"
    flags: Qt.Dialog | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
    visible: false

    // Gate the spurious first deactivate wlroots fires before granting
    // focus (mirrors the Launcher), else the dialog self-closes on open.
    property bool _ignoreDeactivate: false

    function requestOpen() {
        if (Qt.application.screens.length > 0) {
            const s = Qt.application.screens[0];
            x = s.virtualX + Math.floor((s.width - width) / 2);
            y = s.virtualY + Math.floor((s.height - height) / 2);
        }
        _ignoreDeactivate = true;
        visible = true;
        requestActivate();
        armTimer.restart();
    }
    function close() { visible = false; }

    Timer { id: armTimer; interval: 250; onTriggered: root._ignoreDeactivate = false }
    onActiveChanged: if (!active && !_ignoreDeactivate && visible) close()
    Shortcut { sequence: "Escape"; onActivated: root.close() }

    GlassPanel {
        anchors.fill: parent
        anchors.margins: 8

        Column {
            anchors.centerIn: parent
            spacing: 10
            width: parent.width - 48

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: "◈"
                color: Theme.accent
                font.pixelSize: 44
            }
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: qsTr("Jarvis OS")
                color: Theme.text
                font.pixelSize: 20
                font.weight: Font.Bold
            }
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: qsTr("Sistema operacional nativo de IA")
                color: Theme.textDim
                font.pixelSize: 12
            }
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: qsTr("Lilith — assistente integrada")
                color: Theme.textDim
                font.pixelSize: 12
            }
        }

        // Close ×.
        Text {
            anchors.top: parent.top
            anchors.right: parent.right
            anchors.margins: 14
            text: "×"
            color: closeArea.containsMouse ? Theme.text : Theme.textDim
            font.pixelSize: 20
            font.weight: Font.Bold
            MouseArea {
                id: closeArea
                anchors.fill: parent
                anchors.margins: -6
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.close()
            }
        }
    }
}
