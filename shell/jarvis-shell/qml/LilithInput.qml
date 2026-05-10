import QtQuick
import QtQuick.Controls
import Jarvis.Shell

/// The conversational input — what the user types into.
/// Emits `accepted(text)` on Enter; the parent dispatches via LilithBridge.
Item {
    id: root
    implicitHeight: 40

    property string placeholder: qsTr("Diga algo para a Lilith...")
    signal accepted(string text)

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

    TextField {
        id: input
        anchors.fill: parent
        anchors.leftMargin: 12
        anchors.rightMargin: 12
        verticalAlignment: TextInput.AlignVCenter

        // Suppress the Material underline — the rounded glass surface IS the chrome.
        background: Item {}

        placeholderText: root.placeholder
        placeholderTextColor: Theme.textDim
        color: Theme.text
        selectionColor: Theme.accent
        selectedTextColor: Theme.text
        font.pixelSize: 16

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
}
