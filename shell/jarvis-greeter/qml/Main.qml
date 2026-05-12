import QtQuick
import QtQuick.Layouts
import QtQuick.Window
import Jarvis.Greeter

/// Greeter root. Dark background with a slogan banner at the top,
/// a faint starfield, the LoginScreen with the three-mode SwipeView,
/// and a minimal footer line.
Window {
    id: root
    visible: true
    width: 1366
    height: 800
    title: qsTr("Jarvis OS")
    color: Theme.background

    Component.onCompleted: {
        showFullScreen();
    }

    // Faint starfield — a handful of dots sprinkled across the
    // background. Pure decoration; replaced by a particle / shader
    // pass once the compositor work in Phase 3 owns the rendering.
    Repeater {
        model: 42

        Rectangle {
            x: Math.random() * root.width
            y: Math.random() * root.height
            width: Math.random() < 0.85 ? 1 : 2
            height: width
            radius: width / 2
            color: "#aab0c8"
            opacity: 0.15 + Math.random() * 0.35
        }
    }

    // ── Slogan ───────────────────────────────────────────────────
    ColumnLayout {
        anchors.top: parent.top
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.topMargin: 32
        spacing: 4

        Text {
            Layout.alignment: Qt.AlignHCenter
            text: "JARVIS"
            color: Theme.text
            font.pixelSize: 32
            font.weight: Font.Bold
            font.letterSpacing: 6
        }
        Text {
            Layout.alignment: Qt.AlignHCenter
            text: "OS"
            color: Theme.accent
            font.pixelSize: 10
            font.letterSpacing: 4
            font.weight: Font.Bold
        }
        Text {
            Layout.alignment: Qt.AlignHCenter
            Layout.topMargin: 6
            text: qsTr("Conscious systems begin with understanding.")
            color: Theme.textDim
            font.pixelSize: 12
            font.italic: true
        }
    }

    // ── Clock (top-right) ────────────────────────────────────────
    Clock {
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.topMargin: 28
        anchors.rightMargin: 32
    }

    // ── Login (the SwipeView lives here) ─────────────────────────
    LoginScreen {
        id: login
        anchors.fill: parent
        anchors.topMargin: 130
        anchors.bottomMargin: 70

        onInfoMessage: function(text) {
            toast.error = false;
            toast.message = text;
        }
        onErrorMessage: function(text) {
            toast.error = true;
            toast.message = text;
        }
    }

    // ── Toast (above the footer) ─────────────────────────────────
    Toast {
        id: toast
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 60
        anchors.horizontalCenter: parent.horizontalCenter
    }

    // ── Footer ───────────────────────────────────────────────────
    RowLayout {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.margins: 22

        Text {
            text: "Jarvis OS  2.0"
            color: Theme.textDim
            font.pixelSize: 11
            font.letterSpacing: 1
        }
        Item { Layout.fillWidth: true }
        Text {
            text: qsTr("Secure  ·  Adaptive  ·  Conscious")
            color: Theme.textDim
            font.pixelSize: 11
            font.letterSpacing: 1
        }
    }
}
