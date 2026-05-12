import QtQuick
import QtQuick.Layouts
import Jarvis.Greeter

/// 01 · STANDARD — the default mode. Glass card with the ring logo,
/// a welcome line, password entry, and the row of alt-auth pills
/// (visually present in V1, functional only for password).
Item {
    id: root
    property string username: "lucas"
    signal infoMessage(string text)

    GlassCard {
        anchors.centerIn: parent
        implicitWidth: 380
        implicitHeight: contentColumn.implicitHeight + 60

        ColumnLayout {
            id: contentColumn
            anchors.fill: parent
            anchors.leftMargin: 32
            anchors.rightMargin: 32
            anchors.topMargin: 28
            anchors.bottomMargin: 28
            spacing: 14

            // Header chip
            Text {
                Layout.alignment: Qt.AlignHCenter
                text: qsTr("STANDARD LOGIN")
                color: Theme.accent
                font.pixelSize: 11
                font.weight: Font.Bold
                font.letterSpacing: 2
            }

            JarvisLogo {
                Layout.alignment: Qt.AlignHCenter
                size: 96
            }

            Text {
                Layout.alignment: Qt.AlignHCenter
                text: qsTr("Welcome back, ") + root.username + "."
                color: Theme.text
                font.pixelSize: 16
            }

            PasswordField {
                id: pwField
                Layout.fillWidth: true
                onAccepted: root.submit()
            }

            UnlockButton {
                Layout.fillWidth: true
                label: qsTr("UNLOCK SYSTEM")
                busy: GreetdClient.state === "checking"
                   || GreetdClient.state === "starting_session"
                onClicked: root.submit()
            }

            // Alt-auth row
            RowLayout {
                Layout.alignment: Qt.AlignHCenter
                spacing: 12

                AuthIcon {
                    label: qsTr("FACE ID")
                    glyph: "◎"
                    onPicked: root.infoMessage(qsTr("Face ID — disponível em breve"))
                }
                AuthIcon {
                    label: qsTr("VOICE ID")
                    glyph: "♪"
                    onPicked: root.infoMessage(qsTr("Voice ID — disponível em breve"))
                }
                AuthIcon {
                    label: qsTr("PIN")
                    glyph: "▦"
                    onPicked: root.infoMessage(qsTr("PIN — disponível em breve"))
                }
            }

            // Error line.
            Text {
                visible: GreetdClient.error.length > 0
                Layout.fillWidth: true
                text: GreetdClient.error
                color: Theme.danger
                font.pixelSize: 12
                wrapMode: Text.WordWrap
                horizontalAlignment: Text.AlignHCenter
            }
        }
    }

    function submit() {
        if (GreetdClient.state === "awaiting_response") {
            GreetdClient.answerPrompt(pwField.text);
            pwField.text = "";
            return;
        }
        GreetdClient.beginLogin(root.username);
    }
}
