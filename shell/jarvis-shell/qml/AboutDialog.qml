import QtQuick
import QtQuick.Window
import Jarvis.Shell

/// "Sobre este PC" — a small centered card opened from the Jarvis menu.
/// Static product info; no live system probing in V1 (the proactive
/// daemon already owns real telemetry, and a dialog doesn't need it).
PopupSurface {
    placement: "center"
    scope: "jarvis-dialog"
    dim: true
    id: root
    panelWidth: 360
    panelHeight: 240

    // Gate the spurious first deactivate wlroots fires before granting
    // focus (mirrors the Launcher), else the dialog self-closes on open.

    function requestOpen() {
        present();
    }
    function close() { dismiss(); }


    GlassPanel {
        anchors.fill: parent
        backdrop: true

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
                text: qsTr("LilithOS")
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
