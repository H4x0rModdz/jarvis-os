import QtQuick
import QtQuick.Window
import Jarvis.Shell

/// Phase 1a top-level: a borderless transparent window holding the bar
/// and a reply popup. Sized to a wide bar across the bottom of a 1920x60
/// reference area; the window centers itself on first show.
Window {
    id: root
    visible: true
    width: 1280
    height: Theme.barHeight + 80   // room for the reply popup above the bar
    color: "transparent"
    flags: Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint

    // Center on primary screen on first paint AND ask the compositor to
    // make us the active surface — otherwise on labwc/wlroots the user has
    // to click the bar before its TextInput can accept keystrokes.
    Component.onCompleted: {
        const s = Qt.application.screens[0];
        if (s) {
            x = s.virtualX + Math.floor((s.width - width) / 2);
            y = s.virtualY + s.height - height - 24;
        }
        requestActivate();
    }

    // ── Reply popup ───────────────────────────────────────────────────
    Rectangle {
        id: reply
        anchors.bottom: bar.top
        anchors.horizontalCenter: bar.horizontalCenter
        anchors.bottomMargin: 12
        width: bar.width * 0.7
        radius: Theme.radius
        color: Theme.surfaceBright
        border.color: Theme.border
        border.width: 1

        property string text: ""
        property string action: ""
        property bool showing: false

        opacity: showing ? 1.0 : 0.0
        scale: showing ? 1.0 : 0.96
        visible: opacity > 0.01
        implicitHeight: msg.implicitHeight + 24

        Behavior on opacity { NumberAnimation { duration: Theme.animNormal; easing.type: Easing.OutQuad } }
        Behavior on scale { NumberAnimation { duration: Theme.animNormal; easing.type: Easing.OutCubic } }

        Column {
            id: msg
            anchors.fill: parent
            anchors.margins: 12
            spacing: 4

            Text {
                visible: reply.action.length > 0
                text: reply.action
                color: Theme.accent
                font.pixelSize: 11
                font.weight: Font.Bold
                font.capitalization: Font.AllUppercase
            }
            Text {
                text: reply.text
                color: Theme.text
                font.pixelSize: 15
                wrapMode: Text.WordWrap
                width: parent.width
            }
        }

        Timer {
            id: hideTimer
            interval: 6000
            onTriggered: reply.showing = false
        }

        function show(t, a) {
            text = t;
            action = a;
            showing = true;
            hideTimer.restart();
        }
    }

    Bar {
        id: bar
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: 8
        height: Theme.barHeight
        onLauncherRequested: launcher.visible ? launcher.close() : launcher.open()
    }

    Launcher {
        id: launcher
        // When the launcher hides, hand keystrokes back to the bar's input.
        // requestActivate (called inside Launcher.close()) only reactivates
        // the parent Window — it doesn't restore a specific focus target
        // inside it. We do that explicitly here.
        onVisibleChanged: {
            if (!visible) {
                bar.focusInput();
            }
        }
    }

    Connections {
        target: LilithBridge
        function onReplyReceived(replyText, action, resultJson) {
            reply.show(replyText, action);
        }
        function onErrorOccurred(message) {
            reply.show(qsTr("Erro: ") + message, "");
        }
    }

    // The approval dialog is a sibling Window — opens on top of the desktop
    // when PermissionBridge has a pending request, closes on user decision.
    ApprovalDialog {}
}
