import QtQuick
import QtQuick.Layouts
import QtQuick.Window
import Jarvis.Shell

/// Updater splash. Three modes, derived from UpdaterBridge state:
///
///   1. **Active operation** — UpdaterBridge.active. Renders a progress
///      bar for model.pull / os.upgrade. Same UX as Phase 1.
///   2. **OS update prompt** — UpdaterBridge.osUpdateAvailable. Shows an
///      "install now" CTA. Non-blocking — the user can dismiss and
///      apply later, but the splash sits on top so it's hard to miss.
///   3. **Reboot prompt** — UpdaterBridge.requiresReboot. Shown right
///      after a successful OS upgrade completes. "Restart now" is the
///      primary action; the user can defer.
Window {
    id: root
    visible: UpdaterBridge.active
             || UpdaterBridge.osUpdateAvailable
             || UpdaterBridge.requiresReboot
    width: 560
    height: 340
    title: qsTr("Jarvis OS")
    color: "transparent"
    flags: Qt.Dialog | Qt.WindowStaysOnTopHint

    onVisibleChanged: {
        if (visible && Qt.application.screens.length > 0) {
            const s = Qt.application.screens[0];
            x = s.virtualX + Math.floor((s.width - width) / 2);
            y = s.virtualY + Math.floor((s.height - height) / 2);
        }
    }

    // Pick which of the three modes is in front. `requiresReboot` wins
    // over `osUpdateAvailable` so the post-upgrade UI doesn't get
    // confused with a fresh prompt.
    readonly property string mode: {
        if (UpdaterBridge.requiresReboot) return "reboot";
        if (UpdaterBridge.active) return "active";
        if (UpdaterBridge.osUpdateAvailable) return "os-prompt";
        return "idle";
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
                text: {
                    switch (root.mode) {
                        case "os-prompt": return qsTr("ATUALIZAÇÃO DISPONÍVEL");
                        case "reboot":    return qsTr("REINICIALIZAÇÃO PENDENTE");
                        case "active":    return UpdaterBridge.stage === "os.upgrade"
                                              ? qsTr("INSTALANDO ATUALIZAÇÃO")
                                              : qsTr("CONFIGURAÇÃO INICIAL");
                        default:          return qsTr("CONFIGURAÇÃO INICIAL");
                    }
                }
                color: Theme.accent
                font.pixelSize: 11
                font.weight: Font.Bold
            }

            // ── Title ──
            Text {
                text: {
                    if (UpdaterBridge.failed) return qsTr("Falha ao preparar o sistema");
                    switch (root.mode) {
                        case "os-prompt": return qsTr("Atualização do sistema disponível");
                        case "reboot":    return qsTr("Reinicie para concluir");
                        case "active":    return UpdaterBridge.stage === "os.upgrade"
                                              ? qsTr("Instalando atualização do sistema")
                                              : qsTr("Preparando o Jarvis OS");
                        default:          return qsTr("Preparando o Jarvis OS");
                    }
                }
                color: Theme.text
                font.pixelSize: 22
                font.weight: Font.Bold
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
            }

            // ── Body ──
            Text {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: Theme.textDim
                font.pixelSize: 14
                text: {
                    if (UpdaterBridge.failed) return UpdaterBridge.message;
                    switch (root.mode) {
                        case "os-prompt":
                            return qsTr("Uma nova versão do Jarvis OS está pronta para ser " +
                                        "instalada. A instalação dura alguns minutos e exige " +
                                        "reiniciar.")
                                 + (UpdaterBridge.osVersion.length > 0
                                    ? "\n\n" + qsTr("Versão: ") + UpdaterBridge.osVersion
                                    : "");
                        case "reboot":
                            return qsTr("A nova versão foi baixada e está pronta. " +
                                        "Reinicie para passar a usá-la — você pode adiar.");
                        case "active":
                            return UpdaterBridge.stage === "os.upgrade"
                                ? qsTr("Baixando e preparando a nova versão. Você poderá " +
                                       "continuar usando o sistema; será preciso reiniciar " +
                                       "no final.")
                                : qsTr("Baixando os pacotes que a Lilith precisa para " +
                                       "responder. Isso só acontece no primeiro boot — depois " +
                                       "fica tudo offline.");
                        default: return "";
                    }
                }
            }

            Item { Layout.fillHeight: true }

            // ── Progress (only in active mode) ──
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 8
                visible: root.mode === "active"

                Text {
                    Layout.fillWidth: true
                    text: UpdaterBridge.stage.length > 0
                        ? UpdaterBridge.stage + " · " + UpdaterBridge.message
                        : UpdaterBridge.message
                    color: Theme.textDim
                    font.pixelSize: 12
                    elide: Text.ElideRight
                }

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
                        width: parent.width
                             * Math.max(0, Math.min(100, UpdaterBridge.percent))
                             / 100

                        Behavior on width {
                            NumberAnimation {
                                duration: Theme.animFast
                                easing.type: Easing.OutCubic
                            }
                        }
                    }

                    // Indeterminate path — a small marker sliding back and forth.
                    Rectangle {
                        id: indeterminate
                        visible: UpdaterBridge.percent < 0 && !UpdaterBridge.failed
                        radius: 4
                        color: Theme.accent
                        height: parent.height
                        width: parent.width * 0.25

                        SequentialAnimation on x {
                            loops: Animation.Infinite
                            running: indeterminate.visible
                            NumberAnimation {
                                from: 0
                                to: indeterminate.parent.width - indeterminate.width
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

            // ── CTA row (only in os-prompt / reboot modes) ──
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 10
                visible: root.mode === "os-prompt" || root.mode === "reboot"

                ApprovalButton {
                    text: root.mode === "reboot"
                        ? qsTr("Adiar")
                        : qsTr("Mais tarde")
                    accent: Theme.textDim
                    onClicked: root.hide()
                }
                ApprovalButton {
                    text: root.mode === "reboot"
                        ? qsTr("Reiniciar agora")
                        : qsTr("Instalar agora")
                    accent: Theme.accent
                    filled: true
                    onClicked: {
                        if (root.mode === "reboot") {
                            // The shell can't reboot the host directly, so we
                            // just suggest the user run `reboot` themselves
                            // via the notify channel. A future commit can wire
                            // this to a confirmed `systemctl reboot` via the
                            // Action Bus (terminal.execute scope, persistent
                            // grant from this flow).
                            ActionBusBridge.dispatch("system.notify",
                                JSON.stringify({
                                    "title": "Jarvis OS",
                                    "body": "Reinicie para concluir a atualização."
                                }));
                        } else {
                            UpdaterBridge.applyOSUpgrade();
                        }
                    }
                }
            }
        }
    }
}
