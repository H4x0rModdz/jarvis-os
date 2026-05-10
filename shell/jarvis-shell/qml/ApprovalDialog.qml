import QtQuick
import QtQuick.Window
import QtQuick.Layouts
import Jarvis.Shell

/// Modal-ish approval prompt. Opens as a separate top-level window so it
/// can paint over the rest of the desktop without fighting the bar's
/// layer-shell anchor / exclusive zone. The host compositor handles
/// stacking — on labwc the window is centered automatically.
Window {
    id: root
    visible: PermissionBridge.hasPending
    width: 520
    height: 240
    title: qsTr("Permissão necessária")
    color: "transparent"
    flags: Qt.Dialog | Qt.WindowStaysOnTopHint

    // Re-center each time it opens.
    onVisibleChanged: {
        if (visible && Qt.application.screens.length > 0) {
            const s = Qt.application.screens[0];
            x = s.virtualX + Math.floor((s.width - width) / 2);
            y = s.virtualY + Math.floor((s.height - height) / 2);
        }
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
            anchors.margins: 22
            spacing: 12

            // ── Header chip with the scope ──
            Text {
                text: PermissionBridge.pendingScope
                color: Theme.accent
                font.pixelSize: 11
                font.weight: Font.Bold
                font.capitalization: Font.AllUppercase
            }

            Text {
                text: qsTr("Permissão necessária")
                color: Theme.text
                font.pixelSize: 20
                font.weight: Font.Bold
                Layout.fillWidth: true
            }

            // ── Body — explains who wants what ──
            Text {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: Theme.textDim
                font.pixelSize: 14
                textFormat: Text.RichText
                text: qsTr("<b style='color:%1'>%2</b> quer executar <b style='color:%1'>%3</b>.<br>Conceder permissão?")
                    .arg(Theme.text)
                    .arg(PermissionBridge.pendingCaller)
                    .arg(PermissionBridge.pendingAction)
            }

            Item { Layout.fillHeight: true }

            // ── Buttons ──
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 8

                ApprovalButton {
                    text: qsTr("Negar")
                    accent: Theme.danger
                    onClicked: PermissionBridge.deny()
                }
                ApprovalButton {
                    text: qsTr("Permitir uma vez")
                    onClicked: PermissionBridge.approveOnce()
                }
                ApprovalButton {
                    text: qsTr("Permitir sempre")
                    accent: Theme.accent
                    filled: true
                    onClicked: PermissionBridge.approvePersistent()
                }
            }
        }
    }
}
