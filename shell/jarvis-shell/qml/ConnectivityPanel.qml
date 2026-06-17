import QtQuick
import QtQuick.Layouts
import QtQuick.Window
import Jarvis.Shell

/// Wi-Fi panel — list of nearby networks, the active one badged,
/// inline password row for joining secured ones. Wi-Fi toggle at
/// top. Opens via the bar's Wi-Fi button (networksRequested).
///
/// Polling lifecycle: starts on `requestOpen`, stops on hide so
/// we're not forking nmcli every 5 s when the panel isn't visible.
Window {
    id: root
    visible: false
    width: 380
    height: 480
    color: "transparent"
    // Qt.Dialog (not Qt.Tool): under labwc a Tool window is non-activatable,
    // so it never gets keyboard focus — the Escape Shortcut never fires.
    // Qt.Dialog gets the activation the Launcher/JarvisMenu rely on.
    flags: Qt.Dialog | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
    title: qsTr("Wi-Fi")

    /// SSID currently being password-edited, or "" when no row is in
    /// the edit state.
    property string editingSsid: ""

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
        editingSsid = "";
        _ignoreDeactivate = true;
        visible = true;
        requestActivate();
        armTimer.restart();
        NetworkBridge.startPolling();
        NetworkBridge.scan();
    }

    Shortcut {
        sequence: "Escape"
        onActivated: root.visible = false
    }

    onVisibleChanged: {
        if (!visible) {
            NetworkBridge.stopPolling();
            editingSsid = "";
        }
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

            // ── Header + radio toggle ────────────────────────────────
            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                Text {
                    text: qsTr("WI-FI")
                    color: Theme.accent
                    font.pixelSize: 11
                    font.weight: Font.Bold
                    font.letterSpacing: 2
                    Layout.fillWidth: true
                }

                // Radio toggle — matches the hotword / TTS switches
                // in SettingsPanel for visual continuity.
                Rectangle {
                    id: radioSwitch
                    implicitWidth: 44
                    implicitHeight: 24
                    radius: 12
                    color: NetworkBridge.wifiEnabled ? Theme.accent : Qt.rgba(1, 1, 1, 0.08)
                    border.color: NetworkBridge.wifiEnabled ? Theme.accent : Theme.border
                    border.width: 1
                    Behavior on color { ColorAnimation { duration: Theme.animFast } }

                    Rectangle {
                        width: 18; height: 18; radius: 9
                        color: Theme.text
                        anchors.verticalCenter: parent.verticalCenter
                        x: NetworkBridge.wifiEnabled ? parent.width - width - 3 : 3
                        Behavior on x { NumberAnimation { duration: Theme.animFast; easing.type: Easing.OutCubic } }
                    }

                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: NetworkBridge.setWifiEnabled(!NetworkBridge.wifiEnabled)
                    }
                }

                Item {
                    implicitWidth: scanLabel.implicitWidth + 16
                    implicitHeight: 22
                    visible: NetworkBridge.wifiEnabled

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
                        text: NetworkBridge.busy ? qsTr("…") : qsTr("BUSCAR")
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
                        enabled: !NetworkBridge.busy
                        onClicked: NetworkBridge.scan()
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
                visible: !NetworkBridge.wifiEnabled
                text: qsTr("Wi-Fi desativado.")
                color: Theme.textDim
                font.pixelSize: 13
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                Layout.topMargin: 24
            }

            // ── Network list ─────────────────────────────────────────
            ListView {
                visible: NetworkBridge.wifiEnabled
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 6
                model: NetworkBridge.availableNetworks

                delegate: Rectangle {
                    width: ListView.view.width
                    implicitHeight: rowCol.implicitHeight + 16
                    radius: 8
                    color: modelData.in_use
                        ? Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.12)
                        : (rowArea.containsMouse
                            ? Qt.rgba(1, 1, 1, 0.05)
                            : Qt.rgba(1, 1, 1, 0.02))
                    border.color: modelData.in_use
                        ? Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.45)
                        : Theme.border
                    border.width: 1
                    Behavior on color { ColorAnimation { duration: Theme.animFast } }

                    ColumnLayout {
                        id: rowCol
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: 8
                        spacing: 4

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 6

                            Text {
                                text: modelData.ssid
                                color: Theme.text
                                font.pixelSize: 13
                                font.weight: modelData.in_use ? Font.Bold : Font.Normal
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                            }
                            // Tiny lock glyph when the network is secured.
                            // nmcli reports "" for open networks; anything
                            // else (WPA1, WPA2, WPA3, 802.1X) means secured.
                            Text {
                                visible: modelData.security && modelData.security.length > 0
                                text: "🔒"
                                font.pixelSize: 10
                                color: Theme.textDim
                            }
                            Text {
                                text: modelData.signal + "%"
                                color: Theme.textDim
                                font.pixelSize: 11
                            }
                        }

                        Text {
                            visible: modelData.in_use
                            text: qsTr("Conectado")
                            color: Theme.success
                            font.pixelSize: 10
                            font.weight: Font.Bold
                            font.letterSpacing: 1
                        }

                        // Inline password field — appears when the user
                        // clicks a secured row that isn't already in use.
                        Rectangle {
                            visible: root.editingSsid === modelData.ssid
                            Layout.fillWidth: true
                            Layout.preferredHeight: 32
                            Layout.topMargin: 4
                            radius: 16
                            color: Qt.rgba(1, 1, 1, 0.05)
                            border.color: pwInput.activeFocus ? Theme.accent : Theme.border
                            border.width: 1

                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 12
                                anchors.rightMargin: 4
                                spacing: 4

                                TextInput {
                                    id: pwInput
                                    Layout.fillWidth: true
                                    verticalAlignment: TextInput.AlignVCenter
                                    color: Theme.text
                                    font.pixelSize: 13
                                    clip: true
                                    echoMode: TextInput.Password
                                    enabled: !NetworkBridge.busy
                                    onAccepted: {
                                        NetworkBridge.connectTo(modelData.ssid, text);
                                        root.editingSsid = "";
                                    }
                                    Component.onCompleted: forceActiveFocus()
                                }

                                Rectangle {
                                    Layout.preferredWidth: 60
                                    Layout.preferredHeight: 24
                                    radius: 12
                                    color: connectBtn.containsMouse
                                        ? Theme.accent
                                        : Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.35)
                                    Text {
                                        anchors.centerIn: parent
                                        text: qsTr("OK")
                                        color: Theme.text
                                        font.pixelSize: 10
                                        font.weight: Font.Bold
                                        font.letterSpacing: 1
                                    }
                                    MouseArea {
                                        id: connectBtn
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: {
                                            NetworkBridge.connectTo(modelData.ssid, pwInput.text);
                                            root.editingSsid = "";
                                        }
                                    }
                                }
                            }
                        }
                    }

                    MouseArea {
                        id: rowArea
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        enabled: !modelData.in_use
                        // Click logic: secured + not active → open the
                        // password row. Open network → connect directly.
                        onClicked: {
                            const secured = modelData.security && modelData.security.length > 0;
                            if (secured) {
                                root.editingSsid = modelData.ssid;
                            } else {
                                NetworkBridge.connectTo(modelData.ssid, "");
                            }
                        }
                    }
                }
            }

            // ── Error footer ─────────────────────────────────────────
            Text {
                visible: NetworkBridge.lastError.length > 0
                Layout.fillWidth: true
                text: NetworkBridge.lastError
                color: Theme.danger
                font.pixelSize: 11
                wrapMode: Text.WordWrap
                horizontalAlignment: Text.AlignHCenter
            }
        }
    }
}
