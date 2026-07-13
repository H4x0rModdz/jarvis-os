import QtQuick
import QtQuick.Window
import QtQuick.Layouts
import Jarvis.Shell

/// The desktop surface — a second top-level Window alongside Main.qml's bar,
/// recognised by `objectName` and anchored to all four output edges on the
/// wlr-layer-shell *bottom* layer (above swaybg's wallpaper, below every app
/// window). It takes no keyboard focus.
///
/// Hosts the eDEX-style "command center" HUD — SYSTEM (left), LILITH (center)
/// and NETWORK (right) panels, fed by SystemStatsBridge (/proc), the Lilith
/// bridges, and MediaBridge (playerctl now-playing). Lives behind app windows;
/// a deliberate sci-fi *mode* aesthetic (its own cyan palette), distinct from
/// the calm glass chrome of the bar/popups. No desktop icons — the HUD owns the
/// home; app access is the launcher/dock.
Window {
    id: root
    objectName: "jarvis-desktop"
    visible: true
    color: "transparent"
    flags: Qt.FramelessWindowHint

    // ── HUD palette / type (local — not the purple glass Theme) ──────────
    readonly property color hudCyan: "#18ffff"
    readonly property color hudDim: "#67b0b0"
    // Dark, mostly-opaque backing. The wallpaper is a separate Wayland surface
    // *below* this one, so there's nothing to blur through — legibility comes
    // from an opaque-enough fill. 0.55 let a busy wallpaper wash the text (and
    // the avatar) out; ~0.85 over near-black keeps the eDEX look but readable.
    readonly property color hudPanel: Qt.rgba(0.02, 0.05, 0.07, 0.85)
    readonly property color hudBorder: Qt.rgba(0.094, 1.0, 1.0, 0.35)
    readonly property string mono: "monospace"
    readonly property int panelW: 300

    // "Online" = a default route exists (wired or wifi). SystemStatsBridge reads
    // /proc/net/route; the old NetworkBridge.activeConnection check was WiFi-only,
    // so a wired VM always showed OFFLINE.
    readonly property bool netOnline: SystemStatsBridge.online

    // Lilith's state for the center panel (same priority as the dock orb).
    readonly property string lilithState: {
        const v = VoiceBridge.state;
        if (v === "listening") return "listening";
        if (v === "speaking")  return "speaking";
        if (LilithBridge.busy || v === "processing") return "thinking";
        return "idle";
    }

    // Ticking clock for the SYSTEM panel.
    property var now: new Date()
    Timer { interval: 1000; running: true; repeat: true; onTriggered: root.now = new Date() }

    Component.onCompleted: {
        const s = Qt.application.screens[0];
        if (s) { width = s.width; height = s.height; }
    }

    // Desktop icons removed — the HUD owns the home now (ADR 0031). Computador /
    // Pasta Pessoal / Lixeira are reachable from the launcher + file manager.

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
            // Booted OS build + OTA status (UpdaterBridge). Informational — the
            // full install flow lives in the updater splash.
            Text {
                Layout.fillWidth: true
                text: SystemStatsBridge.osRelease
                color: root.hudDim; font.family: root.mono; font.pixelSize: 9
                elide: Text.ElideRight
            }
            Text {
                Layout.fillWidth: true
                visible: text.length > 0
                text: {
                    if (UpdaterBridge.requiresReboot) return "● REINICIE PARA APLICAR";
                    if (UpdaterBridge.active && UpdaterBridge.stage === "os.upgrade")
                        return "● ATUALIZANDO " + (UpdaterBridge.percent >= 0 ? UpdaterBridge.percent + "%" : "…");
                    if (UpdaterBridge.osUpdateAvailable) return "● ATUALIZAÇÃO DISPONÍVEL";
                    return "";
                }
                color: root.hudCyan; font.family: root.mono; font.pixelSize: 9; font.letterSpacing: 1
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

            // Disk — root ("/") and home ("~"). Critical on a bootc OS whose OTA
            // images are large (an update can fill /); home fills with user data.
            RowLayout {
                Layout.topMargin: 4
                Layout.fillWidth: true
                Text { text: "DISK /"; color: root.hudDim; font.family: root.mono; font.pixelSize: 10 }
                Item { Layout.fillWidth: true }
                Text {
                    text: SystemStatsBridge.diskUsedGiB.toFixed(0) + " / " + SystemStatsBridge.diskTotalGiB.toFixed(0) + " GiB"
                    color: root.hudCyan; font.family: root.mono; font.pixelSize: 10
                }
            }
            Rectangle {
                Layout.fillWidth: true; height: 8; color: Qt.rgba(1, 1, 1, 0.06)
                Rectangle {
                    width: parent.width * (SystemStatsBridge.diskPercent / 100.0)
                    height: parent.height
                    color: SystemStatsBridge.diskPercent >= 90 ? "#ff5a5a" : root.hudCyan
                    Behavior on width { NumberAnimation { duration: 400; easing.type: Easing.OutCubic } }
                }
            }
            RowLayout {
                Layout.fillWidth: true
                visible: SystemStatsBridge.homeTotalGiB > 0
                Text { text: "DISK ~"; color: root.hudDim; font.family: root.mono; font.pixelSize: 10 }
                Item { Layout.fillWidth: true }
                Text {
                    text: SystemStatsBridge.homeUsedGiB.toFixed(0) + " / " + SystemStatsBridge.homeTotalGiB.toFixed(0) + " GiB"
                    color: root.hudCyan; font.family: root.mono; font.pixelSize: 10
                }
            }
            Rectangle {
                Layout.fillWidth: true; height: 8; color: Qt.rgba(1, 1, 1, 0.06)
                visible: SystemStatsBridge.homeTotalGiB > 0
                Rectangle {
                    width: parent.width * (SystemStatsBridge.homePercent / 100.0)
                    height: parent.height
                    color: SystemStatsBridge.homePercent >= 90 ? "#ff5a5a" : root.hudCyan
                    Behavior on width { NumberAnimation { duration: 400; easing.type: Easing.OutCubic } }
                }
            }
            // CPU temperature — hidden in a VM with no readable sensor.
            Text {
                visible: SystemStatsBridge.cpuTempC > 0
                text: "CPU TEMP  " + SystemStatsBridge.cpuTempC + "°C"
                color: SystemStatsBridge.cpuTempC >= 85 ? "#ff5a5a" : root.hudDim
                font.family: root.mono; font.pixelSize: 9
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

            // Interface identity — default-route iface + its IP/gateway (wired or
            // wifi; SystemStatsBridge, not the WiFi-only NetworkBridge). SSID is a
            // bonus when on wifi.
            ColumnLayout {
                Layout.fillWidth: true
                Layout.topMargin: 6
                spacing: 2
                visible: root.netOnline && SystemStatsBridge.ipAddress.length > 0
                RowLayout {
                    Layout.fillWidth: true
                    Text { text: "IFACE"; color: root.hudDim; font.family: root.mono; font.pixelSize: 10 }
                    Item { Layout.fillWidth: true }
                    Text { text: SystemStatsBridge.primaryIface; color: root.hudCyan; font.family: root.mono; font.pixelSize: 10 }
                }
                RowLayout {
                    Layout.fillWidth: true
                    Text { text: "IP"; color: root.hudDim; font.family: root.mono; font.pixelSize: 10 }
                    Item { Layout.fillWidth: true }
                    Text { text: SystemStatsBridge.ipAddress; color: root.hudCyan; font.family: root.mono; font.pixelSize: 10 }
                }
                RowLayout {
                    Layout.fillWidth: true
                    visible: SystemStatsBridge.gateway.length > 0
                    Text { text: "GATEWAY"; color: root.hudDim; font.family: root.mono; font.pixelSize: 10 }
                    Item { Layout.fillWidth: true }
                    Text { text: SystemStatsBridge.gateway; color: root.hudCyan; font.family: root.mono; font.pixelSize: 10 }
                }
                RowLayout {
                    Layout.fillWidth: true
                    visible: !!(NetworkBridge.activeConnection && NetworkBridge.activeConnection.ssid)
                    Text { text: "SSID"; color: root.hudDim; font.family: root.mono; font.pixelSize: 10 }
                    Item { Layout.fillWidth: true }
                    Text {
                        Layout.maximumWidth: 150
                        text: NetworkBridge.activeConnection ? (NetworkBridge.activeConnection.ssid || "") : ""
                        color: root.hudCyan; font.family: root.mono; font.pixelSize: 10; elide: Text.ElideRight
                    }
                }
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

            Item { Layout.fillHeight: true }   // pushes the media widget to the bottom

            // ── Now playing (MediaBridge → playerctl) ─────────────────────
            Rectangle { Layout.fillWidth: true; height: 1; color: root.hudBorder }
            Text {
                text: "NOW PLAYING"
                color: root.hudDim; font.family: root.mono; font.pixelSize: 9; font.letterSpacing: 1
            }
            Text {
                Layout.fillWidth: true
                text: MediaBridge.hasPlayer ? (MediaBridge.title.length > 0 ? MediaBridge.title : "—")
                                            : "Nada tocando"
                color: root.hudCyan; font.family: root.mono; font.pixelSize: 11
                elide: Text.ElideRight
            }
            Text {
                Layout.fillWidth: true
                visible: MediaBridge.hasPlayer && MediaBridge.artist.length > 0
                text: MediaBridge.artist
                color: root.hudDim; font.family: root.mono; font.pixelSize: 9
                elide: Text.ElideRight
            }
            RowLayout {
                Layout.fillWidth: true
                Layout.topMargin: 4
                spacing: 22
                Item { Layout.fillWidth: true }
                Text {
                    text: "⏮"
                    color: prevMa.containsMouse ? root.hudCyan : root.hudDim
                    font.pixelSize: 18
                    MouseArea {
                        id: prevMa; anchors.fill: parent; anchors.margins: -8
                        hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                        onClicked: MediaBridge.previous()
                    }
                }
                Text {
                    text: MediaBridge.status === "Playing" ? "⏸" : "▶"
                    color: playMa.containsMouse ? root.hudCyan : root.hudDim
                    font.pixelSize: 20
                    MouseArea {
                        id: playMa; anchors.fill: parent; anchors.margins: -8
                        hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                        onClicked: MediaBridge.playPause()
                    }
                }
                Text {
                    text: "⏭"
                    color: nextMa.containsMouse ? root.hudCyan : root.hudDim
                    font.pixelSize: 18
                    MouseArea {
                        id: nextMa; anchors.fill: parent; anchors.margins: -8
                        hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                        onClicked: MediaBridge.next()
                    }
                }
                Item { Layout.fillWidth: true }
            }
        }
    }

    // ═══════════════════════ LILITH center (Phase 2) ═════════════════════
    // The command-center soul (ADR 0031): the 3D avatar + a live feed of what
    // Lilith is doing (her replies + the tools she ran). The avatar loads via a
    // Loader so a missing QtQuick3D leaves just the feed — the HUD never breaks.
    // Read-only: the desktop takes no keyboard focus, so typing stays in the orb.
    Rectangle {
        id: lilithPanel
        anchors.left: sysPanel.right
        anchors.right: netPanel.left
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.leftMargin: 16
        anchors.rightMargin: 16
        anchors.topMargin: 14
        anchors.bottomMargin: 14
        color: root.hudPanel
        border.color: root.hudBorder
        border.width: 1
        radius: 2

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 16
            spacing: 10

            RowLayout {
                Layout.fillWidth: true
                Text { text: "PANEL"; color: root.hudDim; font.family: root.mono; font.pixelSize: 10; font.letterSpacing: 2 }
                Item { Layout.fillWidth: true }
                Text { text: "LILITH"; color: root.hudCyan; font.family: root.mono; font.pixelSize: 10; font.letterSpacing: 2 }
            }

            // Avatar — isolated so a missing QtQuick3D doesn't take the HUD down.
            Loader {
                id: avatarLoader
                Layout.fillWidth: true
                Layout.preferredHeight: 320
                asynchronous: true
                source: Qt.resolvedUrl("LilithAvatarView.qml")
                onStatusChanged: {
                    if (status === Loader.Error)
                        console.warn("HUD avatar: QtQuick3D unavailable — feed only");
                }
            }

            Text {
                Layout.alignment: Qt.AlignHCenter
                text: {
                    switch (root.lilithState) {
                    case "listening": return "OUVINDO";
                    case "thinking":  return "PROCESSANDO";
                    case "speaking":  return "FALANDO";
                    default:          return LilithBridge.reachable ? "ONLINE" : "OFFLINE";
                    }
                }
                color: root.hudCyan
                font.family: root.mono
                font.pixelSize: 11
                font.letterSpacing: 2
            }

            // Proactive suggestion (LilithBridge — same nudge the orb popup shows).
            // Persists in the HUD until dismissed.
            Rectangle {
                Layout.fillWidth: true
                visible: LilithBridge.proactiveNudgeText.length > 0
                Layout.preferredHeight: nudgeCol.implicitHeight + 16
                color: Qt.rgba(0.094, 1.0, 1.0, 0.08)
                border.color: root.hudBorder; border.width: 1; radius: 2
                ColumnLayout {
                    id: nudgeCol
                    anchors.fill: parent; anchors.margins: 8; spacing: 2
                    RowLayout {
                        Layout.fillWidth: true
                        Text {
                            text: "◈ SUGESTÃO"
                            color: LilithBridge.proactiveNudgeUrgency === "critical" ? "#ff5a5a" : root.hudCyan
                            font.family: root.mono; font.pixelSize: 9; font.letterSpacing: 1
                        }
                        Item { Layout.fillWidth: true }
                        Text {
                            text: "✕"
                            color: nudgeDismiss.containsMouse ? root.hudCyan : root.hudDim
                            font.pixelSize: 12
                            MouseArea {
                                id: nudgeDismiss; anchors.fill: parent; anchors.margins: -6
                                hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                                onClicked: LilithBridge.dismissProactiveNudge()
                            }
                        }
                    }
                    Text {
                        Layout.fillWidth: true
                        text: LilithBridge.proactiveNudgeText
                        color: root.hudCyan; font.family: root.mono; font.pixelSize: 10; wrapMode: Text.WordWrap
                    }
                }
            }

            // A dangerous action awaiting your approval (PermissionBridge). The
            // full dialog also pops from Main.qml; approving/denying from either
            // resolves the same request.
            Rectangle {
                Layout.fillWidth: true
                visible: PermissionBridge.hasPending
                Layout.preferredHeight: confCol.implicitHeight + 16
                color: Qt.rgba(1.0, 0.35, 0.35, 0.10)
                border.color: "#ff5a5a"; border.width: 1; radius: 2
                ColumnLayout {
                    id: confCol
                    anchors.fill: parent; anchors.margins: 8; spacing: 2
                    Text {
                        text: "⚠ CONFIRMAÇÃO PENDENTE"
                        color: "#ff5a5a"; font.family: root.mono; font.pixelSize: 9; font.letterSpacing: 1
                    }
                    Text {
                        Layout.fillWidth: true
                        text: PermissionBridge.pendingAction + "  (" + PermissionBridge.pendingCaller + ")"
                        color: root.hudCyan; font.family: root.mono; font.pixelSize: 10; wrapMode: Text.WordWrap
                    }
                    RowLayout {
                        Layout.fillWidth: true; Layout.topMargin: 2; spacing: 12
                        Text {
                            text: "APROVAR"
                            color: approveMa.containsMouse ? root.hudCyan : root.hudDim
                            font.family: root.mono; font.pixelSize: 10; font.bold: true
                            MouseArea {
                                id: approveMa; anchors.fill: parent; anchors.margins: -6
                                hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                                onClicked: PermissionBridge.approveOnce()
                            }
                        }
                        Text {
                            text: "NEGAR"
                            color: denyMa.containsMouse ? "#ff5a5a" : root.hudDim
                            font.family: root.mono; font.pixelSize: 10; font.bold: true
                            MouseArea {
                                id: denyMa; anchors.fill: parent; anchors.margins: -6
                                hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                                onClicked: PermissionBridge.deny()
                            }
                        }
                        Item { Layout.fillWidth: true }
                    }
                }
            }

            Rectangle { Layout.fillWidth: true; height: 1; color: root.hudBorder }

            Text {
                text: "ACTIVITY FEED"
                color: root.hudDim; font.family: root.mono; font.pixelSize: 9; font.letterSpacing: 1
            }

            // Scrolling log built from the conversation: user lines, the tools
            // Lilith ran, and her replies.
            ListView {
                id: feed
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 6
                model: LilithBridge.conversation
                onCountChanged: positionViewAtEnd()
                Component.onCompleted: positionViewAtEnd()

                delegate: Column {
                    width: ListView.view.width
                    spacing: 2

                    Text {
                        width: parent.width
                        visible: modelData.role === "user"
                        text: "> " + (modelData.text || "")
                        color: root.hudDim
                        font.family: root.mono
                        font.pixelSize: 10
                        wrapMode: Text.WordWrap
                    }
                    Text {
                        width: parent.width
                        visible: modelData.role === "lilith"
                            && modelData.chainSteps !== undefined
                            && modelData.chainSteps.length > 0
                        text: (modelData.chainSteps || [])
                            .map(function (s) { return "→ " + (s.action || ""); }).join("  ")
                        color: "#8ad0ff"
                        font.family: root.mono
                        font.pixelSize: 9
                        wrapMode: Text.WordWrap
                    }
                    Text {
                        width: parent.width
                        visible: modelData.role === "lilith"
                        text: "λ " + (modelData.text || "")
                        color: root.hudCyan
                        font.family: root.mono
                        font.pixelSize: 10
                        wrapMode: Text.WordWrap
                    }
                }
            }

            // Live in-flight line while she's working.
            Text {
                Layout.fillWidth: true
                visible: LilithBridge.busy || LilithBridge.streamingText.length > 0
                text: "λ " + (LilithBridge.streamingText.length > 0 ? LilithBridge.streamingText : "…")
                color: root.hudCyan
                font.family: root.mono
                font.pixelSize: 10
                wrapMode: Text.WordWrap
            }
        }
    }
}
