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
    // State machine so the user always knows what Lilith is doing:
    //   idle              → hidden
    //   thinking          → "Lilith pensando" + animated dots, no auto-hide
    //   awaiting_approval → "Aguardando sua aprovação" + dots, no auto-hide
    //   resolved          → real reply text, hides after 6 s
    //   error             → red error line, hides after 6 s
    Rectangle {
        id: reply
        anchors.bottom: bar.top
        anchors.horizontalCenter: bar.horizontalCenter
        anchors.bottomMargin: 12
        width: bar.width * 0.7
        radius: Theme.radius
        color: Theme.surfaceBright
        border.color: replyState === "error" ? Theme.danger : Theme.border
        border.width: 1

        property string replyState: "idle"
        property string resolvedText: ""
        property string resolvedAction: ""
        property string errorText: ""

        Behavior on border.color {
            ColorAnimation { duration: Theme.animFast }
        }

        readonly property bool showing: replyState !== "idle"
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

            // Header chip (action namespace, e.g. "browser.open") on resolved.
            Text {
                visible: reply.replyState === "resolved" && reply.resolvedAction.length > 0
                text: reply.resolvedAction
                color: Theme.accent
                font.pixelSize: 11
                font.weight: Font.Bold
                font.capitalization: Font.AllUppercase
            }

            // Intermediate state: thinking / awaiting_approval.
            Row {
                visible: reply.replyState === "thinking" || reply.replyState === "awaiting_approval"
                spacing: 8

                Text {
                    text: reply.replyState === "awaiting_approval"
                        ? qsTr("Aguardando sua aprovação")
                        : qsTr("Lilith pensando")
                    color: reply.replyState === "awaiting_approval" ? Theme.accent : Theme.textDim
                    font.pixelSize: 14
                }

                // Three pulsing dots — animated entrance staggered by 200 ms.
                Row {
                    spacing: 4
                    anchors.verticalCenter: parent.verticalCenter

                    Repeater {
                        model: 3
                        Rectangle {
                            width: 4
                            height: 4
                            radius: 2
                            color: reply.replyState === "awaiting_approval"
                                ? Theme.accent
                                : Theme.textDim

                            SequentialAnimation on opacity {
                                loops: Animation.Infinite
                                running: reply.replyState === "thinking"
                                      || reply.replyState === "awaiting_approval"
                                PauseAnimation { duration: index * 200 }
                                NumberAnimation { from: 0.3; to: 1.0; duration: 400 }
                                NumberAnimation { from: 1.0; to: 0.3; duration: 400 }
                                PauseAnimation { duration: (2 - index) * 200 }
                            }
                        }
                    }
                }
            }

            // Resolved text.
            Text {
                visible: reply.replyState === "resolved"
                text: reply.resolvedText
                color: Theme.text
                font.pixelSize: 15
                wrapMode: Text.WordWrap
                width: parent.width
            }

            // Error text — same place, red.
            Text {
                visible: reply.replyState === "error"
                text: reply.errorText
                color: Theme.danger
                font.pixelSize: 15
                wrapMode: Text.WordWrap
                width: parent.width
            }
        }

        Timer {
            id: hideTimer
            interval: 6000
            onTriggered: reply.replyState = "idle"
        }
    }

    // Wire the popup state machine to the bridges. Order matters:
    //   1. busy flips true       → "thinking"
    //   2. approval signal lands → "awaiting_approval"
    //   3. user resolves         → back to "thinking" (or directly to denial-reply)
    //   4. replyReceived/error   → "resolved"/"error" + hide timer
    Connections {
        target: LilithBridge
        function onBusyChanged() {
            if (LilithBridge.busy && reply.replyState === "idle") {
                reply.replyState = "thinking";
            } else if (!LilithBridge.busy
                       && (reply.replyState === "thinking"
                           || reply.replyState === "awaiting_approval")) {
                // Busy dropped without a reply/error event — rare, but reset.
                reply.replyState = "idle";
            }
        }
        function onReplyReceived(replyText, action, resultJson) {
            reply.resolvedText = replyText;
            reply.resolvedAction = action;
            reply.errorText = "";
            reply.replyState = "resolved";
            hideTimer.restart();
        }
        function onErrorOccurred(message) {
            reply.errorText = qsTr("Erro: ") + message;
            reply.resolvedText = "";
            reply.resolvedAction = "";
            reply.replyState = "error";
            hideTimer.restart();
        }
    }

    Connections {
        target: PermissionBridge
        function onPendingChanged() {
            if (PermissionBridge.hasPending && reply.replyState === "thinking") {
                reply.replyState = "awaiting_approval";
            } else if (!PermissionBridge.hasPending
                       && reply.replyState === "awaiting_approval") {
                // User clicked a button; Lilith continues processing.
                reply.replyState = "thinking";
            }
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
        onSettingsRequested: settingsPanel.requestOpen()
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

    // When the voice daemon delivers a transcript, hand it to Lilith as
    // if the user had typed it. Same audit path, same permission gating,
    // same reply popup — voice is just another input modality.
    Connections {
        target: VoiceBridge
        function onLastTranscriptChanged() {
            const t = VoiceBridge.lastTranscript;
            if (t.length > 0 && !LilithBridge.busy) {
                LilithBridge.send(t);
            }
        }
        function onLastErrorChanged() {
            const e = VoiceBridge.lastError;
            if (e.length > 0) {
                // Reuse the same reply popup the LilithBridge errors flow into.
                reply.errorText = qsTr("Voz: ") + e;
                reply.resolvedText = "";
                reply.resolvedAction = "";
                reply.replyState = "error";
                hideTimer.restart();
            }
        }
    }

    // Auto-speak Lilith replies through the voice daemon. Whatever
    // Lilith says in the reply popup also comes out the speakers. The
    // user can turn this off in the SettingsPanel (key
    // `voice.tts_enabled`, default true). When the voice daemon isn't
    // reachable (or piper/paplay are missing) the call no-ops on the
    // bridge side.
    Connections {
        target: LilithBridge
        function onReplyReceived(replyText, action, resultJson) {
            const enabled = SettingsBridge.getBool("voice.tts_enabled", true);
            if (enabled && VoiceBridge.reachable && replyText.trim().length > 0) {
                VoiceBridge.speak(replyText);
            }
        }
    }

    // The approval dialog is a sibling Window — opens on top of the desktop
    // when PermissionBridge has a pending request, closes on user decision.
    ApprovalDialog {}

    // First-boot updater splash. Bound to UpdaterBridge.active — invisible
    // until the daemon emits its first Progress, dismisses on Completed.
    UpdaterSplash {}

    // Preferences panel — opens when the user clicks the gear on the bar.
    SettingsPanel { id: settingsPanel }

    // Toast for incoming notifications. Bound to NotificationsBridge —
    // bottom-right of the primary output, 5s auto-hide, click to
    // dismiss early.
    NotificationToast {}
}
