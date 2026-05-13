import QtQuick
import QtQuick.Window
import QtQuick.Layouts
import Jarvis.Shell

/// Full-screen-ish app launcher. Opens centered, ESC closes, clicking a tile
/// dispatches `app.open` via the Action Bus directly (no Lilith round-trip —
/// the user already chose by clicking).
///
/// Phase 1 — regular toplevel `Qt.Dialog` sized 720x520. Phase 2 will swap
/// this for a fullscreen wlr-layer-shell Overlay surface so it can paint
/// over every window on every output.
Window {
    id: root
    width: 720
    height: 520
    title: qsTr("Launcher")
    color: "transparent"
    flags: Qt.Dialog | Qt.WindowStaysOnTopHint
    visible: false

    // Suppress the auto-close-on-deactivate during the brief window between
    // calling `open()` and the compositor actually granting us focus —
    // otherwise the very first activeChanged event closes us immediately.
    property bool _ignoreDeactivate: false

    function open() {
        const s = Qt.application.screens[0];
        if (s) {
            x = s.virtualX + Math.floor((s.width - width) / 2);
            y = s.virtualY + Math.floor((s.height - height) / 2);
        }
        _ignoreDeactivate = true;
        // Re-walk the .desktop dirs so anything installed since the
        // session started (Flatpaks Lilith just pulled, SDK apps that
        // dropped a manifest) shows up without a relogin.
        apps.rescan();
        visible = true;
        apps.filter = "";
        search.forceActiveFocus();
        // Activation lands a frame or two after show; release the gate
        // shortly after.
        deactivateArm.restart();
    }

    function close() {
        visible = false;
        // Hand activation back to whoever opened us — Wayland doesn't auto-
        // reactivate the transient parent when a child window unmaps, and
        // without this the bar's input field can't pick up keystrokes.
        if (transientParent) {
            transientParent.requestActivate();
        }
    }

    Timer {
        id: deactivateArm
        interval: 250
        onTriggered: root._ignoreDeactivate = false
    }

    // Click outside / focus another surface (e.g. the bar's input) -> close.
    // On Wayland only one surface owns focus at a time, so this also acts
    // as the "click outside to dismiss" behavior.
    onActiveChanged: {
        if (!active && !_ignoreDeactivate && visible) {
            close();
        }
    }

    DesktopAppsModel { id: apps }

    GlassPanel {
        anchors.fill: parent
        anchors.margins: 8
        // accentGlow lights the border when typing — visual cue that
        // the launcher is the focused surface.
        accentGlow: search.activeFocus ? 1.0 : 0.0

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 20
            spacing: 16

            // ── Search bar ────────────────────────────────────────
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 48
                radius: Theme.radius - 4
                color: Qt.rgba(1, 1, 1, 0.05)
                border.color: search.activeFocus ? Theme.accent : Theme.border
                border.width: 1
                Behavior on border.color { ColorAnimation { duration: Theme.animFast } }

                TextInput {
                    id: search
                    anchors.fill: parent
                    anchors.leftMargin: 16
                    anchors.rightMargin: 16
                    verticalAlignment: TextInput.AlignVCenter
                    color: Theme.text
                    selectionColor: Theme.accent
                    selectedTextColor: Theme.text
                    font.pixelSize: 18
                    clip: true
                    onTextChanged: apps.filter = text
                    Keys.onEscapePressed: root.close()
                    Keys.onReturnPressed: {
                        // Launch the first visible result on Enter.
                        if (grid.count > 0) {
                            grid.itemAtIndex(0).launch();
                        }
                    }
                }

                Text {
                    anchors.fill: search
                    verticalAlignment: Text.AlignVCenter
                    text: qsTr("Buscar aplicativos...")
                    color: Theme.textDim
                    font.pixelSize: 18
                    visible: search.text.length === 0 && !search.activeFocus
                }
            }

            // ── Count chip ───────────────────────────────────────
            Text {
                text: qsTr("%1 aplicativos").arg(apps.count)
                color: Theme.textDim
                font.pixelSize: 11
                font.capitalization: Font.AllUppercase
                font.weight: Font.Bold
            }

            // ── Grid ─────────────────────────────────────────────
            GridView {
                id: grid
                Layout.fillWidth: true
                Layout.fillHeight: true
                model: apps
                cellWidth: width / 4
                cellHeight: 96
                clip: true
                focus: false

                delegate: AppCell {
                    width: grid.cellWidth - 6
                    height: grid.cellHeight - 6
                    name: model.name
                    comment: model.comment
                    iconSource: model.icon
                    onClicked: launch()
                    function launch() {
                        ActionBusBridge.dispatch("app.open",
                            JSON.stringify({ "app": model.desktopId }));
                        root.close();
                    }
                }
            }
        }
    }

    // ESC anywhere (even when grid has focus) closes.
    Shortcut {
        sequence: "Escape"
        onActivated: root.close()
    }
}
