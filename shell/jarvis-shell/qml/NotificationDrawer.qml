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
    flags: Qt.Tool | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint

    function requestOpen() {
        NotificationsBridge.refreshHistory();
        if (Qt.application.screens.length > 0) {
            const s = Qt.application.screens[0];
            x = s.virtualX + s.width - width - 16;
            y = s.virtualY + 16;
        }
        visible = true;
        requestActivate();
    }

    // Esc closes.
    Shortcut {
        sequence: "Escape"
        onActivated: root.visible = false
    }

    // Close when focus leaves us.
    onActiveChanged: {
        if (!active && visible) root.visible = false;
    }

    Rectangle {
        anchors.fill: parent
        anchors.margins: 8
        radius: Theme.radius
        color: Theme.surfaceBright
        border.color: Theme.border
        border.width: 1

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 20
            spacing: 12

            Text {
                text: qsTr("NOTIFICAÇÕES")
                color: Theme.accent
                font.pixelSize: 11
                font.weight: Font.Bold
                font.letterSpacing: 2
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
                        anchors.fill: parent
                        anchors.margins: 8
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
                }
            }
        }
    }
}
