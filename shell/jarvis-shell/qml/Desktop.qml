import QtQuick
import QtQuick.Window
import QtQuick.Layouts
import Jarvis.Shell

/// The desktop surface — a second top-level Window alongside Main.qml's bar,
/// recognised by `objectName` and anchored to all four output edges on the
/// wlr-layer-shell *bottom* layer (above swaybg's wallpaper, below every app
/// window). It takes no keyboard focus.
///
/// Hosts two things:
///   1. The desktop icon column (Computador / Pasta Pessoal / Lixeira).
///   2. An eDEX-style "command center" HUD — ambient SYSTEM (left) and
///      NETWORK (right) panels fed by SystemStatsBridge (/proc telemetry).
///      They live on the desktop behind windows, like the reference look.
///      This is a deliberate sci-fi *mode* aesthetic (its own cyan palette),
///      distinct from the calm glassmorphic chrome of the bar/popups.
Window {
    id: root
    objectName: "jarvis-desktop"
    visible: true
    color: "transparent"
    flags: Qt.FramelessWindowHint

    // ── HUD palette / type (local — not the purple glass Theme) ──────────
    readonly property color hudCyan: "#18ffff"
    readonly property color hudDim: "#4f8f8f"
    readonly property color hudPanel: Qt.rgba(0, 0, 0, 0.55)
    readonly property color hudBorder: Qt.rgba(0.094, 1.0, 1.0, 0.35)
    readonly property string mono: "monospace"
    readonly property int panelW: 300

    // "Online" = NetworkManager reports an active connection. activeConnection
    // is always a (possibly empty) map, so this is safe even pre-connect.
    readonly property bool netOnline: Object.keys(NetworkBridge.activeConnection).length > 0

    // Ticking clock for the SYSTEM panel.
    property var now: new Date()
    Timer { interval: 1000; running: true; repeat: true; onTriggered: root.now = new Date() }

    Component.onCompleted: {
        const s = Qt.application.screens[0];
        if (s) { width = s.width; height = s.height; }
    }

    // Clicking the empty desktop clears any icon selection.
    MouseArea {
        anchors.fill: parent
        onClicked: icons.selected = ""
    }

    // ── Desktop icons (shifted right so they clear the SYSTEM panel) ──────
    Column {
        id: icons
        x: root.panelW + 28
        y: 24
        spacing: 10

        property string selected: ""

        DesktopIcon {
            label: qsTr("Computador")
            iconName: "computer"
            selected: icons.selected === label
            onSelectRequested: icons.selected = label
            onActivated: ActionBusBridge.dispatch(
                "app.open", JSON.stringify({ "app": "/" }))
        }
        DesktopIcon {
            label: qsTr("Pasta Pessoal")
            iconName: "user-home"
            selected: icons.selected === label
            onSelectRequested: icons.selected = label
            onActivated: ActionBusBridge.dispatch(
                "app.open", JSON.stringify({ "app": HomePath }))
        }
        DesktopIcon {
            label: qsTr("Lixeira")
            iconName: "user-trash"
            selected: icons.selected === label
            onSelectRequested: icons.selected = label
            onActivated: ActionBusBridge.dispatch(
                "app.open", JSON.stringify({ "app": "trash:///" }))
        }
    }

    // ════════════════════════ SYSTEM panel (left) ════════════════════════
    Rectangle {
        id: sysPanel
        anchors.left: parent.left
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.margins: 14
        width: root.panelW
        color: root.hudPanel
        border.color: root.hudBorder
        border.width: 1
        radius: 2

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 16
            spacing: 12

            // Header
            RowLayout {
                Layout.fillWidth: true
                Text { text: "PANEL"; color: root.hudDim; font.family: root.mono; font.pixelSize: 10; font.letterSpacing: 2 }
                Item { Layout.fillWidth: true }
                Text { text: "SYSTEM"; color: root.hudCyan; font.family: root.mono; font.pixelSize: 10; font.letterSpacing: 2 }
            }

            // Clock
            Text {
                text: Qt.formatDateTime(root.now, "HH:mm:ss")
                color: root.hudCyan
                font.family: root.mono
                font.pixelSize: 40
                font.letterSpacing: 2
            }
            Text {
                text: Qt.formatDateTime(root.now, "yyyy MMM dd").toUpperCase()
                    + "   UP " + SystemStatsBridge.uptimeText
                    + "   TASKS " + SystemStatsBridge.taskCount
                color: root.hudDim
                font.family: root.mono
                font.pixelSize: 10
            }

            // CPU
            Text {
                Layout.topMargin: 4
                text: "CPU  " + SystemStatsBridge.cpuModel
                color: root.hudDim
                font.family: root.mono
                font.pixelSize: 9
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            RowLayout {
                Layout.fillWidth: true
                Text { text: "USAGE"; color: root.hudDim; font.family: root.mono; font.pixelSize: 10 }
                Item { Layout.fillWidth: true }
                Text { text: SystemStatsBridge.cpuPercent + "%"; color: root.hudCyan; font.family: root.mono; font.pixelSize: 12; font.bold: true }
            }
            HudGraph {
                Layout.fillWidth: true
                Layout.preferredHeight: 46
                values: SystemStatsBridge.cpuHistory
                maxValue: 100
                stroke: root.hudCyan
            }
            // Per-core bars
            GridLayout {
                Layout.fillWidth: true
                columns: 2
                rowSpacing: 3
                columnSpacing: 8
                Repeater {
                    model: SystemStatsBridge.perCore
                    delegate: RowLayout {
                        Layout.fillWidth: true
                        spacing: 6
                        Text { text: "#" + (index + 1); color: root.hudDim; font.family: root.mono; font.pixelSize: 9; Layout.preferredWidth: 22 }
                        Rectangle {
                            Layout.fillWidth: true
                            height: 6
                            color: Qt.rgba(1, 1, 1, 0.06)
                            Rectangle {
                                width: parent.width * (modelData / 100.0)
                                height: parent.height
                                color: root.hudCyan
                                Behavior on width { NumberAnimation { duration: 400; easing.type: Easing.OutCubic } }
                            }
                        }
                    }
                }
            }

            // Memory
            RowLayout {
                Layout.topMargin: 4
                Layout.fillWidth: true
                Text { text: "MEMORY"; color: root.hudDim; font.family: root.mono; font.pixelSize: 10 }
                Item { Layout.fillWidth: true }
                Text {
                    text: SystemStatsBridge.memUsedGiB.toFixed(1) + " / " + SystemStatsBridge.memTotalGiB.toFixed(1) + " GiB"
                    color: root.hudCyan; font.family: root.mono; font.pixelSize: 10
                }
            }
            Rectangle {
                Layout.fillWidth: true
                height: 8
                color: Qt.rgba(1, 1, 1, 0.06)
                Rectangle {
                    width: parent.width * (SystemStatsBridge.memPercent / 100.0)
                    height: parent.height
                    color: root.hudCyan
                    Behavior on width { NumberAnimation { duration: 400; easing.type: Easing.OutCubic } }
                }
            }
            Text {
                text: "SWAP  " + SystemStatsBridge.swapUsedGiB.toFixed(1) + " GiB"
                color: root.hudDim; font.family: root.mono; font.pixelSize: 9
            }

            // Top processes
            Text {
                Layout.topMargin: 4
                text: "TOP PROCESSES        PID  NAME        MEM"
                color: root.hudDim; font.family: root.mono; font.pixelSize: 9
            }
            Repeater {
                model: SystemStatsBridge.topProcesses
                delegate: Text {
                    Layout.fillWidth: true
                    text: ("" + modelData.pid).padStart(6) + "  "
                        + (modelData.name || "").padEnd(11).substring(0, 11) + " "
                        + modelData.mem.toFixed(1) + "%"
                    color: root.hudCyan
                    font.family: root.mono
                    font.pixelSize: 10
                    elide: Text.ElideRight
                }
            }

            Item { Layout.fillHeight: true } // push content up
        }
    }

    // ═══════════════════════ NETWORK panel (right) ═══════════════════════
    Rectangle {
        id: netPanel
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.margins: 14
        width: root.panelW
        color: root.hudPanel
        border.color: root.hudBorder
        border.width: 1
        radius: 2

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 16
            spacing: 12

            RowLayout {
                Layout.fillWidth: true
                Text { text: "PANEL"; color: root.hudDim; font.family: root.mono; font.pixelSize: 10; font.letterSpacing: 2 }
                Item { Layout.fillWidth: true }
                Text { text: "NETWORK"; color: root.hudCyan; font.family: root.mono; font.pixelSize: 10; font.letterSpacing: 2 }
            }

            Text { text: "NETWORK STATUS"; color: root.hudDim; font.family: root.mono; font.pixelSize: 10; font.letterSpacing: 1 }
            RowLayout {
                Layout.fillWidth: true
                Text {
                    text: root.netOnline ? "ONLINE" : "OFFLINE"
                    color: root.netOnline ? root.hudCyan : "#ff5a5a"
                    font.family: root.mono; font.pixelSize: 14; font.bold: true
                }
                Item { Layout.fillWidth: true }
            }

            Text {
                Layout.topMargin: 8
                text: "NETWORK TRAFFIC"
                color: root.hudDim; font.family: root.mono; font.pixelSize: 10; font.letterSpacing: 1
            }
            RowLayout {
                Layout.fillWidth: true
                Text {
                    text: "↑ " + SystemStatsBridge.netUpKBs.toFixed(1) + " KB/s"
                    color: root.hudCyan; font.family: root.mono; font.pixelSize: 11
                }
                Item { Layout.fillWidth: true }
                Text {
                    text: "↓ " + SystemStatsBridge.netDownKBs.toFixed(1) + " KB/s"
                    color: root.hudCyan; font.family: root.mono; font.pixelSize: 11
                }
            }
            HudGraph {
                Layout.fillWidth: true
                Layout.preferredHeight: 60
                values: SystemStatsBridge.netDownHistory
                maxValue: 0   // auto-scale
                stroke: root.hudCyan
                fill: true
            }
            HudGraph {
                Layout.fillWidth: true
                Layout.preferredHeight: 40
                values: SystemStatsBridge.netUpHistory
                maxValue: 0
                stroke: "#8ad0ff"
            }

            Item { Layout.fillHeight: true }
        }
    }
}
