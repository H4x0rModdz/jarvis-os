pragma Singleton
import QtQuick

/// Centralised tokens for the Jarvis design language.
/// Numbers come from .jarvis/skills/jarvis-design-language.md:
///   - glass opacity 0.6 - 0.85
///   - blur 8 - 20px
///   - animations <= 250ms ease-out
///   - radius 12px on surfaces
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

    readonly property int   barHeight:     64
    readonly property int   radius:        12
    readonly property int   gap:           12
    readonly property int   pad:           14

    readonly property int   animFast:      120
    readonly property int   animNormal:    200
    readonly property int   animSlow:      250
}
