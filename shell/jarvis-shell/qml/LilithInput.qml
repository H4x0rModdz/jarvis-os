import QtQuick
import Jarvis.Shell

/// The conversational input — what the user types into.
/// Emits `accepted(text)` on Enter; the parent dispatches via LilithBridge.
///
/// Uses raw TextInput (not Quick Controls' TextField) so the placeholder
/// behaves like a CLI hint — visible while empty AND unfocused, gone the
/// moment the user clicks in. Material-style floating labels look out of
/// place on a single-line system prompt.
Item {
    id: root
    implicitHeight: 40

    // Placeholder reflects what Lilith is doing right now. The user always
    // knows whether they can type and, if not, why not — instead of staring
    // at a disabled-looking input with no explanation.
    // The streaming/chain state from Phase 10 makes the "Lilith pensando"
    // case far more informative: instead of a wall of text after a long
    // pause, the user sees what tool is running and the assistant's
    // tokens as they arrive.
    property string placeholder: {
        if (LilithBridge.busy && PermissionBridge.hasPending) {
            return qsTr("Aguardando sua aprovação...");
        }
        if (LilithBridge.busy) {
            const steps = LilithBridge.chainSteps;
            if (steps.length > 0) {
                const last = steps[steps.length - 1];
                return qsTr("Lilith → %1…").arg(last.action);
            }
            if (LilithBridge.streamingText.length > 0) {
                // Show the latest characters Lilith has streamed so far.
                // Truncate from the left so the most recent tokens stay
                // visible in a single line of placeholder text.
                const t = LilithBridge.streamingText;
                return t.length > 60 ? "…" + t.substring(t.length - 60) : t;
            }
            return qsTr("Lilith pensando...");
        }
        return qsTr("Diga algo para a Lilith...");
    }
    signal accepted(string text)
    /// Emitted whenever the input field gains focus, so the parent
    /// can open the Lilith popup with its empty-state suggestions
    /// (Phase 12). Bar.qml wires this to lilithPopup.requestOpen().
    signal inputFocused()

    function focusInput() {
        input.forceActiveFocus();
    }

    Rectangle {
        anchors.fill: parent
        radius: Theme.radius - 4
        color: Qt.rgba(1, 1, 1, 0.06)
        border.color: input.activeFocus ? Theme.accent : Theme.border
        border.width: 1

        Behavior on border.color {
            ColorAnimation { duration: Theme.animFast }
        }
    }

    TextInput {
        id: input
        anchors.fill: parent
        anchors.leftMargin: 12
        anchors.rightMargin: 12
        verticalAlignment: TextInput.AlignVCenter
        color: Theme.text
        selectionColor: Theme.accent
        selectedTextColor: Theme.text
        font.pixelSize: 16
        clip: true

        enabled: !LilithBridge.busy
        opacity: enabled ? 1.0 : 0.55

        onActiveFocusChanged: {
            if (activeFocus) root.inputFocused();
        }

        Behavior on opacity {
            NumberAnimation { duration: Theme.animFast }
        }

        onAccepted: {
            const t = text.trim();
            if (t.length === 0) return;
            root.accepted(t);
            text = "";
        }
    }

    // Hint that disappears the instant the field is focused.
    Text {
        anchors.fill: input
        verticalAlignment: Text.AlignVCenter
        text: root.placeholder
        color: Theme.textDim
        font.pixelSize: 16
        visible: input.text.length === 0 && !input.activeFocus
        // Don't intercept clicks — the field underneath has to get focus.
        // (Items default to mouse-transparent unless a MouseArea is added.)
    }
}
