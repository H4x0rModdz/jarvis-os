import QtQuick
import QtQuick.Controls
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
    title: qsTr("Preferências do LilithOS")
    color: "transparent"
    flags: Qt.Dialog | Qt.WindowStaysOnTopHint

    function requestOpen() {
        if (Qt.application.screens.length > 0) {
            const s = Qt.application.screens[0];
            x = s.virtualX + Math.floor((s.width - width) / 2);
            y = s.virtualY + Math.floor((s.height - height) / 2);
        }
        // Pull a fresh enrolled-users list — daemon state may have
        // changed since the panel was last opened (CLI enroll, etc.).
        VoiceBridge.refreshEnrolledUsers();
        visible = true;
        requestActivate();
    }

    /// Signal up to Main.qml, which owns the DisplayPanel instance.
    /// Kept as a signal rather than reaching across the QML tree
    /// because SettingsPanel is itself a Window — direct lookups
    /// would couple unrelated trees.
    signal displayRequested()

    function openDisplay() { displayRequested(); }

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

    GlassPanel {
        anchors.fill: parent
        anchors.margins: 8

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 24
            spacing: 16

            RowLayout {
                Layout.fillWidth: true
                Text {
                    text: qsTr("PREFERÊNCIAS")
                    color: Theme.accent
                    font.pixelSize: 11
                    font.weight: Font.Bold
                    Layout.fillWidth: true
                }
                // Close — explicit affordance (Esc + click-outside
                // aren't reliable in a VM where focus can lag).
                Text {
                    text: "×"
                    color: closeArea.containsMouse ? Theme.danger : Theme.textDim
                    font.pixelSize: 20
                    font.weight: Font.Bold
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

            Text {
                text: qsTr("LilithOS")
                color: Theme.text
                font.pixelSize: 22
                font.weight: Font.Bold
                Layout.fillWidth: true
            }

            // The header (title + × close) above stays fixed; only the settings
            // below scroll, so the × is always reachable no matter how far down
            // you've scrolled.
            Flickable {
                Layout.fillWidth: true
                Layout.fillHeight: true
                contentWidth: width
                contentHeight: col.implicitHeight
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

                ColumnLayout {
                    id: col
                    width: parent.width
                    spacing: 16

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

            // ── Row: Lilith proativa ──────────────────────────────────
            RowLayout {
                Layout.fillWidth: true
                spacing: 12
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 2
                    Text {
                        text: qsTr("Lilith proativa")
                        color: Theme.text
                        font.pixelSize: 14
                    }
                    Text {
                        text: qsTr("Permite que a Lilith fale sem ser perguntada quando algo merece atenção (bateria crítica, etc.). Desligado, ela só responde a comandos.")
                        color: Theme.textDim
                        font.pixelSize: 11
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                }

                Rectangle {
                    id: proactiveSwitch
                    Layout.alignment: Qt.AlignVCenter
                    implicitWidth: 44
                    implicitHeight: 24
                    radius: 12
                    property bool checked: (root._settingsTick, SettingsBridge.getBool("lilith.proactive_enabled", true))
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
                        onClicked: {
                            const next = !proactiveSwitch.checked;
                            // Daemon re-reads this key on every tick
                            // (every 30 s) — no DBus method to call.
                            SettingsBridge.setBool("lilith.proactive_enabled", next);
                        }
                    }
                }
            }

            // ── Sub-row: Lilith fala em voz alta ──────────────────────
            // Indented + visually slaved to the proativa toggle: when
            // proativa is off, this row dims out (no proactive →
            // no speech to gate).
            RowLayout {
                Layout.fillWidth: true
                Layout.leftMargin: 24
                spacing: 12
                opacity: proactiveSwitch.checked ? 1.0 : 0.45
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 2
                    Text {
                        text: qsTr("Falar em voz alta")
                        color: Theme.text
                        font.pixelSize: 13
                    }
                    Text {
                        text: qsTr("Avisos críticos (bateria crítica, etc.) tocam pela TTS além do banner. Avisos comuns ficam silenciosos.")
                        color: Theme.textDim
                        font.pixelSize: 11
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                }

                Rectangle {
                    id: proactiveSpeaksSwitch
                    Layout.alignment: Qt.AlignVCenter
                    implicitWidth: 40
                    implicitHeight: 22
                    radius: 11
                    enabled: proactiveSwitch.checked
                    property bool checked: (root._settingsTick, SettingsBridge.getBool("lilith.proactive_speaks", true))
                    color: checked ? Theme.accent : Qt.rgba(1, 1, 1, 0.08)
                    border.color: checked ? Theme.accent : Theme.border
                    border.width: 1
                    Behavior on color { ColorAnimation { duration: Theme.animFast } }

                    Rectangle {
                        width: 16; height: 16; radius: 8
                        color: Theme.text
                        anchors.verticalCenter: parent.verticalCenter
                        x: parent.checked ? parent.width - width - 3 : 3
                        Behavior on x { NumberAnimation { duration: Theme.animFast; easing.type: Easing.OutCubic } }
                    }

                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        enabled: proactiveSwitch.checked
                        onClicked: {
                            const next = !proactiveSpeaksSwitch.checked;
                            SettingsBridge.setBool("lilith.proactive_speaks", next);
                        }
                    }
                }
            }

            // ── Row: Hotword toggle ──────────────────────────────────
            RowLayout {
                Layout.fillWidth: true
                spacing: 12
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 2
                    Text {
                        text: qsTr("Hotword \"oi lilith\"")
                        color: Theme.text
                        font.pixelSize: 14
                    }
                    Text {
                        text: qsTr("Escuta contínua para a frase de ativação. Usa ~15%% de um núcleo enquanto ligado. Áudio nunca sai do dispositivo.")
                        color: Theme.textDim
                        font.pixelSize: 11
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                }

                Rectangle {
                    id: hotwordSwitch
                    Layout.alignment: Qt.AlignVCenter
                    implicitWidth: 44
                    implicitHeight: 24
                    radius: 12
                    // Tick guard for re-reads after the bridge writes back.
                    property bool checked: (root._settingsTick, SettingsBridge.getBool("voice.hotword.enabled", false))
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
                        onClicked: {
                            const next = !hotwordSwitch.checked;
                            // Persist the preference and ask the daemon to
                            // start/stop the actor right away. The daemon
                            // also reads this setting at boot to restore
                            // state across sessions.
                            SettingsBridge.setBool("voice.hotword.enabled", next);
                            VoiceBridge.setHotwordEnabled(next);
                        }
                    }
                }
            }

            // ── Row: Voiceprint biometric ────────────────────────────
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 6

                Text {
                    text: qsTr("Reconhecimento de voz biométrico")
                    color: Theme.text
                    font.pixelSize: 14
                }
                Text {
                    text: qsTr("Registre sua voz para desbloquear o sistema sem digitar a senha. MFCC + DTW; pode ser burlado por gravação. Sempre há fallback de senha.")
                    color: Theme.textDim
                    font.pixelSize: 11
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }

                // Enrolled list — one row per user.
                Repeater {
                    model: VoiceBridge.enrolledUsers
                    delegate: RowLayout {
                        Layout.fillWidth: true
                        spacing: 8

                        Rectangle {
                            implicitWidth: 8
                            implicitHeight: 8
                            radius: 4
                            color: Theme.success
                        }
                        Text {
                            text: modelData.user
                            color: Theme.text
                            font.pixelSize: 13
                            Layout.fillWidth: true
                        }
                        Text {
                            text: qsTr("desde ") +
                                  (modelData.enrolled_at || "").substring(0, 10)
                            color: Theme.textDim
                            font.pixelSize: 11
                        }
                        Text {
                            text: "×"
                            color: deleteArea.containsMouse ? Theme.danger : Theme.textDim
                            font.pixelSize: 16
                            font.weight: Font.Bold
                            Layout.preferredWidth: 20
                            horizontalAlignment: Text.AlignHCenter

                            MouseArea {
                                id: deleteArea
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: VoiceBridge.deleteVoiceprint(modelData.user)
                            }
                        }
                    }
                }

                Text {
                    visible: VoiceBridge.enrolledUsers.length === 0
                    text: qsTr("Nenhuma voz registrada.")
                    color: Theme.textDim
                    font.pixelSize: 12
                    font.italic: true
                }

                // Action row — enroll + verify the current user.
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 32
                        radius: 16
                        color: enrollArea.containsMouse
                            ? Theme.accent
                            : Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.25)
                        border.color: Theme.accent
                        border.width: 1
                        Behavior on color { ColorAnimation { duration: Theme.animFast } }

                        Text {
                            anchors.centerIn: parent
                            text: qsTr("REGISTRAR MINHA VOZ (3s)")
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
                            // currentUser = $USER from the bridge —
                            // same identity pam-jarvis will see during
                            // a verify call on the lock screen.
                            onClicked: VoiceBridge.enrollVoiceprint(
                                VoiceBridge.currentUser, 3)
                        }
                    }
                    Rectangle {
                        Layout.preferredWidth: 100
                        Layout.preferredHeight: 32
                        radius: 16
                        color: verifyArea.containsMouse
                            ? Qt.rgba(1, 1, 1, 0.10)
                            : Qt.rgba(1, 1, 1, 0.05)
                        border.color: Theme.border
                        border.width: 1
                        Behavior on color { ColorAnimation { duration: Theme.animFast } }

                        Text {
                            anchors.centerIn: parent
                            text: qsTr("TESTAR")
                            color: Theme.textDim
                            font.pixelSize: 10
                            font.weight: Font.Bold
                            font.letterSpacing: 1
                        }
                        MouseArea {
                            id: verifyArea
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                // Verify the first enrolled user; in
                                // V1 the user is always the current
                                // logged-in operator anyway.
                                if (VoiceBridge.enrolledUsers.length > 0) {
                                    VoiceBridge.verifyVoiceprint(
                                        VoiceBridge.enrolledUsers[0].user);
                                }
                            }
                        }
                    }
                }

                // Inline feedback line — last enroll/verify result.
                Text {
                    visible: VoiceBridge.lastEnrollMessage.length > 0
                    text: VoiceBridge.lastEnrollMessage
                    color: Theme.textDim
                    font.pixelSize: 12
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }
            }

            // ── Row: Idle auto-lock timeout ──────────────────────────
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 4
                Text {
                    text: qsTr("Bloqueio automático por inatividade")
                    color: Theme.text
                    font.pixelSize: 14
                }
                Text {
                    text: qsTr("Segundos sem atividade antes do bloqueio. 0 desativa.")
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
                    border.color: idleInput.activeFocus ? Theme.accent : Theme.border
                    border.width: 1
                    Behavior on border.color { ColorAnimation { duration: Theme.animFast } }
                    TextInput {
                        id: idleInput
                        anchors.fill: parent
                        anchors.leftMargin: 12
                        anchors.rightMargin: 12
                        verticalAlignment: TextInput.AlignVCenter
                        color: Theme.text
                        font.pixelSize: 14
                        clip: true
                        // Treat invalid input as the default (300s) on commit.
                        text: (root._settingsTick, String(Math.round(SettingsBridge.getNumber("lock.idle_timeout_seconds", 300))))
                        validator: IntValidator { bottom: 0; top: 3600 }
                        onEditingFinished: {
                            const v = parseInt(text);
                            const clamped = isNaN(v) ? 300 : Math.max(0, Math.min(3600, v));
                            SettingsBridge.setNumber("lock.idle_timeout_seconds", clamped);
                        }
                    }
                }
            }

            // ── Row: Audio output ─────────────────────────────────────
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 4
                visible: AudioBridge.sinks.length > 0

                Text {
                    text: qsTr("Saída de áudio")
                    color: Theme.text
                    font.pixelSize: 14
                }
                Text {
                    text: qsTr("Onde o som sai. Trocar move todos os streams ativos pra nova saída.")
                    color: Theme.textDim
                    font.pixelSize: 11
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }

                Repeater {
                    model: AudioBridge.sinks
                    delegate: Rectangle {
                        Layout.fillWidth: true
                        implicitHeight: 32
                        radius: 8
                        color: modelData.isDefault
                            ? Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.18)
                            : (sinkArea.containsMouse
                                ? Qt.rgba(1, 1, 1, 0.05)
                                : Qt.rgba(1, 1, 1, 0.02))
                        border.color: modelData.isDefault ? Theme.accent : Theme.border
                        border.width: 1

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 10
                            anchors.rightMargin: 10
                            spacing: 8

                            // Bullet — filled when default.
                            Rectangle {
                                width: 8
                                height: 8
                                radius: 4
                                color: modelData.isDefault ? Theme.accent : "transparent"
                                border.color: modelData.isDefault ? Theme.accent : Theme.textDim
                                border.width: 1
                            }
                            Text {
                                text: modelData.description || modelData.name
                                color: Theme.text
                                font.pixelSize: 12
                                font.weight: modelData.isDefault ? Font.Bold : Font.Normal
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                            }
                            Text {
                                text: modelData.volume + "%"
                                color: Theme.textDim
                                font.pixelSize: 10
                            }
                        }

                        MouseArea {
                            id: sinkArea
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            enabled: !modelData.isDefault
                            onClicked: AudioBridge.setDefaultSink(modelData.name)
                        }
                    }
                }
            }

            // ── Row: Display config opener ────────────────────────────
            RowLayout {
                Layout.fillWidth: true
                spacing: 12
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 2
                    Text {
                        text: qsTr("Monitores")
                        color: Theme.text
                        font.pixelSize: 14
                    }
                    Text {
                        text: qsTr("Resolução, escala, ligar/desligar saídas externas.")
                        color: Theme.textDim
                        font.pixelSize: 11
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                }
                Rectangle {
                    Layout.alignment: Qt.AlignVCenter
                    implicitWidth: 92
                    implicitHeight: 28
                    radius: 14
                    color: dispArea.containsMouse
                        ? Qt.rgba(1, 1, 1, 0.10)
                        : Qt.rgba(1, 1, 1, 0.04)
                    border.color: Theme.border
                    border.width: 1
                    Text {
                        anchors.centerIn: parent
                        text: qsTr("CONFIGURAR")
                        color: Theme.text
                        font.pixelSize: 9
                        font.weight: Font.Bold
                        font.letterSpacing: 1
                    }
                    MouseArea {
                        id: dispArea
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.openDisplay()
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

            // ── Row: STT model (Whisper) ──────────────────────────────
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 4
                Text {
                    text: qsTr("Modelo de reconhecimento (Whisper)")
                    color: Theme.text
                    font.pixelSize: 14
                }
                Text {
                    text: qsTr("Maior = mais preciso, porém mais lento. Ao escolher, o " +
                               "modelo é baixado se ainda não estiver no sistema e passa " +
                               "a valer na próxima fala (sem reiniciar).")
                    color: Theme.textDim
                    font.pixelSize: 11
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    Repeater {
                        model: ["base", "small", "medium", "large-v3"]
                        delegate: Rectangle {
                            property bool selected: (root._settingsTick,
                                SettingsBridge.getString("voice.model", "small")) === modelData
                            Layout.fillWidth: true
                            implicitHeight: 34
                            radius: 8
                            color: selected
                                ? Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.18)
                                : (modelArea.containsMouse
                                    ? Qt.rgba(1, 1, 1, 0.05)
                                    : Qt.rgba(1, 1, 1, 0.02))
                            border.color: selected ? Theme.accent : Theme.border
                            border.width: 1
                            Text {
                                anchors.centerIn: parent
                                text: modelData
                                color: Theme.text
                                font.pixelSize: 12
                                font.weight: selected ? Font.Bold : Font.Normal
                            }
                            MouseArea {
                                id: modelArea
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: {
                                    // Save the choice (stt reads it live) then
                                    // pull the model if it isn't on disk yet.
                                    SettingsBridge.setString("voice.model", modelData);
                                    VoiceBridge.ensureModel(modelData);
                                }
                            }
                        }
                    }
                }

                // Real download bar while a model is being fetched.
                Item {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 6
                    visible: VoiceBridge.modelPercent >= 0
                    Rectangle {
                        anchors.fill: parent
                        radius: 3
                        color: Theme.border
                    }
                    Rectangle {
                        radius: 3
                        height: parent.height
                        color: Theme.accent
                        width: parent.width
                             * Math.max(0, Math.min(100, VoiceBridge.modelPercent)) / 100
                        Behavior on width {
                            NumberAnimation { duration: Theme.animFast; easing.type: Easing.OutCubic }
                        }
                    }
                }

                Text {
                    visible: VoiceBridge.modelStatus.length > 0
                    Layout.fillWidth: true
                    text: VoiceBridge.modelStatus
                    color: VoiceBridge.modelStatus.startsWith("erro") ? Theme.danger : Theme.textDim
                    font.pixelSize: 11
                    wrapMode: Text.WordWrap
                }
            }

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
    }
}
