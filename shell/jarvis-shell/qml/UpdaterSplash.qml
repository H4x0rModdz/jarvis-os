import QtQuick
import QtQuick.Window
import QtQuick.Layouts
import Jarvis.Shell

/// First-boot splash. Bound to UpdaterBridge.active — appears the moment
/// the daemon starts emitting Progress, dismisses on Completed(success).
/// On failure it stays up so the user sees what went wrong.
///
/// Sized to take a comfortable chunk of the screen without going full
/// edge-to-edge — the user is meant to feel that the OS is *setting itself
/// up*, not that something is broken.
Window {
    id: root
    visible: UpdaterBridge.active
    width: 560
    height: 320
    title: qsTr("Preparando o Jarvis OS")
    color: "transparent"
    flags: Qt.Dialog | Qt.WindowStaysOnTopHint

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
            anchors.margins: 28
            spacing: 14

            // ── Header chip ──
            Text {
                text: qsTr("CONFIGURAÇÃO INICIAL")
                color: Theme.accent
                font.pixelSize: 11
                font.weight: Font.Bold
            }

            Text {
                text: UpdaterBridge.failed
                      ? qsTr("Falha ao preparar o sistema")
                      : qsTr("Preparando o Jarvis OS")
                color: Theme.text
                font.pixelSize: 22
                font.weight: Font.Bold
                Layout.fillWidth: true
            }

            Text {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: Theme.textDim
                font.pixelSize: 14
                text: UpdaterBridge.failed
                    ? UpdaterBridge.message
                    : qsTr("Baixando os pacotes que a Lilith precisa para responder. " +
                           "Isso só acontece no primeiro boot — depois fica tudo offline.")
            }

            Item { Layout.fillHeight: true }

            // ── Stage line ──
            Text {
                Layout.fillWidth: true
                text: UpdaterBridge.stage.length > 0
                    ? UpdaterBridge.stage + " · " + UpdaterBridge.message
                    : UpdaterBridge.message
                color: Theme.textDim
                font.pixelSize: 12
                elide: Text.ElideRight
            }

            // ── Progress bar ──
            Item {
                Layout.fillWidth: true
                Layout.preferredHeight: 8

                Rectangle {
                    anchors.fill: parent
                    radius: 4
                    color: Theme.border
                }

                // Determinate path.
                Rectangle {
                    visible: UpdaterBridge.percent >= 0 && !UpdaterBridge.failed
                    radius: 4
                    color: Theme.accent
                    height: parent.height
                    width: parent.width * Math.max(0, Math.min(100, UpdaterBridge.percent)) / 100

                    Behavior on width {
                        NumberAnimation {
                            duration: Theme.animFast
                            easing.type: Easing.OutCubic
                        }
                    }
                }

                // Indeterminate path — a small marker sliding back and forth.
                Rectangle {
                    visible: UpdaterBridge.percent < 0 && !UpdaterBridge.failed
                    radius: 4
                    color: Theme.accent
                    height: parent.height
                    width: parent.width * 0.25

                    SequentialAnimation on x {
                        loops: Animation.Infinite
                        running: parent.visible
                        NumberAnimation {
                            from: 0
                            to: parent.parent.width - parent.width
                            duration: 1200
                            easing.type: Easing.InOutQuad
                        }
                        NumberAnimation {
                            to: 0
                            duration: 1200
                            easing.type: Easing.InOutQuad
                        }
                    }
                }

                // Failure state.
                Rectangle {
                    visible: UpdaterBridge.failed
                    anchors.fill: parent
                    radius: 4
                    color: Theme.danger
                    opacity: 0.6
                }
            }

            // Percent number on the right.
            Text {
                Layout.alignment: Qt.AlignRight
                text: UpdaterBridge.percent >= 0
                    ? UpdaterBridge.percent + "%"
                    : "…"
                color: Theme.text
                font.pixelSize: 12
                font.weight: Font.Bold
            }
        }
    }
}
