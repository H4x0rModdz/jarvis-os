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
    property string placeholder: {
        if (LilithBridge.busy && PermissionBridge.hasPending) {
            return qsTr("Aguardando sua aprovação...");
        }
        if (LilithBridge.busy) {
            return qsTr("Lilith pensando...");
        }
        return qsTr("Diga algo para a Lilith...");
    }
    signal accepted(string text)

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
