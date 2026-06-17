import QtQuick
import QtQuick.Layouts
import QtQuick.Window
import Jarvis.Shell

/// History panel — sibling Window anchored to the right edge of the
/// primary output. Shows the daemon's RecentNotifications list,
/// newest first. Esc or click-outside closes.
///
/// V2 — read-only display. Future commits will add dismiss/clear
/// actions and grouping by app.
Window {
    id: root
    visible: false
    width: 360
    height: 640
    title: qsTr("Notificações")
    color: "transparent"
    // Qt.Dialog (not Qt.Tool): under labwc a Tool window is non-activatable,
    // so it never gets keyboard focus — the Escape Shortcut never fires.
    // Qt.Dialog gets the activation the Launcher/JarvisMenu rely on.
    flags: Qt.Dialog | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint

    // Suppress the spurious first deactivate wlroots fires between show and
    // the compositor granting focus — without it the drawer closes instantly.
    property bool _ignoreDeactivate: false
    Timer { id: armTimer; interval: 250; onTriggered: root._ignoreDeactivate = false }

    function requestOpen() {
        NotificationsBridge.refreshHistory();
        if (Qt.application.screens.length > 0) {
            const s = Qt.application.screens[0];
            x = s.virtualX + s.width - width - 16;
            y = s.virtualY + 16;
        }
        _ignoreDeactivate = true;
        visible = true;
        requestActivate();
        armTimer.restart();
    }

    // Esc closes.
    Shortcut {
        sequence: "Escape"
        onActivated: root.visible = false
    }

    // Close when focus leaves us.
    onActiveChanged: {
        if (!active && !_ignoreDeactivate && visible) root.visible = false;
    }

    GlassPanel {
        anchors.fill: parent
        anchors.margins: 8

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 20
            spacing: 12

            RowLayout {
                Layout.fillWidth: true
                Text {
                    text: qsTr("NOTIFICAÇÕES")
                    color: Theme.accent
                    font.pixelSize: 11
                    font.weight: Font.Bold
                    font.letterSpacing: 2
                    Layout.fillWidth: true
                }
                // "Clear all" — only meaningful when there's history.
                // Daemon's HistoryChanged signal repaints the list.
                Item {
                    visible: NotificationsBridge.history.length > 0
                    implicitWidth: clearLabel.implicitWidth + 16
                    implicitHeight: 22

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
                        text: qsTr("LIMPAR TUDO")
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
                        onClicked: NotificationsBridge.clear()
                    }
                }
            }

            Text {
                visible: NotificationsBridge.history.length === 0
                text: qsTr("Nenhuma notificação recente.")
                color: Theme.textDim
                font.pixelSize: 13
                Layout.fillWidth: true
            }

            ListView {
                visible: NotificationsBridge.history.length > 0
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 10
                model: NotificationsBridge.history

                delegate: Rectangle {
                    width: ListView.view.width
                    implicitHeight: rowCol.implicitHeight + 16
                    radius: 8
                    color: Qt.rgba(1, 1, 1, 0.04)
                    border.color: {
                        switch (modelData.urgency) {
                            case "critical": return Theme.danger;
                            case "low":      return Theme.border;
                            default:         return Theme.accent;
                        }
                    }
                    border.width: 1

                    ColumnLayout {
                        id: rowCol
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: 8
                        // Leave room on the right for the dismiss button so
                        // long summaries don't slide under it.
                        anchors.rightMargin: 28
                        spacing: 2

                        Text {
                            text: (modelData.app || "Jarvis").toUpperCase()
                            color: Theme.accent
                            font.pixelSize: 9
                            font.weight: Font.Bold
                            font.letterSpacing: 1
                        }
                        Text {
                            visible: (modelData.summary || "").length > 0
                            text: modelData.summary
                            color: Theme.text
                            font.pixelSize: 13
                            font.weight: Font.Bold
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                        }
                        Text {
                            visible: (modelData.body || "").length > 0
                            text: modelData.body
                            color: Theme.textDim
                            font.pixelSize: 12
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                        }
                    }

                    // Per-row dismiss. Daemon-side Dismiss(id) removes
                    // the entry and emits HistoryChanged so the list
                    // repaints without an explicit refresh.
                    Item {
                        id: dismissBtn
                        anchors.top: parent.top
                        anchors.right: parent.right
                        anchors.margins: 4
                        width: 20
                        height: 20

                        Text {
                            anchors.centerIn: parent
                            text: "×"
                            color: dismissArea.containsMouse
                                ? Theme.text
                                : Theme.textDim
                            font.pixelSize: 18
                            font.weight: Font.Bold
                        }
                        MouseArea {
                            id: dismissArea
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: NotificationsBridge.dismiss(modelData.id)
                        }
                    }
                }
            }
        }
    }
}
