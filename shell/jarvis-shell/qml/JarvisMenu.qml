import QtQuick
import QtQuick.Window
import Jarvis.Shell

/// The system menu — drops from the mark at the left end of the menu bar:
/// about, settings, updates, and the power actions. Its own layer surface
/// anchored to the top-left corner (a menu extends past the thin bar surface,
/// so it cannot live inside it).
///
/// Power items dispatch the `system.power` Action Bus action; Sobre /
/// Configurações bubble up as signals so Main opens the window it owns.
PopupSurface {
    id: root
    placement: "top-left"
    scope: "jarvis-menu"
    panelWidth: 250
    panelHeight: col.implicitHeight + 10


    signal aboutRequested()
    signal settingsRequested()

    function open() { present(); }
    function close() { dismiss(); }

    GlassPanel {
        anchors.fill: parent
        radius: 12
        backdrop: true

        Column {
            id: col
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: 5
            spacing: 0

            // `power` rows carry a system.power op; `divider` draws a
            // separator; the rest raise a signal.
            Repeater {
                model: [
                    { label: qsTr("Sobre o LilithOS"),       kind: "about" },
                    { divider: true },
                    { label: qsTr("Ajustes do Sistema…"),    kind: "settings" },
                    { label: qsTr("Atualização de Software…"), kind: "update" },
                    { divider: true },
                    { label: qsTr("Repousar"),               kind: "power", op: "suspend" },
                    { label: qsTr("Reiniciar…"),             kind: "power", op: "reboot" },
                    { label: qsTr("Desligar…"),              kind: "power", op: "poweroff" },
                    { divider: true },
                    { label: qsTr("Bloquear Tela"),          kind: "power", op: "lock", hint: "Super L" }
                ]

                delegate: Item {
                    width: col.width
                    height: modelData.divider === true ? 11 : 24

                    Rectangle {
                        visible: modelData.divider === true
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.leftMargin: 9
                        anchors.rightMargin: 9
                        height: 1
                        color: Theme.separator
                    }

                    Rectangle {
                        visible: modelData.divider !== true
                        anchors.fill: parent
                        radius: 6
                        color: rowArea.containsMouse ? Theme.accent : "transparent"

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.left: parent.left
                            anchors.leftMargin: 10
                            text: modelData.label || ""
                            color: Theme.text
                            font.pixelSize: Theme.fontBody
                        }
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.right: parent.right
                            anchors.rightMargin: 10
                            visible: modelData.hint !== undefined
                            text: modelData.hint || ""
                            color: rowArea.containsMouse ? Theme.text : Theme.textDim
                            font.pixelSize: Theme.fontBody
                        }

                        MouseArea {
                            id: rowArea
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: {
                                root.close();
                                switch (modelData.kind) {
                                case "about":    root.aboutRequested(); break;
                                case "settings": root.settingsRequested(); break;
                                case "update":   UpdaterBridge.checkNow(); break;
                                case "power":
                                    ActionBusBridge.dispatch(
                                        "system.power",
                                        JSON.stringify({ "op": modelData.op }));
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
