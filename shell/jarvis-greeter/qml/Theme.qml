pragma Singleton
import QtQuick

/// Mirror of shell/jarvis-shell/qml/Theme.qml. Duplicated rather than
/// imported because the greeter runs in its own process before the
/// user session — Jarvis.Shell isn't on the QML path here. Keep these
/// values in sync with the shell's Theme to avoid a visual seam at
/// login → desktop transition.
QtObject {
    readonly property color background:    "#0a0a14"
    readonly property color surface:       Qt.rgba(0.08, 0.08, 0.12, 0.78)
    readonly property color surfaceBright: Qt.rgba(0.15, 0.15, 0.22, 0.85)
    readonly property color border:        Qt.rgba(1, 1, 1, 0.08)
    readonly property color text:          "#e8e8f0"
    readonly property color textDim:       "#9090a0"
    readonly property color accent:        "#7c5cff"
    readonly property color success:       "#3ad17a"
    readonly property color danger:        "#ff5c7c"

    readonly property int radius:       12
    readonly property int gap:          12
    readonly property int pad:          14

    readonly property int animFast:     120
    readonly property int animNormal:   200
}
