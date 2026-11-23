import QtQuick
import QtQuick.Layouts
import Jarvis.Shell

/// The menu bar. Transparent — the wallpaper runs straight up to the top of
/// the screen and the bar is only text and glyphs over it, with a faint
/// darkening so white stays legible on a bright wallpaper.
///
/// Left: the system mark (opens the system menu) and the name of the focused
/// application. Right: status items, then date and time.
///
/// There are no per-application menus (File, Edit, …): Linux apps do not
/// export a global menu to the shell, and decorative menus that do nothing
/// would be worse than none.
Item {
    id: root

    signal jarvisMenuRequested()
    signal lilithRequested()
    signal launcherRequested()
    signal networksRequested()
    signal bluetoothRequested()
    signal notificationsRequested()
    signal settingsRequested()

    /// Which popup is open, so its bar item stays highlighted:
    /// "" | "system" | "wifi" | "bluetooth" | "settings" | "notifications".
    property string openPopup: ""

    RunningAppsModel { id: running }

    // Minute resolution: no seconds on the bar, so the bar surface repaints
    // once a minute instead of once a second. The first tick lands on the
    // minute boundary; after that it is every 60 s.
    property var now: new Date()
    Timer {
        interval: 1000 * (60 - root.now.getSeconds())
        running: true
        repeat: true
        onTriggered: { root.now = new Date(); interval = 60000; }
    }

    /// "org.mozilla.firefox" → "Firefox"; nothing focused → the desktop.
    readonly property string activeAppName: {
        const id = running.activeAppId;
        if (!id || id.length === 0) return qsTr("Mesa");
        const seg = id.substring(id.lastIndexOf(".") + 1);
        return seg.charAt(0).toUpperCase() + seg.substring(1);
    }

    // Legibility scrim: darkest at the very top, gone by the bottom edge.
    Rectangle {
        anchors.fill: parent
        gradient: Gradient {
            GradientStop { position: 0.0; color: Qt.rgba(0, 0, 0, 0.26) }
            GradientStop { position: 1.0; color: Qt.rgba(0, 0, 0, 0.06) }
        }
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 6
        anchors.rightMargin: 8
        spacing: 0

        // ── System mark ────────────────────────────────────────────────
        BarItem {
            active: root.openPopup === "system"
            horizontalPadding: 9
            onClicked: root.jarvisMenuRequested()
            // The Lilith mark: a crescent moon.
            SymbolIcon {
                anchors.verticalCenter: parent.verticalCenter
                size: 16
                name: "weather-clear-night-symbolic"
                fallback: "☾"
            }
        }

        // ── Focused application ────────────────────────────────────────
        BarItem {
            horizontalPadding: 8
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: root.activeAppName
                color: Theme.text
                font.pixelSize: Theme.fontBody
                font.weight: Font.Bold
            }
        }

        Item { Layout.fillWidth: true }

        // ── Status items ───────────────────────────────────────────────
        BarItem {
            onClicked: root.lilithRequested()
            // Lilith: a small luminous orb, the same mark as in the dock.
            Rectangle {
                width: 14; height: 14; radius: 7
                anchors.verticalCenter: parent.verticalCenter
                gradient: Gradient {
                    GradientStop { position: 0.0; color: "#64d2ff" }
                    GradientStop { position: 0.55; color: "#5e5ce6" }
                    GradientStop { position: 1.0; color: "#bf5af2" }
                }
                opacity: LilithBridge.reachable ? 1.0 : 0.4
            }
        }

        BarItem {
            visible: PowerBridge.hasBattery
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: Math.round(PowerBridge.percentage) + "%"
                color: Theme.text
                font.pixelSize: Theme.fontCaption + 1
            }
            SymbolIcon {
                anchors.verticalCenter: parent.verticalCenter
                size: 20
                name: {
                    const lvl = Math.min(100, Math.max(0, Math.round(PowerBridge.percentage / 10) * 10));
                    return "battery-level-" + lvl + (PowerBridge.charging ? "-charging" : "") + "-symbolic";
                }
                fallback: "▭"
            }
        }

        BarItem {
            active: root.openPopup === "bluetooth"
            visible: BluetoothBridge.poweredOn
            onClicked: root.bluetoothRequested()
            SymbolIcon {
                anchors.verticalCenter: parent.verticalCenter
                name: "bluetooth-active-symbolic"
                fallback: "ᛒ"
            }
        }

        BarItem {
            active: root.openPopup === "wifi"
            onClicked: root.networksRequested()
            SymbolIcon {
                anchors.verticalCenter: parent.verticalCenter
                name: {
                    if (!NetworkBridge.wifiEnabled) {
                        // No Wi-Fi radio in use: show the wired state instead.
                        return SystemStatsBridge.online ? "network-wired-symbolic"
                                                        : "network-offline-symbolic";
                    }
                    const sig = NetworkBridge.activeConnection.signal;
                    if (NetworkBridge.activeConnection.ssid === undefined)
                        return "network-wireless-offline-symbolic";
                    if (sig >= 75) return "network-wireless-signal-excellent-symbolic";
                    if (sig >= 50) return "network-wireless-signal-good-symbolic";
                    if (sig >= 25) return "network-wireless-signal-ok-symbolic";
                    return "network-wireless-signal-weak-symbolic";
                }
                fallback: "◠"
            }
        }

        BarItem {
            onClicked: root.launcherRequested()
            SymbolIcon {
                anchors.verticalCenter: parent.verticalCenter
                name: "system-search-symbolic"
                fallback: "⌕"
            }
        }

        // Control Centre: two stacked toggles.
        BarItem {
            active: root.openPopup === "settings"
            onClicked: root.settingsRequested()
            Item {
                width: 16; height: 14
                anchors.verticalCenter: parent.verticalCenter
                Repeater {
                    model: 2
                    Rectangle {
                        y: index * 8
                        width: 16; height: 6; radius: 3
                        color: "transparent"
                        border.color: Theme.text
                        border.width: 1.3
                        Rectangle {
                            width: 6; height: 6; radius: 3
                            x: index === 0 ? parent.width - width : 0
                            color: Theme.text
                        }
                    }
                }
            }
        }

        // ── Date + time (opens Notification Centre) ────────────────────
        BarItem {
            id: clock
            active: root.openPopup === "notifications"
            horizontalPadding: 8
            onClicked: root.notificationsRequested()

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: Qt.formatDateTime(root.now, "ddd d MMM").replace(/\./g, "")
                      + "   " + Qt.formatDateTime(root.now, "HH:mm")
                color: Theme.text
                font.pixelSize: Theme.fontBody
                font.weight: Font.Medium
            }
        }
    }
}
