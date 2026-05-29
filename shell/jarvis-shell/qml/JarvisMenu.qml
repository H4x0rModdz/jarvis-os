import QtQuick
import QtQuick.Window
import Jarvis.Shell

/// The Jarvis menu — the macOS Apple-menu analogue that drops from the
/// logo at the top-left of the menu bar. A separate frameless popup
/// Window (menus extend past the thin top-bar surface, so they can't
/// live inside it). Hosted by Main.qml, which positions it under the
/// logo and listens to its signals.
///
/// Power items dispatch the system.power Action Bus action; Sobre /
/// Configurações bubble up as signals so Main opens the dialog / panel
/// it owns.
Window {
    id: root
    width: 240
    height: col.implicitHeight + 16
    color: "transparent"
    flags: Qt.Tool | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
    visible: false

    signal aboutRequested()
    signal settingsRequested()

    // Suppress the spurious first deactivate that wlroots fires between
    // show and the compositor granting focus (same gate the Launcher
    // uses) — without it the menu closes the instant it opens.
    property bool _ignoreDeactivate: false

    function openAt(px, py) {
        x = px;
        y = py;
        _ignoreDeactivate = true;
        visible = true;
        requestActivate();
        armTimer.restart();
    }
    function close() { visible = false; }

    Timer { id: armTimer; interval: 250; onTriggered: root._ignoreDeactivate = false }
    onActiveChanged: if (!active && !_ignoreDeactivate && visible) close()

    Shortcut {
        sequence: "Escape"
        onActivated: root.close()
    }

    GlassPanel {
        anchors.fill: parent
        anchors.margins: 4

        Column {
            id: col
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: 6
            spacing: 0

            // Menu rows. `power` carries a system.power op; `divider`
            // draws a separator; otherwise a signal/handler fires.
            Repeater {
                model: [
                    { label: qsTr("Sobre este PC"),          kind: "about" },
                    { divider: true },
                    { label: qsTr("Configurações…"),         kind: "settings" },
                    { label: qsTr("Atualização do sistema"), kind: "update" },
                    { divider: true },
                    { label: qsTr("Bloquear tela"),          kind: "power", op: "lock" },
                    { label: qsTr("Suspender"),              kind: "power", op: "suspend" },
                    { label: qsTr("Reiniciar…"),             kind: "power", op: "reboot" },
                    { label: qsTr("Desligar…"),              kind: "power", op: "poweroff" }
                ]

                delegate: Item {
                    width: col.width
                    height: modelData.divider === true ? 9 : 30

                    // Separator line.
                    Rectangle {
                        visible: modelData.divider === true
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.leftMargin: 6
                        anchors.rightMargin: 6
                        height: 1
                        color: Theme.border
                    }

                    Rectangle {
                        visible: modelData.divider !== true
                        anchors.fill: parent
                        radius: 6
                        color: rowArea.containsMouse
                            ? Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.22)
                            : "transparent"

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.left: parent.left
                            anchors.leftMargin: 10
                            text: modelData.label || ""
                            color: Theme.text
                            font.pixelSize: 13
                        }

                        MouseArea {
                            id: rowArea
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                root.close();
                                switch (modelData.kind) {
                                case "about":    root.aboutRequested(); break;
                                case "settings": root.settingsRequested(); break;
                                case "update":   ActionBusBridge.dispatch("updater.check", "{}"); break;
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
