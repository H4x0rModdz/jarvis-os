import QtQuick
import QtQuick.Layouts
import QtQuick.Window
import Jarvis.Shell

/// Bluetooth panel — radio toggle, paired devices with
/// connect/disconnect/unpair per row, nearby devices found during
/// scan (click → pair). V1 is "just works" pairing only —
/// headphones / mice / keyboards from the last decade. Devices
/// that need a numeric passkey return an error in lastError;
/// supporting that flow is V2 work (in-process BlueZ agent).
///
/// Mirrors ConnectivityPanel's lifecycle: opens via the bar's
/// Bluetooth button, polls bluetoothctl every 5 s while visible,
/// stops on close.
Window {
    id: root
    visible: false
    width: 400
    height: 500
    color: "transparent"
    // Qt.Dialog (not Qt.Tool): under labwc a Tool window is non-activatable,
    // so it never gets keyboard focus — the Escape Shortcut never fires.
    // Qt.Dialog gets the activation the Launcher/JarvisMenu rely on.
    flags: Qt.Dialog | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
    title: qsTr("Bluetooth")

    // Suppress the spurious first deactivate wlroots fires between show and
    // the compositor granting focus — without it the panel closes instantly.
    property bool _ignoreDeactivate: false
    Timer { id: armTimer; interval: 250; onTriggered: root._ignoreDeactivate = false }

    function requestOpen() {
        if (Qt.application.screens.length > 0) {
            const s = Qt.application.screens[0];
            x = s.virtualX + s.width - width - 16;
            y = s.virtualY + s.height - height - Theme.barHeight - 16;
        }
        _ignoreDeactivate = true;
        visible = true;
        requestActivate();
        armTimer.restart();
        BluetoothBridge.startPolling();
    }

    Shortcut {
        sequence: "Escape"
        onActivated: root.visible = false
    }

    onVisibleChanged: {
        if (!visible) BluetoothBridge.stopPolling();
    }

    onActiveChanged: {
        if (!active && !_ignoreDeactivate && visible) root.visible = false;
    }

    GlassPanel {
        anchors.fill: parent
        anchors.margins: 8

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 18
            spacing: 12

            // ── Header + radio toggle + scan ─────────────────────────
            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                Text {
                    text: qsTr("BLUETOOTH")
                    color: Theme.accent
                    font.pixelSize: 11
                    font.weight: Font.Bold
                    font.letterSpacing: 2
                    Layout.fillWidth: true
                }

                Rectangle {
                    id: radioSwitch
                    implicitWidth: 44
                    implicitHeight: 24
                    radius: 12
                    color: BluetoothBridge.poweredOn ? Theme.accent : Qt.rgba(1, 1, 1, 0.08)
                    border.color: BluetoothBridge.poweredOn ? Theme.accent : Theme.border
                    border.width: 1
                    Behavior on color { ColorAnimation { duration: Theme.animFast } }

                    Rectangle {
                        width: 18; height: 18; radius: 9
                        color: Theme.text
                        anchors.verticalCenter: parent.verticalCenter
                        x: BluetoothBridge.poweredOn ? parent.width - width - 3 : 3
                        Behavior on x { NumberAnimation { duration: Theme.animFast; easing.type: Easing.OutCubic } }
                    }

                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: BluetoothBridge.setPowered(!BluetoothBridge.poweredOn)
                    }
                }

                Item {
                    implicitWidth: scanLabel.implicitWidth + 16
                    implicitHeight: 22
                    visible: BluetoothBridge.poweredOn

                    Rectangle {
                        anchors.fill: parent
                        radius: 11
                        color: scanArea.containsMouse
                            ? Qt.rgba(1, 1, 1, 0.08)
                            : Qt.rgba(1, 1, 1, 0.04)
                        border.color: Theme.border
                        border.width: 1
                    }
                    Text {
                        id: scanLabel
                        anchors.centerIn: parent
                        text: BluetoothBridge.scanning ? qsTr("…") : qsTr("BUSCAR")
                        color: Theme.textDim
                        font.pixelSize: 9
                        font.weight: Font.Bold
                        font.letterSpacing: 1
                    }
                    MouseArea {
                        id: scanArea
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        enabled: !BluetoothBridge.scanning
                        onClicked: BluetoothBridge.scan()
                    }
                }

                // Close — explicit affordance (Esc + click-outside
                // aren't reliable in a VM where focus can lag).
                Text {
                    text: "×"
                    color: closeArea.containsMouse ? Theme.danger : Theme.textDim
                    font.pixelSize: 20
                    font.weight: Font.Bold
                    Layout.alignment: Qt.AlignVCenter
                    MouseArea {
                        id: closeArea
                        anchors.fill: parent
                        anchors.margins: -6
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.visible = false
                    }
                }
            }

            // ── Radio-off state ──────────────────────────────────────
            Text {
                visible: !BluetoothBridge.poweredOn
                text: qsTr("Bluetooth desativado.")
                color: Theme.textDim
                font.pixelSize: 13
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                Layout.topMargin: 24
            }

            // ── Paired list ──────────────────────────────────────────
            ColumnLayout {
                visible: BluetoothBridge.poweredOn
                    && BluetoothBridge.pairedDevices.length > 0
                Layout.fillWidth: true
                spacing: 4

                Text {
                    text: qsTr("PAREADOS")
                    color: Theme.textDim
                    font.pixelSize: 10
                    font.weight: Font.Bold
                    font.letterSpacing: 1
                }

                Repeater {
                    model: BluetoothBridge.pairedDevices
                    delegate: Rectangle {
                        Layout.fillWidth: true
                        implicitHeight: pairedRow.implicitHeight + 12
                        radius: 8
                        color: modelData.connected
                            ? Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.12)
                            : Qt.rgba(1, 1, 1, 0.04)
                        border.color: modelData.connected
                            ? Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.45)
                            : Theme.border
                        border.width: 1

                        RowLayout {
                            id: pairedRow
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.leftMargin: 8
                            anchors.rightMargin: 8
                            spacing: 6

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 2
                                Text {
                                    text: modelData.name
                                    color: Theme.text
                                    font.pixelSize: 13
                                    font.weight: modelData.connected ? Font.Bold : Font.Normal
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                                Text {
                                    text: modelData.connected ? qsTr("Conectado") : modelData.mac
                                    color: modelData.connected ? Theme.success : Theme.textDim
                                    font.pixelSize: 10
                                    font.letterSpacing: modelData.connected ? 1 : 0
                                    font.weight: modelData.connected ? Font.Bold : Font.Normal
                                }
                            }

                            // Connect / disconnect pill.
                            Rectangle {
                                implicitWidth: connectLabel.implicitWidth + 14
                                implicitHeight: 22
                                radius: 11
                                color: connectArea.containsMouse
                                    ? Qt.rgba(1, 1, 1, 0.10)
                                    : Qt.rgba(1, 1, 1, 0.05)
                                border.color: Theme.border
                                border.width: 1

                                Text {
                                    id: connectLabel
                                    anchors.centerIn: parent
                                    text: modelData.connected ? qsTr("DESCONECTAR") : qsTr("CONECTAR")
                                    color: Theme.text
                                    font.pixelSize: 9
                                    font.weight: Font.Bold
                                    font.letterSpacing: 1
                                }
                                MouseArea {
                                    id: connectArea
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    enabled: !BluetoothBridge.busy
                                    onClicked: {
                                        if (modelData.connected) {
                                            BluetoothBridge.disconnectDevice(modelData.mac);
                                        } else {
                                            BluetoothBridge.connectDevice(modelData.mac);
                                        }
                                    }
                                }
                            }

                            // Unpair × button.
                            Text {
                                text: "×"
                                color: unpairArea.containsMouse ? Theme.danger : Theme.textDim
                                font.pixelSize: 16
                                font.weight: Font.Bold
                                Layout.preferredWidth: 18
                                horizontalAlignment: Text.AlignHCenter
                                MouseArea {
                                    id: unpairArea
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    enabled: !BluetoothBridge.busy
                                    onClicked: BluetoothBridge.unpair(modelData.mac)
                                }
                            }
                        }
                    }
                }
            }

            // ── Nearby list ──────────────────────────────────────────
            ColumnLayout {
                visible: BluetoothBridge.poweredOn
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 4

                Text {
                    text: qsTr("PRÓXIMOS")
                    color: Theme.textDim
                    font.pixelSize: 10
                    font.weight: Font.Bold
                    font.letterSpacing: 1
                }

                Text {
                    visible: BluetoothBridge.nearbyDevices.length === 0
                        && !BluetoothBridge.scanning
                    text: qsTr("Nenhum dispositivo. Clique BUSCAR.")
                    color: Theme.textDim
                    font.pixelSize: 12
                    font.italic: true
                }

                ListView {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    spacing: 4
                    model: BluetoothBridge.nearbyDevices

                    delegate: Rectangle {
                        width: ListView.view.width
                        implicitHeight: 36
                        radius: 8
                        color: nearbyArea.containsMouse
                            ? Qt.rgba(1, 1, 1, 0.05)
                            : Qt.rgba(1, 1, 1, 0.02)
                        border.color: Theme.border
                        border.width: 1

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 10
                            anchors.rightMargin: 10
                            spacing: 6

                            Text {
                                text: modelData.name
                                color: Theme.text
                                font.pixelSize: 12
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                            }
                            Text {
                                text: modelData.mac
                                color: Theme.textDim
                                font.pixelSize: 9
                            }
                        }

                        MouseArea {
                            id: nearbyArea
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            enabled: !BluetoothBridge.busy
                            onClicked: BluetoothBridge.pair(modelData.mac)
                        }
                    }
                }
            }

            // ── Error footer ────────────────────────────────────────
            Text {
                visible: BluetoothBridge.lastError.length > 0
                Layout.fillWidth: true
                text: BluetoothBridge.lastError
                color: Theme.danger
                font.pixelSize: 11
                wrapMode: Text.WordWrap
                horizontalAlignment: Text.AlignHCenter
            }
        }
    }
}
