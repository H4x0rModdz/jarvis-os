pragma Singleton
import QtQuick

/// Design tokens — the single source for colour, shape, type and motion.
/// The reference is the Liquid Glass language of macOS 26 (dark appearance):
/// chrome is translucent material over the desktop, content is white text at
/// graded opacities, one blue accent, large soft radii, quick ease-out motion.
/// (See .jarvis/skills/jarvis-design-language.md.)
///
/// Nothing in the shell should spell out a colour, a radius or a duration —
/// if a value is missing here, add a token, don't inline a literal.
QtObject {
    // ── Type ────────────────────────────────────────────────────────────
    // Inter is the UI face (the image ships it); the rest are fallbacks.
    readonly property string fontFamily: "Inter, Noto Sans, sans-serif"
    readonly property string monoFamily: "JetBrains Mono, Noto Sans Mono, monospace"
    readonly property int fontCaption: 11
    readonly property int fontBody:    13
    readonly property int fontTitle:   15
    readonly property int fontLarge:   22

    // ── Labels (white at graded opacity, like the system label colours) ──
    readonly property color text:       Qt.rgba(1, 1, 1, 0.92)
    readonly property color textDim:    Qt.rgba(1, 1, 1, 0.55)
    readonly property color textFaint:  Qt.rgba(1, 1, 1, 0.28)

    // ── Accent + semantic colours ───────────────────────────────────────
    readonly property color accent:  "#0a84ff"
    readonly property color success: "#30d158"
    readonly property color warning: "#ffd60a"
    readonly property color danger:  "#ff453a"

    // ── Glass materials ─────────────────────────────────────────────────
    // A glass surface is: blurred backdrop → tint → 1 px specular rim.
    // `glassTint` sits over a blurred backdrop; `glassSolid` is used where
    // there is no backdrop to show through (and for "reduce transparency").
    readonly property color glassTint:   Qt.rgba(0.11, 0.11, 0.13, 0.52)
    readonly property color glassSolid:  Qt.rgba(0.13, 0.13, 0.15, 0.94)
    readonly property color glassRim:    Qt.rgba(1, 1, 1, 0.16)
    readonly property color glassShine:  Qt.rgba(1, 1, 1, 0.34)
    readonly property color shadow:      Qt.rgba(0, 0, 0, 0.38)

    // Fills for controls sitting ON glass.
    readonly property color fill:        Qt.rgba(1, 1, 1, 0.08)
    readonly property color fillHover:   Qt.rgba(1, 1, 1, 0.14)
    readonly property color fillPressed: Qt.rgba(1, 1, 1, 0.20)
    readonly property color separator:   Qt.rgba(1, 1, 1, 0.12)

    // Legacy names, still used across the panels. They map onto the tokens
    // above so every surface moves together.
    readonly property color background:    "#1c1c1e"
    readonly property color surface:       glassSolid
    readonly property color surfaceBright: Qt.rgba(0.17, 0.17, 0.19, 0.94)
    readonly property color surfaceTop:    glassSolid
    readonly property color surfaceBottom: glassSolid
    readonly property color border:        separator
    // Translucent scrim painted behind a modal panel to dim the desktop.
    readonly property color scrim:         Qt.rgba(0, 0, 0, 0.35)

    // ── Shape ───────────────────────────────────────────────────────────
    readonly property int radiusControl: 8    // buttons, menu rows, fields
    readonly property int radius:        16   // menus, popovers, cards
    readonly property int radiusLarge:   22   // windows, sheets, big panels
    readonly property int radiusDock:    24

    // ── Metrics ─────────────────────────────────────────────────────────
    readonly property int topBarHeight: 30
    readonly property int barHeight:    64    // legacy: height reserved by the dock
    readonly property int dockIcon:     50
    readonly property int gap:          12
    readonly property int pad:          14
    /// Gap between a screen edge / the menu bar and a floating surface.
    readonly property int edge:         8

    // ── Motion ──────────────────────────────────────────────────────────
    // Modelled on Hyprland's default animations: things "pop in" from 87 %
    // scale on a strongly front-loaded curve (its easeOutQuint bezier), and
    // leave quickly and linearly. Capped at 250 ms — Hyprland's own 410 ms
    // pop-in is tuned for windows; on a menu it reads as lag.
    readonly property var  curveOut:   [0.23, 1.0, 0.32, 1.0, 1.0, 1.0]
    readonly property real popScale:   0.87
    readonly property int  animPopIn:  250
    readonly property int  animPopOut: 150
    readonly property int animFast:   120
    readonly property int animNormal: 200
    readonly property int animSlow:   250
}
