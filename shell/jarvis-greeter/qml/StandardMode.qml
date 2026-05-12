import QtQuick
import QtQuick.Layouts
import Jarvis.Greeter

/// 01 · STANDARD — the default mode. Glass card with the ring logo,
/// a welcome line that doubles as the username editor (click to
/// change), password entry, and the row of alt-auth pills.
///
/// V1.5 polish: username is bound to `GreeterState.username` and
/// persisted across boots, so "Welcome back, …" reads as personal.
Item {
    id: root
    property string username: GreeterState.username
    property bool editingUsername: false
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

            // Welcome line — click to edit the username.
            // Sized by whichever child is visible. Avoids `childrenRect`
            // (binding loop) and keeps anchored children out of Rows.
            Item {
                id: welcomeLine
                Layout.alignment: Qt.AlignHCenter
                implicitHeight: 28
                implicitWidth: root.editingUsername
                    ? usernameEditor.implicitWidth
                    : welcomeRow.implicitWidth

                Row {
                    id: welcomeRow
                    anchors.centerIn: parent
                    spacing: 4
                    visible: !root.editingUsername

                    Text {
                        text: qsTr("Welcome back, ")
                        color: Theme.text
                        font.pixelSize: 16
                    }
                    Text {
                        text: root.username + "."
                        color: Theme.accent
                        font.pixelSize: 16
                        font.weight: Font.Bold
                    }
                }

                // Click target sits alongside the Row, not inside it —
                // Row rejects anchored children.
                MouseArea {
                    anchors.fill: welcomeRow
                    visible: !root.editingUsername
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        root.editingUsername = true;
                        usernameInput.text = root.username;
                        usernameInput.forceActiveFocus();
                        usernameInput.selectAll();
                    }
                }

                // Editor (active while editingUsername).
                Rectangle {
                    id: usernameEditor
                    visible: root.editingUsername
                    anchors.centerIn: parent
                    implicitWidth: 220
                    implicitHeight: 28
                    radius: 14
                    color: Qt.rgba(1, 1, 1, 0.05)
                    border.color: Theme.accent
                    border.width: 1

                    TextInput {
                        id: usernameInput
                        anchors.fill: parent
                        anchors.leftMargin: 12
                        anchors.rightMargin: 12
                        verticalAlignment: TextInput.AlignVCenter
                        color: Theme.text
                        font.pixelSize: 13
                        clip: true
                        onAccepted: root.commitUsername()
                        Keys.onEscapePressed: root.editingUsername = false
                    }
                }
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
        }
    }

    function commitUsername() {
        const v = usernameInput.text.trim();
        if (v.length > 0) {
            root.username = v;
            GreeterState.username = v;
        }
        root.editingUsername = false;
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
