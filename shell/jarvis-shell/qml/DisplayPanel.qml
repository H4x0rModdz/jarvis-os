import QtQuick
import QtQuick.Layouts
import QtQuick.Window
import Jarvis.Shell

/// Display panel — one card per output, each with an on/off toggle,
/// a mode dropdown, and a scale slider. Position drag is V2 — the
/// rare multi-monitor user can call DisplayBridge.setPosition from
/// Lilith ("monitor externo à direita") until then.
///
/// Opened from the SettingsPanel's "Display" button rather than a
/// bar icon — display config is infrequent enough that a bar slot
/// would be wasted real estate.
Window {
    id: root
    visible: false
    width: 520
    height: 480
    color: "transparent"
    // Qt.Dialog (not Qt.Tool): under labwc a Tool window is non-activatable,
    // so it never gets keyboard focus — the Escape Shortcut never fires.
    // Qt.Dialog gets the activation the Launcher/JarvisMenu rely on.
    flags: Qt.Dialog | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
    title: qsTr("Monitores")

    // Suppress the spurious first deactivate wlroots fires between show and
    // the compositor granting focus — without it the panel closes instantly.
    property bool _ignoreDeactivate: false
    Timer { id: armTimer; interval: 250; onTriggered: root._ignoreDeactivate = false }

    function requestOpen() {
        if (Qt.application.screens.length > 0) {
            const s = Qt.application.screens[0];
            x = s.virtualX + Math.floor((s.width - width) / 2);
            y = s.virtualY + Math.floor((s.height - height) / 2);
        }
        _ignoreDeactivate = true;
        visible = true;
        requestActivate();
        armTimer.restart();
        DisplayBridge.refresh();
    }

    Shortcut {
        sequence: "Escape"
        onActivated: root.visible = false
    }

    onActiveChanged: {
        if (!active && !_ignoreDeactivate && visible) root.visible = false;
    }

    GlassPanel {
        anchors.fill: parent
        anchors.margins: 8

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 18
            spacing: 12

            // ── Header ─────────────────────────────────────────────
            RowLayout {
                Layout.fillWidth: true
                Text {
                    text: qsTr("MONITORES")
                    color: Theme.accent
                    font.pixelSize: 11
                    font.weight: Font.Bold
                    font.letterSpacing: 2
                    Layout.fillWidth: true
                }
                Text {
                    text: DisplayBridge.outputs.length + " "
                        + (DisplayBridge.outputs.length === 1
                            ? qsTr("conectado")
                            : qsTr("conectados"))
                    color: Theme.textDim
                    font.pixelSize: 11
                }

                // Close — explicit affordance (Esc + click-outside
                // aren't reliable in a VM where focus can lag).
                Text {
                    text: "×"
                    color: closeArea.containsMouse ? Theme.danger : Theme.textDim
                    font.pixelSize: 20
                    font.weight: Font.Bold
                    Layout.alignment: Qt.AlignVCenter
                    MouseArea {
                        id: closeArea
                        anchors.fill: parent
                        anchors.margins: -6
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.visible = false
                    }
                }
            }

            // ── Outputs list ──────────────────────────────────────
            ListView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 12
                model: DisplayBridge.outputs

                delegate: Rectangle {
                    width: ListView.view.width
                    implicitHeight: card.implicitHeight + 24
                    radius: 12
                    color: modelData.enabled
                        ? Qt.rgba(1, 1, 1, 0.05)
                        : Qt.rgba(1, 1, 1, 0.02)
                    border.color: modelData.enabled ? Theme.accent : Theme.border
                    border.width: 1
                    opacity: modelData.enabled ? 1.0 : 0.65

                    // Hoisted reference so inner Repeaters (whose
                    // modelData is the mode entry, not the output)
                    // can still reach the parent output.
                    property var outputData: modelData

                    ColumnLayout {
                        id: card
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: 12
                        spacing: 8

                        // Name + enabled toggle.
                        RowLayout {
                            Layout.fillWidth: true
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 1
                                Text {
                                    text: modelData.description && modelData.description.length > 0
                                        ? modelData.description
                                        : modelData.name
                                    color: Theme.text
                                    font.pixelSize: 14
                                    font.weight: Font.Bold
                                }
                                Text {
                                    text: modelData.name
                                    color: Theme.textDim
                                    font.pixelSize: 10
                                }
                            }
                            // Power toggle.
                            Rectangle {
                                implicitWidth: 44
                                implicitHeight: 24
                                radius: 12
                                color: modelData.enabled
                                    ? Theme.accent
                                    : Qt.rgba(1, 1, 1, 0.08)
                                border.color: modelData.enabled
                                    ? Theme.accent
                                    : Theme.border
                                border.width: 1
                                Rectangle {
                                    width: 18; height: 18; radius: 9
                                    color: Theme.text
                                    anchors.verticalCenter: parent.verticalCenter
                                    x: modelData.enabled
                                        ? parent.width - width - 3
                                        : 3
                                    Behavior on x {
                                        NumberAnimation {
                                            duration: Theme.animFast
                                            easing.type: Easing.OutCubic
                                        }
                                    }
                                }
                                MouseArea {
                                    anchors.fill: parent
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: DisplayBridge.setEnabled(
                                        modelData.name, !modelData.enabled)
                                }
                            }
                        }

                        // Mode picker — pills for each available mode,
                        // current one highlighted.
                        Text {
                            text: qsTr("Resolução")
                            color: Theme.textDim
                            font.pixelSize: 10
                            font.weight: Font.Bold
                            font.letterSpacing: 1
                            visible: modelData.enabled
                        }
                        Flow {
                            Layout.fillWidth: true
                            spacing: 4
                            visible: modelData.enabled

                            Repeater {
                                model: modelData.modes || []
                                delegate: Rectangle {
                                    readonly property var modeData: modelData
                                    readonly property string outName: outputData.name
                                    readonly property bool current:
                                        modeData.mode === outputData.currentMode
                                    implicitWidth: modeLabel.implicitWidth + 14
                                    implicitHeight: 22
                                    radius: 11
                                    color: current
                                        ? Qt.rgba(Theme.accent.r,
                                                  Theme.accent.g,
                                                  Theme.accent.b, 0.35)
                                        : (modeArea.containsMouse
                                            ? Qt.rgba(1, 1, 1, 0.08)
                                            : Qt.rgba(1, 1, 1, 0.03))
                                    border.color: current
                                        ? Theme.accent
                                        : Theme.border
                                    border.width: 1
                                    Text {
                                        id: modeLabel
                                        anchors.centerIn: parent
                                        text: modeData.mode
                                        color: current ? Theme.text : Theme.textDim
                                        font.pixelSize: 10
                                        font.weight: current ? Font.Bold : Font.Normal
                                    }
                                    MouseArea {
                                        id: modeArea
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: DisplayBridge.setMode(
                                            outName, modeData.mode)
                                    }
                                }
                            }
                        }

                        // Scale slider as +/- buttons + value text.
                        // No native QtQuick.Controls Slider import to
                        // keep the dependency surface flat.
                        RowLayout {
                            Layout.fillWidth: true
                            visible: modelData.enabled
                            spacing: 8

                            Text {
                                text: qsTr("Escala")
                                color: Theme.textDim
                                font.pixelSize: 10
                                font.weight: Font.Bold
                                font.letterSpacing: 1
                            }
                            Text {
                                text: (modelData.scale || 1.0).toFixed(2) + "×"
                                color: Theme.text
                                font.pixelSize: 12
                                Layout.fillWidth: true
                                horizontalAlignment: Text.AlignRight
                            }
                            Repeater {
                                model: [
                                    { label: "−", delta: -0.25 },
                                    { label: "+", delta:  0.25 },
                                ]
                                delegate: Rectangle {
                                    readonly property var btnData: modelData
                                    readonly property string outName: outputData.name
                                    readonly property real currentScale:
                                        parseFloat(outputData.scale) || 1.0
                                    implicitWidth: 24
                                    implicitHeight: 22
                                    radius: 11
                                    color: scaleArea.containsMouse
                                        ? Qt.rgba(1, 1, 1, 0.10)
                                        : Qt.rgba(1, 1, 1, 0.04)
                                    border.color: Theme.border
                                    border.width: 1
                                    Text {
                                        anchors.centerIn: parent
                                        text: btnData.label
                                        color: Theme.text
                                        font.pixelSize: 13
                                        font.weight: Font.Bold
                                    }
                                    MouseArea {
                                        id: scaleArea
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: {
                                            const next = Math.max(0.5,
                                                Math.min(3.0,
                                                    currentScale + btnData.delta));
                                            DisplayBridge.setScale(outName, next);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── Error footer ──────────────────────────────────────
            Text {
                visible: DisplayBridge.lastError.length > 0
                Layout.fillWidth: true
                text: DisplayBridge.lastError
                color: Theme.danger
                font.pixelSize: 11
                wrapMode: Text.WordWrap
                horizontalAlignment: Text.AlignHCenter
            }
        }
    }

}
