import QtQuick
import QtQuick.Layouts
import QtQuick.Window
import Jarvis.Lock

/// Fullscreen lock surface. Wallpaper PNG behind a darkened overlay,
/// glass card centred with the eye logo, current time, password
/// field, and an UNLOCK button. The card auto-focuses the password
/// field on every show so the user can type immediately.
Window {
    id: root
    visible: true
    width: 1366
    height: 800
    color: Theme.background
    flags: Qt.FramelessWindowHint

    // Wallpaper (shared with greeter via /branding qrc prefix).
    Image {
        anchors.fill: parent
        source: "qrc:/branding/jarvis-op-default-wallpaper.png"
        sourceSize.width: root.width
        sourceSize.height: root.height
        fillMode: Image.PreserveAspectCrop
        smooth: true
    }

    // Darker overlay than the greeter — locked screens should feel
    // more "sealed" than the login one.
    Rectangle {
        anchors.fill: parent
        color: "#000000"
        opacity: 0.45
    }

    // Top-right clock — same component shape as the greeter but
    // re-implemented inline so the lock window doesn't import the
    // greeter's QML module.
    Column {
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: 32
        spacing: 4

        property string _time: ""
        property string _date: ""
        function refresh() {
            const now = new Date();
            _time = now.toLocaleTimeString(Qt.locale(), "HH:mm");
            _date = now.toLocaleDateString(Qt.locale(), "dddd, dd MMM");
        }
        Component.onCompleted: refresh()
        Timer { interval: 1000; running: true; repeat: true; onTriggered: parent.refresh() }

        Text {
            anchors.right: parent.right
            text: parent._time
            color: Theme.text
            font.pixelSize: 44
            font.weight: Font.Bold
            font.letterSpacing: 2
        }
        Text {
            anchors.right: parent.right
            text: parent._date
            color: Theme.textDim
            font.pixelSize: 13
            font.letterSpacing: 1
        }
    }

    // ── Lock card ────────────────────────────────────────────────
    Rectangle {
        anchors.centerIn: parent
        implicitWidth: 360
        implicitHeight: cardCol.implicitHeight + 56
        radius: 18
        color: Theme.surfaceBright
        border.color: Theme.border
        border.width: 1

        // Soft inner-top highlight.
        Rectangle {
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.margins: 1
            height: 1
            color: Qt.rgba(1, 1, 1, 0.07)
        }

        ColumnLayout {
            id: cardCol
            anchors.fill: parent
            anchors.leftMargin: 32
            anchors.rightMargin: 32
            anchors.topMargin: 28
            anchors.bottomMargin: 28
            spacing: 16

            Text {
                Layout.alignment: Qt.AlignHCenter
                text: qsTr("BLOQUEADO")
                color: Theme.accent
                font.pixelSize: 11
                font.weight: Font.Bold
                font.letterSpacing: 2
            }

            Image {
                Layout.alignment: Qt.AlignHCenter
                source: "qrc:/branding/jarvis-os-default-icon.png"
                sourceSize.width: 180
                sourceSize.height: 180
                width: 90
                height: 90
                smooth: true
                fillMode: Image.PreserveAspectFit
            }

            Text {
                Layout.alignment: Qt.AlignHCenter
                text: qsTr("Digite sua senha para continuar")
                color: Theme.textDim
                font.pixelSize: 13
            }

            // Password field.
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 44
                radius: 22
                color: Qt.rgba(1, 1, 1, 0.04)
                border.color: pwInput.activeFocus ? Theme.accent : Theme.border
                border.width: 1
                Behavior on border.color { ColorAnimation { duration: Theme.animFast } }

                TextInput {
                    id: pwInput
                    anchors.fill: parent
                    anchors.leftMargin: 18
                    anchors.rightMargin: 18
                    verticalAlignment: TextInput.AlignVCenter
                    color: Theme.text
                    selectionColor: Theme.accent
                    selectedTextColor: Theme.text
                    font.pixelSize: 15
                    clip: true
                    echoMode: TextInput.Password
                    enabled: LockClient.state !== "checking"
                    onAccepted: root.submit()
                    Component.onCompleted: forceActiveFocus()
                }

                Text {
                    anchors.fill: pwInput
                    verticalAlignment: Text.AlignVCenter
                    text: qsTr("Senha")
                    color: Theme.textDim
                    font.pixelSize: 15
                    visible: pwInput.text.length === 0 && !pwInput.activeFocus
                }
            }

            // Unlock button.
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 44
                radius: 22
                color: btnArea.containsMouse
                    ? Theme.accent
                    : Qt.darker(Theme.accent, 1.2)
                border.color: Theme.accent
                border.width: 1
                opacity: LockClient.state === "checking" ? 0.55 : 1.0
                Behavior on color { ColorAnimation { duration: Theme.animFast } }
                Behavior on opacity { NumberAnimation { duration: Theme.animFast } }

                Text {
                    anchors.centerIn: parent
                    text: LockClient.state === "checking"
                        ? qsTr("VERIFICANDO…")
                        : qsTr("DESBLOQUEAR")
                    color: Theme.text
                    font.pixelSize: 13
                    font.weight: Font.Bold
                    font.letterSpacing: 2
                }

                MouseArea {
                    id: btnArea
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    enabled: LockClient.state !== "checking"
                        && LockClient.state !== "listening"
                    onClicked: root.submit()
                }
            }

            // Voice-unlock pill — calls the voice-only PAM stack via
            // com.jarvis.Lock.VerifyVoice(). Visually subordinate to
            // the typed-password path so users never feel forced into
            // the voice route. Phase 8 fix to the latency trade-off
            // Phase 7 introduced — see ADR 0020.
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 36
                radius: 18
                color: voiceArea.containsMouse
                    ? Qt.rgba(1, 1, 1, 0.10)
                    : Qt.rgba(1, 1, 1, 0.04)
                border.color: LockClient.state === "listening"
                    ? Theme.accent
                    : Theme.border
                border.width: 1
                opacity: LockClient.state === "checking" ? 0.4 : 1.0
                Behavior on color { ColorAnimation { duration: Theme.animFast } }
                Behavior on border.color { ColorAnimation { duration: Theme.animFast } }

                Text {
                    anchors.centerIn: parent
                    text: LockClient.state === "listening"
                        ? qsTr("OUVINDO…")
                        : qsTr("🎙  FALAR PARA DESBLOQUEAR")
                    color: LockClient.state === "listening"
                        ? Theme.accent
                        : Theme.textDim
                    font.pixelSize: 11
                    font.weight: Font.Bold
                    font.letterSpacing: 1
                }

                MouseArea {
                    id: voiceArea
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    enabled: LockClient.state !== "checking"
                        && LockClient.state !== "listening"
                    onClicked: LockClient.verifyVoice()
                }
            }

            // Error line.
            Text {
                visible: LockClient.error.length > 0
                Layout.fillWidth: true
                text: LockClient.error
                color: Theme.danger
                font.pixelSize: 12
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.WordWrap
            }
        }
    }

    function submit() {
        if (LockClient.state === "checking") return;
        LockClient.verify(pwInput.text);
        pwInput.text = "";
    }
}
