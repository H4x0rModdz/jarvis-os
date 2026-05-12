import QtQuick
import QtQuick.Layouts
import Jarvis.Greeter

/// The login card. Holds the username + an input that becomes a
/// password field when greetd asks for a secret. State is driven
/// entirely by GreetdClient — the QML is a thin renderer over the
/// state machine.
Rectangle {
    id: root
    implicitWidth: 420
    implicitHeight: 320
    radius: 16
    color: Theme.surfaceBright
    border.color: Theme.border
    border.width: 1

    // Subtle inner highlight along the top edge.
    Rectangle {
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.margins: 1
        height: 1
        color: Qt.rgba(1, 1, 1, 0.06)
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 28
        spacing: 18

        Text {
            text: qsTr("JARVIS OS")
            color: Theme.accent
            font.pixelSize: 13
            font.weight: Font.Bold
            font.letterSpacing: 2
        }

        Text {
            text: qsTr("Entrar")
            color: Theme.text
            font.pixelSize: 26
            font.weight: Font.Bold
        }

        // Username field — pre-filled with the single V1 user.
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 6

            Text {
                text: qsTr("Usuário")
                color: Theme.textDim
                font.pixelSize: 11
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 40
                radius: 8
                color: Qt.rgba(1, 1, 1, 0.05)
                border.color: usernameField.activeFocus ? Theme.accent : Theme.border
                border.width: 1
                Behavior on border.color { ColorAnimation { duration: Theme.animFast } }

                TextInput {
                    id: usernameField
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    verticalAlignment: TextInput.AlignVCenter
                    color: Theme.text
                    selectionColor: Theme.accent
                    selectedTextColor: Theme.text
                    font.pixelSize: 16
                    clip: true
                    text: "jarvis"
                    onAccepted: secretField.forceActiveFocus()
                }
            }
        }

        // Secret / prompt field. Visible only after greetd asks.
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 6
            visible: GreetdClient.state === "awaiting_response"
                  || GreetdClient.state === "idle"
                  || GreetdClient.state === "checking"

            Text {
                text: GreetdClient.prompt.length > 0
                    ? GreetdClient.prompt
                    : qsTr("Senha")
                color: Theme.textDim
                font.pixelSize: 11
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 40
                radius: 8
                color: Qt.rgba(1, 1, 1, 0.05)
                border.color: secretField.activeFocus ? Theme.accent : Theme.border
                border.width: 1
                Behavior on border.color { ColorAnimation { duration: Theme.animFast } }

                TextInput {
                    id: secretField
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    verticalAlignment: TextInput.AlignVCenter
                    color: Theme.text
                    selectionColor: Theme.accent
                    selectedTextColor: Theme.text
                    font.pixelSize: 16
                    clip: true
                    echoMode: GreetdClient.secret ? TextInput.Password : TextInput.Normal
                    onAccepted: root.submit()

                    Component.onCompleted: forceActiveFocus()
                }
            }
        }

        // Error line — only shown when greetd surfaced one.
        Text {
            visible: GreetdClient.error.length > 0
            text: GreetdClient.error
            color: Theme.danger
            font.pixelSize: 12
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }

        Item { Layout.fillHeight: true }

        // Login button.
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 44
            radius: 8
            color: clickArea.containsMouse ? Theme.accent : Qt.darker(Theme.accent, 1.1)
            border.color: Theme.accent
            border.width: 1
            opacity: GreetdClient.state === "checking"
                  || GreetdClient.state === "starting_session"
                ? 0.55 : 1.0
            Behavior on color { ColorAnimation { duration: Theme.animFast } }
            Behavior on opacity { NumberAnimation { duration: Theme.animFast } }

            Text {
                anchors.centerIn: parent
                text: GreetdClient.state === "starting_session"
                    ? qsTr("Iniciando…")
                    : (GreetdClient.state === "checking"
                       ? qsTr("Verificando…")
                       : qsTr("Entrar"))
                color: Theme.text
                font.pixelSize: 15
                font.weight: Font.Bold
            }

            MouseArea {
                id: clickArea
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.submit()
            }
        }
    }

    function submit() {
        if (GreetdClient.state === "checking"
            || GreetdClient.state === "starting_session") {
            return; // ignore while in flight
        }
        if (GreetdClient.state === "awaiting_response") {
            GreetdClient.answerPrompt(secretField.text);
            secretField.text = "";
            return;
        }
        // idle / error — fire a fresh create_session.
        GreetdClient.beginLogin(usernameField.text);
        // Don't clear the secret field yet — greetd hasn't asked for
        // it. If a prompt comes back and the user types again, we'll
        // clear once they submit.
    }
}
