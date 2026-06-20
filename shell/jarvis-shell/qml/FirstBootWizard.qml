import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import Jarvis.Shell

/// First-boot wizard. Opens once after install — never again unless
/// the QSettings flag `first_boot.completed` is reset. Four pages:
///   1. Welcome
///   2. Wi-Fi (pick a network so the rest of the OS can phone home)
///   3. Voice enrollment teaser (skippable)
///   4. Tour
///
/// Each page has its own "skip / next" semantics. The wizard never
/// blocks — every step is skippable. The final page replaces "Next"
/// with "Concluir" which sets the flag and hides the window.
Window {
    id: root
    visible: false
    width: 720
    height: 540
    color: "transparent"
    flags: Qt.Dialog | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
    title: qsTr("Bem-vindo ao LilithOS")

    /// The QSettings group name + key used to gate the wizard.
    readonly property string completedKey: "first_boot.completed"

    function maybeOpen() {
        if (SettingsBridge.getBool(completedKey, false)) return;
        if (Qt.application.screens.length > 0) {
            const s = Qt.application.screens[0];
            x = s.virtualX + Math.floor((s.width - width) / 2);
            y = s.virtualY + Math.floor((s.height - height) / 2);
        }
        visible = true;
        requestActivate();
        // Start Wi-Fi polling early so the network step renders
        // populated when the user gets there.
        NetworkBridge.startPolling();
    }

    function complete() {
        SettingsBridge.setBool(completedKey, true);
        NetworkBridge.stopPolling();
        visible = false;
    }

    GlassPanel {
        anchors.fill: parent
        anchors.margins: 8

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 32
            spacing: 16

            // ── Step indicator ─────────────────────────────────────
            RowLayout {
                Layout.alignment: Qt.AlignHCenter
                spacing: 8

                Repeater {
                    model: 4
                    delegate: Rectangle {
                        implicitWidth: 32
                        implicitHeight: 4
                        radius: 2
                        color: index <= swipe.currentIndex
                            ? Theme.accent
                            : Qt.rgba(1, 1, 1, 0.15)
                        Behavior on color { ColorAnimation { duration: Theme.animFast } }
                    }
                }
            }

            // ── Pages ──────────────────────────────────────────────
            SwipeView {
                id: swipe
                Layout.fillWidth: true
                Layout.fillHeight: true
                interactive: false  // navigation is button-driven
                clip: true

                // Page 1 — Welcome.
                ColumnLayout {
                    spacing: 12

                    Item { Layout.fillHeight: true }

                    // Branding mark: large accent ring with the
                    // letter "L" (Lilith) inside, drawn from primitives so
                    // we don't depend on a baked PNG resource here.
                    Rectangle {
                        Layout.alignment: Qt.AlignHCenter
                        implicitWidth: 120
                        implicitHeight: 120
                        radius: 60
                        color: "transparent"
                        border.color: Theme.accent
                        border.width: 3
                        Text {
                            anchors.centerIn: parent
                            text: "L"
                            color: Theme.text
                            font.pixelSize: 56
                            font.weight: Font.Bold
                            font.italic: true
                        }
                    }

                    Text {
                        Layout.alignment: Qt.AlignHCenter
                        text: qsTr("Bem-vindo ao LilithOS")
                        color: Theme.text
                        font.pixelSize: 26
                        font.weight: Font.Bold
                    }

                    Text {
                        Layout.alignment: Qt.AlignHCenter
                        Layout.preferredWidth: 480
                        text: qsTr("Um desktop AI-nativo. A Lilith vive na barra embaixo da tela e \
                                    executa qualquer ação do sistema. Quatro passos rápidos pra \
                                    deixar tudo pronto.")
                        color: Theme.textDim
                        font.pixelSize: 14
                        wrapMode: Text.WordWrap
                        horizontalAlignment: Text.AlignHCenter
                    }

                    Item { Layout.fillHeight: true }
                }

                // Page 2 — Wi-Fi.
                ColumnLayout {
                    id: wifiPage
                    spacing: 12
                    /// SSID currently being password-edited; "" when no
                    /// row is in the edit state.
                    property string editingSsid: ""

                    Text {
                        text: qsTr("Conecte ao Wi-Fi")
                        color: Theme.text
                        font.pixelSize: 22
                        font.weight: Font.Bold
                    }
                    Text {
                        text: qsTr("Escolha uma rede agora ou pule e configure depois pela barra.")
                        color: Theme.textDim
                        font.pixelSize: 12
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8

                        // Inline radio toggle so the user can see the
                        // state even if the dnf bring-up hasn't enabled
                        // the radio yet.
                        Rectangle {
                            implicitWidth: 44
                            implicitHeight: 24
                            radius: 12
                            color: NetworkBridge.wifiEnabled ? Theme.accent : Qt.rgba(1, 1, 1, 0.08)
                            border.color: NetworkBridge.wifiEnabled ? Theme.accent : Theme.border
                            border.width: 1
                            Rectangle {
                                width: 18; height: 18; radius: 9
                                color: Theme.text
                                anchors.verticalCenter: parent.verticalCenter
                                x: NetworkBridge.wifiEnabled ? parent.width - width - 3 : 3
                                Behavior on x { NumberAnimation { duration: Theme.animFast } }
                            }
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked: NetworkBridge.setWifiEnabled(!NetworkBridge.wifiEnabled)
                            }
                        }
                        Text {
                            text: qsTr("Wi-Fi")
                            color: Theme.text
                            font.pixelSize: 13
                            Layout.fillWidth: true
                        }
                        Rectangle {
                            implicitWidth: 60
                            implicitHeight: 22
                            radius: 11
                            color: Qt.rgba(1, 1, 1, 0.04)
                            border.color: Theme.border
                            border.width: 1
                            Text {
                                anchors.centerIn: parent
                                text: NetworkBridge.busy ? qsTr("…") : qsTr("BUSCAR")
                                color: Theme.textDim
                                font.pixelSize: 9
                                font.weight: Font.Bold
                                font.letterSpacing: 1
                            }
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked: NetworkBridge.scan()
                            }
                        }
                    }

                    ListView {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        clip: true
                        spacing: 6
                        visible: NetworkBridge.wifiEnabled
                        model: NetworkBridge.availableNetworks

                        delegate: Rectangle {
                            width: ListView.view.width
                            implicitHeight: rowCol.implicitHeight + 16
                            radius: 8
                            color: modelData.in_use
                                ? Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.18)
                                : Qt.rgba(1, 1, 1, 0.04)
                            border.color: modelData.in_use ? Theme.accent : Theme.border
                            border.width: 1

                            ColumnLayout {
                                id: rowCol
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.top: parent.top
                                anchors.margins: 8
                                spacing: 4

                                RowLayout {
                                    Layout.fillWidth: true
                                    Text {
                                        text: modelData.ssid
                                        color: Theme.text
                                        font.pixelSize: 13
                                        font.weight: modelData.in_use ? Font.Bold : Font.Normal
                                        Layout.fillWidth: true
                                        elide: Text.ElideRight
                                    }
                                    Text {
                                        visible: modelData.security && modelData.security.length > 0
                                        text: "🔒"
                                        font.pixelSize: 10
                                    }
                                    Text {
                                        text: modelData.signal + "%"
                                        color: Theme.textDim
                                        font.pixelSize: 11
                                    }
                                }

                                Rectangle {
                                    visible: wifiPage.editingSsid === modelData.ssid
                                    Layout.fillWidth: true
                                    Layout.preferredHeight: 32
                                    radius: 16
                                    color: Qt.rgba(1, 1, 1, 0.06)
                                    border.color: pwIn.activeFocus ? Theme.accent : Theme.border
                                    border.width: 1
                                    TextInput {
                                        id: pwIn
                                        anchors.fill: parent
                                        anchors.leftMargin: 12
                                        anchors.rightMargin: 12
                                        verticalAlignment: TextInput.AlignVCenter
                                        color: Theme.text
                                        font.pixelSize: 13
                                        echoMode: TextInput.Password
                                        clip: true
                                        onAccepted: {
                                            NetworkBridge.connectTo(modelData.ssid, text);
                                            text = "";
                                            wifiPage.editingSsid = "";
                                        }
                                        Component.onCompleted: forceActiveFocus()
                                    }
                                }
                            }

                            MouseArea {
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                enabled: !modelData.in_use
                                onClicked: {
                                    const secured = modelData.security && modelData.security.length > 0;
                                    if (secured) {
                                        wifiPage.editingSsid = modelData.ssid;
                                    } else {
                                        NetworkBridge.connectTo(modelData.ssid, "");
                                    }
                                }
                            }
                        }
                    }

                    Text {
                        visible: !NetworkBridge.wifiEnabled
                        text: qsTr("Wi-Fi desativado. Use o toggle acima ou pule este passo.")
                        color: Theme.textDim
                        font.pixelSize: 12
                        Layout.fillWidth: true
                        Layout.topMargin: 24
                        horizontalAlignment: Text.AlignHCenter
                    }
                }

                // Page 3 — Voice enrollment.
                ColumnLayout {
                    spacing: 16

                    Item { Layout.fillHeight: true }

                    Text {
                        Layout.alignment: Qt.AlignHCenter
                        text: qsTr("Registre sua voz")
                        color: Theme.text
                        font.pixelSize: 22
                        font.weight: Font.Bold
                    }
                    Text {
                        Layout.alignment: Qt.AlignHCenter
                        Layout.preferredWidth: 520
                        text: qsTr("Você pode desbloquear a tela falando em vez de digitar a senha. \
                                    Vou capturar 3 segundos da sua voz para criar uma impressão \
                                    digital local. Áudio nunca sai do dispositivo. Pode pular e \
                                    fazer depois em Preferências.")
                        color: Theme.textDim
                        font.pixelSize: 13
                        wrapMode: Text.WordWrap
                        horizontalAlignment: Text.AlignHCenter
                    }

                    RowLayout {
                        Layout.alignment: Qt.AlignHCenter
                        spacing: 12

                        Rectangle {
                            implicitWidth: 180
                            implicitHeight: 36
                            radius: 18
                            color: enrollArea.containsMouse ? Theme.accent : Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.30)
                            border.color: Theme.accent
                            border.width: 1
                            Text {
                                anchors.centerIn: parent
                                text: qsTr("REGISTRAR (3s)")
                                color: Theme.text
                                font.pixelSize: 11
                                font.weight: Font.Bold
                                font.letterSpacing: 1
                            }
                            MouseArea {
                                id: enrollArea
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: VoiceBridge.enrollVoiceprint(VoiceBridge.currentUser, 3)
                            }
                        }
                    }

                    Text {
                        visible: VoiceBridge.lastEnrollMessage.length > 0
                        Layout.alignment: Qt.AlignHCenter
                        Layout.preferredWidth: 420
                        text: VoiceBridge.lastEnrollMessage
                        color: Theme.textDim
                        font.pixelSize: 12
                        wrapMode: Text.WordWrap
                        horizontalAlignment: Text.AlignHCenter
                    }

                    Item { Layout.fillHeight: true }
                }

                // Page 4 — Tour.
                ColumnLayout {
                    spacing: 16

                    Item { Layout.fillHeight: true }

                    Text {
                        Layout.alignment: Qt.AlignHCenter
                        text: qsTr("Você está pronto")
                        color: Theme.text
                        font.pixelSize: 22
                        font.weight: Font.Bold
                    }
                    Text {
                        Layout.alignment: Qt.AlignHCenter
                        Layout.preferredWidth: 540
                        text: qsTr("Clique no campo da barra e diga \"oi lilith\" — ou apenas \
                                    digite o que precisa. \"abrir o navegador\", \"tirar um \
                                    print\", \"instala o gimp\". Use Super+L pra bloquear e \
                                    Super pra falar com a Lilith.")
                        color: Theme.textDim
                        font.pixelSize: 14
                        wrapMode: Text.WordWrap
                        horizontalAlignment: Text.AlignHCenter
                    }

                    Item { Layout.fillHeight: true }
                }
            }

            // ── Navigation row ─────────────────────────────────────
            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                Rectangle {
                    implicitWidth: 100
                    implicitHeight: 36
                    radius: 18
                    visible: swipe.currentIndex > 0
                    color: backArea.containsMouse
                        ? Qt.rgba(1, 1, 1, 0.10)
                        : Qt.rgba(1, 1, 1, 0.04)
                    border.color: Theme.border
                    border.width: 1
                    Text {
                        anchors.centerIn: parent
                        text: qsTr("VOLTAR")
                        color: Theme.textDim
                        font.pixelSize: 11
                        font.weight: Font.Bold
                        font.letterSpacing: 1
                    }
                    MouseArea {
                        id: backArea
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: swipe.currentIndex--
                    }
                }

                Item { Layout.fillWidth: true }

                Rectangle {
                    implicitWidth: 100
                    implicitHeight: 36
                    radius: 18
                    visible: swipe.currentIndex < swipe.count - 1
                    color: Qt.rgba(1, 1, 1, 0.04)
                    border.color: Theme.border
                    border.width: 1
                    Text {
                        anchors.centerIn: parent
                        text: qsTr("PULAR")
                        color: Theme.textDim
                        font.pixelSize: 11
                        font.weight: Font.Bold
                        font.letterSpacing: 1
                    }
                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: swipe.currentIndex = swipe.count - 1
                    }
                }

                Rectangle {
                    implicitWidth: 120
                    implicitHeight: 36
                    radius: 18
                    color: nextArea.containsMouse
                        ? Theme.accent
                        : Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.45)
                    border.color: Theme.accent
                    border.width: 1
                    Text {
                        anchors.centerIn: parent
                        text: swipe.currentIndex === swipe.count - 1
                            ? qsTr("CONCLUIR")
                            : qsTr("AVANÇAR")
                        color: Theme.text
                        font.pixelSize: 11
                        font.weight: Font.Bold
                        font.letterSpacing: 1
                    }
                    MouseArea {
                        id: nextArea
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            if (swipe.currentIndex === swipe.count - 1) {
                                root.complete();
                            } else {
                                swipe.currentIndex++;
                            }
                        }
                    }
                }
            }
        }
    }
}
