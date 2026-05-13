import QtQuick
import QtQuick.Layouts
import Jarvis.Greeter

/// 02 · LILITH — cinematic mode. The avatar image is deferred to V2
/// (asset pipeline + licensing TBD); V1 renders a tall glow column
/// where the character will live, plus the voice waveform + suggestion
/// chips that are the real interaction surface.
///
/// The auth path is still password through greetd — Lilith Mode is
/// the *aesthetic* of conversational login; real voice/face unlock
/// lives behind PAM hooks added in V2/V3.
Item {
    id: root
    property string username: GreeterState.username
    signal infoMessage(string text)

    GlassCard {
        anchors.centerIn: parent
        implicitWidth: 460
        implicitHeight: contentColumn.implicitHeight + 80

        ColumnLayout {
            id: contentColumn
            anchors.fill: parent
            anchors.leftMargin: 36
            anchors.rightMargin: 36
            anchors.topMargin: 32
            anchors.bottomMargin: 32
            spacing: 16

            Text {
                Layout.alignment: Qt.AlignHCenter
                text: qsTr("LILITH INTERFACE")
                color: Theme.accent
                font.pixelSize: 11
                font.weight: Font.Bold
                font.letterSpacing: 2
            }

            // Avatar slot. AnimeAvatar swaps between PNG sprites
            // when the qrc:/avatar/ assets are present, falls back to
            // the procedural glow column when they aren't (V1 reality
            // — real Lilith art is V2 work). The state drives both
            // the sprite choice and the procedural pulse rate, so the
            // composition reads as "alive" either way.
            AnimeAvatar {
                Layout.alignment: Qt.AlignHCenter
                Layout.preferredWidth: 140
                Layout.preferredHeight: 180
                state: pwField.text.length > 0 ? "listening" : "idle"
            }

            Text {
                Layout.alignment: Qt.AlignHCenter
                text: qsTr("Good evening, ") + root.username + "."
                color: Theme.text
                font.pixelSize: 18
                font.weight: Font.Bold
            }

            Text {
                Layout.alignment: Qt.AlignHCenter
                text: qsTr("How may I assist you today?")
                color: Theme.textDim
                font.pixelSize: 13
            }

            // The "type something" prompt — same control as the
            // standard mode's password field, just with a softer
            // placeholder so the LilithMode reads as conversational.
            PasswordField {
                id: pwField
                Layout.fillWidth: true
                placeholder: qsTr("Say something like…")
                onAccepted: root.submit()
            }

            // Decorative voice waveform.
            VoiceWaveform {
                Layout.alignment: Qt.AlignHCenter
            }

            // Suggestion chips.
            RowLayout {
                Layout.alignment: Qt.AlignHCenter
                spacing: 8

                SuggestionChip {
                    label: qsTr("Open Developer Workspace")
                    onPicked: root.infoMessage(qsTr("Workspaces — disponíveis em breve"))
                }
                SuggestionChip {
                    label: qsTr("System status")
                    onPicked: root.infoMessage(qsTr("System status — disponível em breve"))
                }
            }

        }
    }

    function submit() {
        if (GreetdClient.state === "awaiting_response") {
            GreetdClient.answerPrompt(pwField.text);
            pwField.text = "";
            return;
        }
        GreeterState.persist();
        GreetdClient.beginLogin(root.username);
    }
}
