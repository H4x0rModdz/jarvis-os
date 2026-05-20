import QtQuick
import Jarvis.Shell

/// Battery indicator on the bar. Battery glyph drawn from
/// Rectangles (no icon-font / SVG dep): a horizontal "body" with a
/// small terminal nub on the right, plus an inner fill bar whose
/// width tracks percentage. Charging state lights the fill with the
/// accent color and overlays a small "+" mark.
///
/// Auto-hidden when PowerBridge.hasBattery is false — desktops, VMs
/// without batteries, or boots before UPower decides on a device
/// don't get a confusing empty cell on the bar.
Item {
    id: root
    implicitWidth: visible ? 36 : 0
    implicitHeight: 32
    visible: PowerBridge.hasBattery

    /// Pull percentage into a clamped integer so the visual never
    /// goes <1 or >100 even if UPower glitches.
    readonly property int pct: Math.max(0, Math.min(100, Math.round(PowerBridge.percentage)))
    readonly property bool charging: PowerBridge.charging
    readonly property bool low: !charging && pct <= 15
    readonly property bool critical: !charging && pct <= 5

    Row {
        anchors.centerIn: parent
        spacing: 1

        // ── Battery body ──────────────────────────────────────────
        Rectangle {
            id: body
            width: 24
            height: 12
            radius: 2
            color: "transparent"
            border.color: critical
                ? Theme.danger
                : (low ? "#ffb547" : Theme.text)
            border.width: 1
            opacity: 0.9

            // Inner fill: width tracks pct.
            Rectangle {
                anchors.left: parent.left
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                anchors.margins: 2
                width: Math.max(1, (parent.width - 4) * root.pct / 100)
                radius: 1
                color: root.charging
                    ? Theme.accent
                    : (root.critical
                        ? Theme.danger
                        : (root.low ? "#ffb547" : Theme.text))
                Behavior on width {
                    NumberAnimation { duration: Theme.animFast }
                }
                Behavior on color {
                    ColorAnimation { duration: Theme.animFast }
                }
            }

            // Charging "+" — tiny vertical + horizontal bar pair
            // overlaid on the fill. Only when charging.
            Item {
                anchors.centerIn: parent
                width: 8
                height: 8
                visible: root.charging

                Rectangle {
                    anchors.centerIn: parent
                    width: 6
                    height: 2
                    color: Theme.background
                    radius: 1
                }
                Rectangle {
                    anchors.centerIn: parent
                    width: 2
                    height: 6
                    color: Theme.background
                    radius: 1
                }
            }
        }

        // ── Terminal nub on the right of the body ────────────────
        Rectangle {
            width: 2
            height: 6
            radius: 1
            anchors.verticalCenter: parent.verticalCenter
            color: critical
                ? Theme.danger
                : (low ? "#ffb547" : Theme.text)
            opacity: 0.9
        }
    }

    // Hover tooltip — minimal, just shows the percentage + state
    // textually so the user can confirm what the glyph means.
    MouseArea {
        id: hover
        anchors.fill: parent
        hoverEnabled: true
    }

    Rectangle {
        visible: hover.containsMouse
        anchors.bottom: parent.top
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottomMargin: 6
        implicitWidth: tooltip.implicitWidth + 16
        implicitHeight: tooltip.implicitHeight + 10
        radius: 6
        color: Theme.surfaceBright
        border.color: Theme.border
        border.width: 1
        z: 100

        Text {
            id: tooltip
            anchors.centerIn: parent
            text: {
                const label = root.charging
                    ? qsTr("Carregando")
                    : (PowerBridge.state === "full"
                        ? qsTr("Cheia")
                        : qsTr("Descarregando"));
                return root.pct + "% — " + label;
            }
            color: Theme.text
            font.pixelSize: 11
        }
    }
}
