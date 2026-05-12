import QtQuick
import QtQuick.Layouts
import Jarvis.Greeter

/// 03 · FOCUS — minimal mode. Strips effects to the bare minimum
/// (no logo orbit, no glass, no glow). Designed for slow hardware,
/// low battery, and users who just want to log in fast.
Item {
    id: root
    property string username: GreeterState.username

    ColumnLayout {
        anchors.centerIn: parent
        spacing: 22
        width: 320

        Text {
            Layout.alignment: Qt.AlignHCenter
            text: qsTr("03 · FOCUS")
            color: Theme.textDim
            font.pixelSize: 10
            font.letterSpacing: 2
        }

        // Chevron — drawn from two rotated rectangles. No animation.
        Item {
            Layout.alignment: Qt.AlignHCenter
            implicitWidth: 56
            implicitHeight: 56

            Rectangle {
                x: 12; y: 14
                width: 4; height: 32
                radius: 2
                color: Theme.accent
                transform: Rotation { origin.x: 2; origin.y: 16; angle: -35 }
            }
            Rectangle {
                x: 36; y: 14
                width: 4; height: 32
                radius: 2
                color: Theme.accent
                transform: Rotation { origin.x: 2; origin.y: 16; angle: 35 }
            }
        }

        Text {
            Layout.alignment: Qt.AlignHCenter
            text: "JARVIS OS"
            color: Theme.text
            font.pixelSize: 22
            font.weight: Font.Bold
            font.letterSpacing: 4
        }

        PasswordField {
            id: pwField
            Layout.fillWidth: true
            placeholder: qsTr("Password")
            onAccepted: root.submit()
        }

        UnlockButton {
            Layout.fillWidth: true
            label: qsTr("UNLOCK")
            busy: GreetdClient.state === "checking"
               || GreetdClient.state === "starting_session"
            onClicked: root.submit()
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
