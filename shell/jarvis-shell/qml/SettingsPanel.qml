import QtQuick
import QtQuick.Layouts
import QtQuick.Window
import Jarvis.Shell

/// Preferences panel — a sibling Window that reads/writes through the
/// SettingsBridge. Three settings ship in the panel today; future
/// commits will grow the list as new pieces of the OS earn a
/// user-tunable surface.
///
/// Opens via `SettingsPanel.requestOpen()` from outside (gear button on
/// the bar). Esc closes.
Window {
    id: root
    visible: false
    width: 520
    height: 380
    title: qsTr("Preferências do Jarvis OS")
    color: "transparent"
    flags: Qt.Dialog | Qt.WindowStaysOnTopHint

    function requestOpen() {
        if (Qt.application.screens.length > 0) {
            const s = Qt.application.screens[0];
            x = s.virtualX + Math.floor((s.width - width) / 2);
            y = s.virtualY + Math.floor((s.height - height) / 2);
        }
        visible = true;
        requestActivate();
    }

    Shortcut {
        sequence: "Escape"
        onActivated: root.visible = false
    }

    // Bind to SettingsBridge.valueChanged so the panel re-resolves
    // current values when the daemon broadcasts. The number is a
    // version counter that bindings can sample to force a re-read.
    property int _settingsTick: 0
    Connections {
        target: SettingsBridge
        function onValueChanged(key) { root._settingsTick++; }
    }

    Rectangle {
        anchors.fill: parent
        anchors.margins: 8
        radius: Theme.radius
        color: Theme.surfaceBright
        border.color: Theme.border
        border.width: 1

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 24
            spacing: 16

            Text {
                text: qsTr("PREFERÊNCIAS")
                color: Theme.accent
                font.pixelSize: 11
                font.weight: Font.Bold
            }

            Text {
                text: qsTr("Jarvis OS")
                color: Theme.text
                font.pixelSize: 22
                font.weight: Font.Bold
                Layout.fillWidth: true
            }

            // ── Row: Lilith model ─────────────────────────────────────
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 4
                Text {
                    text: qsTr("Modelo da Lilith")
                    color: Theme.text
                    font.pixelSize: 14
                }
                Text {
                    text: qsTr("Tag do Ollama. Mudar exige reiniciar o jarvis-lilith.")
                    color: Theme.textDim
                    font.pixelSize: 11
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }
                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 36
                    radius: Theme.radius - 4
                    color: Qt.rgba(1, 1, 1, 0.05)
                    border.color: modelInput.activeFocus ? Theme.accent : Theme.border
                    border.width: 1
                    Behavior on border.color { ColorAnimation { duration: Theme.animFast } }
                    TextInput {
                        id: modelInput
                        anchors.fill: parent
                        anchors.leftMargin: 12
                        anchors.rightMargin: 12
                        verticalAlignment: TextInput.AlignVCenter
                        color: Theme.text
                        font.pixelSize: 14
                        clip: true
                        // The tick guard forces a re-read when SettingsBridge
                        // broadcasts a Changed for this key. Reads after a
                        // write also flow back through here.
                        text: (root._settingsTick, SettingsBridge.getString("lilith.model", "qwen3:4b"))
                        onEditingFinished: SettingsBridge.setString("lilith.model", text)
                    }
                }
            }

            // ── Row: TTS toggle ───────────────────────────────────────
            RowLayout {
                Layout.fillWidth: true
                spacing: 12
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 2
                    Text {
                        text: qsTr("Falar respostas da Lilith")
                        color: Theme.text
                        font.pixelSize: 14
                    }
                    Text {
                        text: qsTr("Quando ligado, a resposta também sai pelos alto-falantes via piper.")
                        color: Theme.textDim
                        font.pixelSize: 11
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                }

                // Custom Switch — a pill that slides a circle when toggled.
                Rectangle {
                    id: ttsSwitch
                    Layout.alignment: Qt.AlignVCenter
                    implicitWidth: 44
                    implicitHeight: 24
                    radius: 12
                    property bool checked: (root._settingsTick, SettingsBridge.getBool("voice.tts_enabled", true))
                    color: checked ? Theme.accent : Qt.rgba(1, 1, 1, 0.08)
                    border.color: checked ? Theme.accent : Theme.border
                    border.width: 1
                    Behavior on color { ColorAnimation { duration: Theme.animFast } }

                    Rectangle {
                        width: 18; height: 18; radius: 9
                        color: Theme.text
                        anchors.verticalCenter: parent.verticalCenter
                        x: parent.checked ? parent.width - width - 3 : 3
                        Behavior on x { NumberAnimation { duration: Theme.animFast; easing.type: Easing.OutCubic } }
                    }

                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: SettingsBridge.setBool("voice.tts_enabled", !ttsSwitch.checked)
                    }
                }
            }

            // ── Row: STT language ─────────────────────────────────────
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 4
                Text {
                    text: qsTr("Idioma do reconhecimento de voz")
                    color: Theme.text
                    font.pixelSize: 14
                }
                Text {
                    text: qsTr("Código ISO (pt, en, …) ou \"auto\" para detecção automática.")
                    color: Theme.textDim
                    font.pixelSize: 11
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }
                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 36
                    radius: Theme.radius - 4
                    color: Qt.rgba(1, 1, 1, 0.05)
                    border.color: langInput.activeFocus ? Theme.accent : Theme.border
                    border.width: 1
                    Behavior on border.color { ColorAnimation { duration: Theme.animFast } }
                    TextInput {
                        id: langInput
                        anchors.fill: parent
                        anchors.leftMargin: 12
                        anchors.rightMargin: 12
                        verticalAlignment: TextInput.AlignVCenter
                        color: Theme.text
                        font.pixelSize: 14
                        clip: true
                        text: (root._settingsTick, SettingsBridge.getString("voice.language", "auto"))
                        onEditingFinished: SettingsBridge.setString("voice.language", text)
                    }
                }
            }

            Item { Layout.fillHeight: true }

            // ── Footer ────────────────────────────────────────────────
            Text {
                Layout.alignment: Qt.AlignRight
                text: SettingsBridge.reachable
                    ? qsTr("Conectado a com.jarvis.Settings")
                    : qsTr("Settings daemon offline — alterações não serão salvas")
                color: SettingsBridge.reachable ? Theme.success : Theme.danger
                font.pixelSize: 11
            }
        }
    }
}
