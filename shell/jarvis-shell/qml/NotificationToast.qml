import QtQuick
import QtQuick.Layouts
import QtQuick.Window
import Jarvis.Shell

/// A single toast bubble that lives in the bottom-right of the primary
/// output. Bound to NotificationsBridge — every increment of `tick`
/// re-positions, re-fills, and restarts the auto-hide timer.
///
/// V1 surface: one toast at a time. A burst of three notifications in
/// quick succession will visually look like the last one overwriting
/// the previous two. V2 will stack them.
Window {
    id: root
    width: 360
    height: contentColumn.implicitHeight + 40
    title: qsTr("Notificação")
    color: "transparent"
    visible: false
    flags: Qt.Tool | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
                   | Qt.WindowDoesNotAcceptFocus

    // The toast doesn't take focus — we don't want it to steal the
    // bar's input when a notification fires while the user is typing
    // to Lilith. Qt.WindowDoesNotAcceptFocus + Qt.Tool together
    // achieve that on labwc/wlroots.

    function place() {
        if (Qt.application.screens.length === 0) return;
        const s = Qt.application.screens[0];
        x = s.virtualX + s.width - width - 24;
        // Above the bar (Theme.barHeight + bottom margin).
        y = s.virtualY + s.height - height - (Theme.barHeight + 32);
    }

    Connections {
        target: NotificationsBridge
        function onNotificationChanged() {
            if (NotificationsBridge.currentSummary.length === 0
                && NotificationsBridge.currentBody.length === 0) {
                return;
            }
            root.place();
            root.visible = true;
            hideTimer.restart();
        }
    }

    Timer {
        id: hideTimer
        interval: 5000
        onTriggered: root.visible = false
    }

    Rectangle {
        anchors.fill: parent
        anchors.margins: 8
        radius: Theme.radius
        color: Theme.surfaceBright
        border.color: {
            switch (NotificationsBridge.currentUrgency) {
                case "critical": return Theme.danger;
                case "low":      return Theme.border;
                default:         return Theme.accent;
            }
        }
        border.width: 1

        ColumnLayout {
            id: contentColumn
            anchors.fill: parent
            anchors.margins: 14
            spacing: 4

            Text {
                text: NotificationsBridge.currentApp.length > 0
                    ? NotificationsBridge.currentApp.toUpperCase()
                    : qsTr("JARVIS")
                color: Theme.accent
                font.pixelSize: 10
                font.weight: Font.Bold
            }

            Text {
                visible: NotificationsBridge.currentSummary.length > 0
                text: NotificationsBridge.currentSummary
                color: Theme.text
                font.pixelSize: 14
                font.weight: Font.Bold
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }

            Text {
                visible: NotificationsBridge.currentBody.length > 0
                text: NotificationsBridge.currentBody
                color: Theme.textDim
                font.pixelSize: 13
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }

            // V2 — action buttons. The daemon hands us a flat
            // `key, label, key, label, …` list. Pair them up at
            // render time. Clicking sends the user's choice back
            // through NotificationsBridge.invokeAction.
            Flow {
                Layout.fillWidth: true
                Layout.topMargin: 6
                spacing: 6
                visible: NotificationsBridge.currentActions.length >= 2

                Repeater {
                    model: Math.floor(NotificationsBridge.currentActions.length / 2)

                    Rectangle {
                        readonly property string actionKey:
                            NotificationsBridge.currentActions[index * 2]
                        readonly property string actionLabel:
                            NotificationsBridge.currentActions[index * 2 + 1]

                        height: 26
                        width: btnText.implicitWidth + 22
                        radius: 13
                        color: btnArea.containsMouse
                            ? Theme.accent
                            : Qt.rgba(0.49, 0.36, 1.0, 0.18)
                        border.color: Theme.accent
                        border.width: 1
                        Behavior on color { ColorAnimation { duration: Theme.animFast } }

                        Text {
                            id: btnText
                            anchors.centerIn: parent
                            text: parent.actionLabel
                            color: Theme.text
                            font.pixelSize: 11
                            font.weight: Font.Bold
                        }

                        MouseArea {
                            id: btnArea
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                NotificationsBridge.invokeAction(
                                    NotificationsBridge.currentId,
                                    parent.actionKey);
                                root.visible = false;
                                hideTimer.stop();
                            }
                        }
                    }
                }
            }
        }

        // Click anywhere on the toast to dismiss early.
        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: {
                root.visible = false;
                hideTimer.stop();
            }
        }
    }
}
