import QtQuick
import QtQuick.Layouts
import QtQuick.Window
import Jarvis.Greeter

/// Greeter root. The wallpaper PNG handles the brand mark + slogan +
/// Lilith silhouette; the QML layer adds the interactive cards on top.
Window {
    id: root
    visible: true
    width: 1366
    height: 800
    title: qsTr("LilithOS")
    color: Theme.background

    Component.onCompleted: {
        showFullScreen();
    }

    // ── Wallpaper ────────────────────────────────────────────────
    // The PNG already contains the JARVIS / OS logo on the left,
    // the "Conscious systems begin with understanding." slogan in
    // the lower left, and the Lilith character on the right. Filling
    // the screen with it removes the need for a separate slogan
    // banner or starfield decoration from V1.
    Image {
        anchors.fill: parent
        source: "qrc:/branding/jarvis-op-default-wallpaper.png"
        sourceSize.width: root.width
        sourceSize.height: root.height
        fillMode: Image.PreserveAspectCrop
        smooth: true
    }

    // Subtle vignette + tint so the cards stay legible even when the
    // wallpaper has bright regions behind them.
    Rectangle {
        anchors.fill: parent
        color: "#000000"
        opacity: 0.28
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
        anchors.topMargin: 90
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
            text: "LilithOS  2.0"
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
