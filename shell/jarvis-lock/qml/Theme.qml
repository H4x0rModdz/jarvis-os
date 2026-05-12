pragma Singleton
import QtQuick

/// Mirror of shell/jarvis-shell/qml/Theme.qml. Duplicated rather than
/// imported because the lock screen is its own process with its own
/// QML module — it doesn't have access to Jarvis.Shell. Keep these
/// values in sync with the shell + greeter Theme.qml so the visual
/// transition between desktop → lock → desktop is seamless.
QtObject {
    readonly property color background:    "#0a0a14"
    readonly property color surface:       Qt.rgba(0.08, 0.08, 0.12, 0.78)
    readonly property color surfaceBright: Qt.rgba(0.15, 0.15, 0.22, 0.85)
    readonly property color border:        Qt.rgba(1, 1, 1, 0.08)
    readonly property color text:          "#e8e8f0"
    readonly property color textDim:       "#9090a0"
    readonly property color accent:        "#7c5cff"
    readonly property color danger:        "#ff5c7c"

    readonly property int radius:       12
    readonly property int animFast:     120
    readonly property int animNormal:   200
}
