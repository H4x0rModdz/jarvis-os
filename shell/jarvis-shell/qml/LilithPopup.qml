import QtQuick
import QtQuick.Layouts
import QtQuick.Window
import Jarvis.Shell

/// Floating conversation panel that auto-shows above the bar whenever
/// Lilith is processing a command. Renders the conversation history
/// from LilithBridge.conversation plus the live streaming text + chain
/// step pills for the in-flight command.
///
/// Lifecycle:
///   - Bridge.busy goes true (send() fired) → window shows.
///   - While busy or streaming, stays visible and scrolls to the bottom
///     so the latest tokens are always in view.
///   - After busy turns false, the fade timer starts; once it fires the
///     window hides.
///   - Click outside the window (active = false) hides immediately —
///     same convention as the launcher and notification drawer.
///   - The "limpar" button calls Bridge.resetConversation() to wipe
///     both the local list and the daemon's session memory.
Window {
    id: root
    visible: false
    width: 480
    height: 380
    color: "transparent"
    flags: Qt.Tool | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
    title: qsTr("Lilith")

    /// Gap from the bottom edge so the popup sits just above the dock
    /// without overlapping it (the floating dock is ~90px tall).
    property int bottomGap: 104

    function requestOpen() {
        if (Qt.application.screens.length > 0) {
            const s = Qt.application.screens[0];
            x = s.virtualX + Math.floor((s.width - width) / 2);
            y = s.virtualY + s.height - height - bottomGap;
        }
        visible = true;
        // Auto-open path (busy / proactive) does NOT grab focus — only
        // an explicit orb click (toggle) activates + focuses the input.
    }

    /// Orb-click entry point: open + activate + focus the input so the
    /// user can type immediately. Clicking the orb again closes.
    function toggle() {
        if (visible) {
            hideNow();
            return;
        }
        requestOpen();
        requestActivate();
        promptInput.forceActiveFocus();
    }

    function hideNow() {
        visible = false;
        fadeTimer.stop();
    }

    // ── Error surfacing ─────────────────────────────────────────────
    // The retired bottom bar used to show Lilith/voice errors in its
    // reply box; that box is gone, so the popup owns errors now. Shows
    // a red row, pops the popup, and fades on the same timer.
    property string errorText: ""
    Connections {
        target: LilithBridge
        function onErrorOccurred(message) {
            root.errorText = qsTr("Erro: ") + message;
            if (!root.visible) root.requestOpen();
            fadeTimer.restart();
        }
    }
    Connections {
        target: VoiceBridge
        function onLastErrorChanged() {
            const e = VoiceBridge.lastError;
            if (e.length > 0) {
                root.errorText = qsTr("Voz: ") + e;
                if (!root.visible) root.requestOpen();
                fadeTimer.restart();
            }
        }
    }

    // ── Auto-show / auto-fade lifecycle ─────────────────────────────
    Connections {
        target: LilithBridge
        function onBusyChanged() {
            if (LilithBridge.busy) {
                root.requestOpen();
                fadeTimer.stop();
            } else if (root.visible) {
                // Reply landed. Give the user time to read before we
                // fade — 8 s is the same window Phase 8's lock toast
                // settled on. Streaming may still be flushing when
                // busy flips, so start the timer here, not when text
                // stops growing.
                fadeTimer.restart();
            }
        }
        // Proactive nudge arrived — pop the popup if it's closed so
        // the banner is actually seen. Don't fade automatically;
        // proactive messages stay until the user dismisses them
        // (cancelling fadeTimer if it was running).
        function onProactiveNudgeReceived(rule, text, urgency) {
            if (!root.visible) root.requestOpen();
            fadeTimer.stop();
        }
    }

    // ESC closes immediately when the popup itself has focus.
    Shortcut {
        sequence: "Escape"
        onActivated: root.hideNow()
    }

    Timer {
        id: fadeTimer
        interval: 8000
        repeat: false
        onTriggered: root.hideNow()
    }

    GlassPanel {
        anchors.fill: parent
        anchors.margins: 8

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 16
            spacing: 8

            // ── Header ─────────────────────────────────────────────
            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                Text {
                    text: qsTr("LILITH")
                    color: Theme.accent
                    font.pixelSize: 11
                    font.weight: Font.Bold
                    font.letterSpacing: 2
                    Layout.fillWidth: true
                }

                Item {
                    implicitWidth: clearLabel.implicitWidth + 16
                    implicitHeight: 22
                    visible: LilithBridge.conversation.length > 0

                    Rectangle {
                        anchors.fill: parent
                        radius: 11
                        color: clearArea.containsMouse
                            ? Qt.rgba(1, 1, 1, 0.08)
                            : Qt.rgba(1, 1, 1, 0.04)
                        border.color: Theme.border
                        border.width: 1
                    }
                    Text {
                        id: clearLabel
                        anchors.centerIn: parent
                        text: qsTr("LIMPAR")
                        color: Theme.textDim
                        font.pixelSize: 9
                        font.weight: Font.Bold
                        font.letterSpacing: 1
                    }
                    MouseArea {
                        id: clearArea
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: LilithBridge.resetConversation()
                    }
                }

                Text {
                    text: "×"
                    color: closeArea.containsMouse ? Theme.text : Theme.textDim
                    font.pixelSize: 18
                    font.weight: Font.Bold
                    Layout.preferredWidth: 22
                    horizontalAlignment: Text.AlignHCenter

                    MouseArea {
                        id: closeArea
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.hideNow()
                    }
                }
            }

            // ── Error row ──────────────────────────────────────────
            // Red strip for Lilith / voice errors (the retired bar's
            // reply box used to own this). Cleared when the user sends
            // a new message.
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: errLabel.implicitHeight + 16
                visible: root.errorText.length > 0
                radius: 10
                color: Qt.rgba(Theme.danger.r, Theme.danger.g, Theme.danger.b, 0.16)
                border.color: Theme.danger
                border.width: 1
                Text {
                    id: errLabel
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    text: root.errorText
                    color: Theme.text
                    font.pixelSize: 13
                    wrapMode: Text.WordWrap
                }
            }

            // ── Empty-state suggestions ────────────────────────────
            // First-run onboarding without a tutorial: when the
            // conversation is empty (fresh boot, after LIMPAR, etc.)
            // we surface four clickable prompts so the user sees
            // what Lilith can do without trial-and-error.
            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 10
                visible: LilithBridge.conversation.length === 0
                    && !LilithBridge.busy
                    && LilithBridge.streamingText.length === 0

                Text {
                    Layout.alignment: Qt.AlignHCenter
                    Layout.topMargin: 24
                    text: qsTr("Diga algo para começar.")
                    color: Theme.textDim
                    font.pixelSize: 13
                }

                GridLayout {
                    Layout.alignment: Qt.AlignHCenter
                    columns: 2
                    rowSpacing: 8
                    columnSpacing: 8

                    Repeater {
                        // The list reflects the four most-used action
                        // namespaces. Order matters: install → see →
                        // capture → reach-the-web parallels the
                        // typical "set up my desktop" flow.
                        model: [
                            { label: qsTr("Abrir o navegador"),    prompt: "abrir o navegador" },
                            { label: qsTr("Tirar um screenshot"),  prompt: "tirar um screenshot" },
                            { label: qsTr("Instalar o GIMP"),      prompt: "instalar o gimp" },
                            { label: qsTr("O que você sabe fazer?"), prompt: "o que você sabe fazer" },
                        ]
                        delegate: Rectangle {
                            implicitWidth: 196
                            implicitHeight: 56
                            radius: 10
                            color: suggestionArea.containsMouse
                                ? Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.18)
                                : Qt.rgba(1, 1, 1, 0.04)
                            border.color: suggestionArea.containsMouse
                                ? Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.50)
                                : Theme.border
                            border.width: 1
                            Behavior on color { ColorAnimation { duration: Theme.animFast } }
                            Behavior on border.color { ColorAnimation { duration: Theme.animFast } }

                            Text {
                                anchors.fill: parent
                                anchors.margins: 10
                                text: modelData.label
                                color: Theme.text
                                font.pixelSize: 12
                                wrapMode: Text.WordWrap
                                horizontalAlignment: Text.AlignHCenter
                                verticalAlignment: Text.AlignVCenter
                            }

                            MouseArea {
                                id: suggestionArea
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: LilithBridge.send(modelData.prompt)
                            }
                        }
                    }
                }

                Item { Layout.fillHeight: true } // pushes grid up
            }

            // ── Proactive nudge banner ─────────────────────────────
            // Renders above the conversation when LilithBridge has a
            // live nudge. Accent strip color tracks urgency. Click
            // the × to dismiss (UI-side only; daemon cooldown is
            // unaffected).
            Rectangle {
                id: proactiveBanner
                Layout.fillWidth: true
                Layout.preferredHeight: bannerCol.implicitHeight + 16
                visible: LilithBridge.proactiveNudgeText.length > 0
                radius: 10
                color: {
                    const u = LilithBridge.proactiveNudgeUrgency;
                    if (u === "critical")
                        return Qt.rgba(Theme.danger.r, Theme.danger.g, Theme.danger.b, 0.16);
                    if (u === "warning")
                        return Qt.rgba(1, 0.71, 0.28, 0.16);  // amber 0xffb547
                    return Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.16);
                }
                border.width: 1
                border.color: {
                    const u = LilithBridge.proactiveNudgeUrgency;
                    if (u === "critical") return Theme.danger;
                    if (u === "warning") return "#ffb547";
                    return Theme.accent;
                }

                // Left accent strip — a 4px tall colored bar at the
                // top edge, mirroring the notification-toast style.
                Rectangle {
                    anchors.left: parent.left
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    width: 4
                    radius: 2
                    color: parent.border.color
                }

                RowLayout {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.leftMargin: 14
                    anchors.rightMargin: 8
                    spacing: 8

                    ColumnLayout {
                        id: bannerCol
                        Layout.fillWidth: true
                        spacing: 2
                        Text {
                            text: {
                                const u = LilithBridge.proactiveNudgeUrgency;
                                if (u === "critical") return qsTr("LILITH — ATENÇÃO");
                                if (u === "warning")  return qsTr("LILITH — AVISO");
                                return qsTr("LILITH");
                            }
                            color: proactiveBanner.border.color
                            font.pixelSize: 9
                            font.weight: Font.Bold
                            font.letterSpacing: 1
                        }
                        Text {
                            text: LilithBridge.proactiveNudgeText
                            color: Theme.text
                            font.pixelSize: 13
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                        }
                    }

                    Text {
                        text: "×"
                        color: dismissArea.containsMouse ? Theme.danger : Theme.textDim
                        font.pixelSize: 18
                        font.weight: Font.Bold
                        Layout.preferredWidth: 22
                        horizontalAlignment: Text.AlignHCenter
                        MouseArea {
                            id: dismissArea
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: LilithBridge.dismissProactiveNudge()
                        }
                    }
                }
            }

            // ── Conversation list ──────────────────────────────────
            ListView {
                id: convoList
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: LilithBridge.conversation.length > 0
                    || LilithBridge.busy
                    || LilithBridge.streamingText.length > 0
                clip: true
                spacing: 8
                model: LilithBridge.conversation
                // Scroll to the bottom whenever the model grows so
                // the latest entry is always visible.
                onCountChanged: positionViewAtEnd()
                Component.onCompleted: positionViewAtEnd()

                delegate: Item {
                    width: ListView.view.width
                    implicitHeight: bubble.implicitHeight

                    Rectangle {
                        id: bubble
                        anchors.left: modelData.role === "user" ? undefined : parent.left
                        anchors.right: modelData.role === "user" ? parent.right : undefined
                        implicitWidth: Math.min(parent.width * 0.85, contentCol.implicitWidth + 24)
                        implicitHeight: contentCol.implicitHeight + 16
                        radius: 12
                        color: modelData.role === "user"
                            ? Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.18)
                            : Qt.rgba(1, 1, 1, 0.05)
                        border.color: modelData.role === "user"
                            ? Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.40)
                            : Theme.border
                        border.width: 1

                        ColumnLayout {
                            id: contentCol
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.top: parent.top
                            anchors.margins: 10
                            spacing: 4

                            Text {
                                text: modelData.role === "user" ? qsTr("VOCÊ") : qsTr("LILITH")
                                color: modelData.role === "user" ? Theme.accent : Theme.textDim
                                font.pixelSize: 9
                                font.weight: Font.Bold
                                font.letterSpacing: 1
                            }

                            Text {
                                text: modelData.text || ""
                                color: Theme.text
                                font.pixelSize: 13
                                wrapMode: Text.WordWrap
                                Layout.fillWidth: true
                            }

                            // Chain-step badges — one pill per tool
                            // call that ran while Lilith composed
                            // this reply.
                            Flow {
                                Layout.fillWidth: true
                                spacing: 4
                                visible: modelData.role === "lilith"
                                    && modelData.chainSteps !== undefined
                                    && modelData.chainSteps.length > 0

                                Repeater {
                                    model: modelData.chainSteps || []
                                    delegate: Rectangle {
                                        implicitWidth: stepLabel.implicitWidth + 14
                                        implicitHeight: 18
                                        radius: 9
                                        color: Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.18)
                                        border.color: Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.40)
                                        border.width: 1

                                        Text {
                                            id: stepLabel
                                            anchors.centerIn: parent
                                            text: "→ " + (modelData.action || "")
                                            color: Theme.text
                                            font.pixelSize: 9
                                            font.weight: Font.Bold
                                            font.letterSpacing: 0.5
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── Live in-flight strip ──────────────────────────────
            // While Lilith is mid-command (busy or streaming text
            // arriving) we render an extra "live" row below the
            // committed history. Disappears when the reply lands
            // and the entry joins the conversation list above.
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: liveCol.implicitHeight + 16
                visible: LilithBridge.busy || LilithBridge.streamingText.length > 0
                radius: 12
                color: Qt.rgba(1, 1, 1, 0.03)
                border.color: Theme.accent
                border.width: 1

                ColumnLayout {
                    id: liveCol
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: 10
                    spacing: 4

                    RowLayout {
                        spacing: 6
                        Text {
                            text: qsTr("LILITH")
                            color: Theme.accent
                            font.pixelSize: 9
                            font.weight: Font.Bold
                            font.letterSpacing: 1
                        }
                        // Tiny breathing dot to signal "alive, working".
                        Rectangle {
                            implicitWidth: 6
                            implicitHeight: 6
                            radius: 3
                            color: Theme.accent
                            SequentialAnimation on opacity {
                                loops: Animation.Infinite
                                running: LilithBridge.busy
                                NumberAnimation { to: 0.3; duration: 600; easing.type: Easing.InOutSine }
                                NumberAnimation { to: 1.0; duration: 600; easing.type: Easing.InOutSine }
                            }
                        }
                    }

                    Flow {
                        Layout.fillWidth: true
                        spacing: 4
                        visible: LilithBridge.chainSteps.length > 0

                        Repeater {
                            model: LilithBridge.chainSteps
                            delegate: Rectangle {
                                implicitWidth: liveStepLabel.implicitWidth + 14
                                implicitHeight: 18
                                radius: 9
                                color: Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.18)
                                border.color: Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.40)
                                border.width: 1
                                Text {
                                    id: liveStepLabel
                                    anchors.centerIn: parent
                                    text: "→ " + (modelData.action || "")
                                    color: Theme.text
                                    font.pixelSize: 9
                                    font.weight: Font.Bold
                                    font.letterSpacing: 0.5
                                }
                            }
                        }
                    }

                    Text {
                        visible: LilithBridge.streamingText.length > 0
                        text: LilithBridge.streamingText
                        color: Theme.text
                        font.pixelSize: 13
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                }
            }

            // ── Prompt input ───────────────────────────────────────
            // The conversation surface is now the only place to type to
            // Lilith (the always-on bar input is retired). Enter sends
            // and clears; sending also wipes any error row.
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 40
                radius: Theme.radius - 4
                color: Qt.rgba(1, 1, 1, 0.05)
                border.color: promptInput.activeFocus ? Theme.accent : Theme.border
                border.width: 1
                Behavior on border.color { ColorAnimation { duration: Theme.animFast } }

                TextInput {
                    id: promptInput
                    anchors.fill: parent
                    anchors.leftMargin: 14
                    anchors.rightMargin: 14
                    verticalAlignment: TextInput.AlignVCenter
                    color: Theme.text
                    selectionColor: Theme.accent
                    selectedTextColor: Theme.text
                    font.pixelSize: 14
                    clip: true
                    enabled: !LilithBridge.busy
                    onAccepted: {
                        const t = text.trim();
                        if (t.length === 0) return;
                        root.errorText = "";
                        LilithBridge.send(t);
                        text = "";
                    }
                }

                Text {
                    anchors.fill: promptInput
                    verticalAlignment: Text.AlignVCenter
                    text: qsTr("Pergunte à Lilith…")
                    color: Theme.textDim
                    font.pixelSize: 14
                    visible: promptInput.text.length === 0
                }
            }
        }
    }
}
