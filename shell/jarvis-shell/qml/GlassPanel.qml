import QtQuick
import QtQuick.Effects
import Jarvis.Shell

/// Glassmorphism surface — shadowed translucent fill with a soft inner
/// highlight and an accent-glow ring. Drop-in replacement for the
/// `Rectangle { color: Theme.surfaceBright; border... }` pattern that
/// every panel uses.
///
/// V1 scope (this commit):
///   - layered drop shadow via MultiEffect for a real "floating" feel
///   - inner top highlight (1 px) for the soft-glass top edge
///   - hover-able accent ring on opt-in panels
///
/// What V1 deliberately doesn't do — and why:
///   - backdrop blur (the desktop behind the panel staying visible but
///     blurred) needs compositor support. labwc doesn't expose a blur
///     protocol; the Jarvis Smithay compositor will. Until then we
///     keep the colour stack opaque enough that the absence of real
///     backdrop blur reads as intentional flat-glass design rather
///     than a missing effect.
///
/// Usage:
///   GlassPanel {
///       anchors.fill: parent
///       anchors.margins: 8
///       // children paint inside the glass card
///   }
Item {
    id: root

    /// Inner content padding so callers don't have to anchor children
    /// to a child Item — set anchors.fill: parent on children to
    /// fill the contentArea.
    default property alias contentChildren: contentArea.children

    /// Optional accent glow strength when hovered/focused. 0 disables.
    property real accentGlow: 0.0

    /// Whether the soft drop shadow underneath is rendered. Off when
    /// the panel is anchored to a screen edge (bar) — a shadow that
    /// runs off the output looks like a clipping bug, not a depth cue.
    property bool dropShadow: true

    // ── Soft drop shadow ─────────────────────────────────────────────
    // MultiEffect on the panel itself gives us a real GPU-rendered
    // shadow, not the "two translucent rectangles stacked" hack. The
    // shadowColor is darker than Theme.background so we read against
    // the wallpaper too, not just other dark surfaces.
    layer.enabled: dropShadow
    layer.effect: MultiEffect {
        shadowEnabled: true
        shadowBlur: 1.0
        shadowOpacity: 0.5
        shadowColor: "#000000"
        shadowVerticalOffset: 4
        shadowHorizontalOffset: 0
    }

    // ── Glass body ───────────────────────────────────────────────────
    Rectangle {
        id: body
        anchors.fill: parent
        radius: Theme.radius
        color: Theme.surfaceBright
        border.color: root.accentGlow > 0.01
            ? Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b,
                      0.25 + 0.55 * root.accentGlow)
            : Theme.border
        border.width: 1

        Behavior on border.color {
            ColorAnimation { duration: Theme.animFast }
        }

        // ── Inner top highlight ─────────────────────────────────────
        // 1-pixel strip along the top inner edge; fakes the soft light
        // source the design language calls for. Same trick every
        // existing panel uses — centralised here so we don't repeat
        // the literals.
        Rectangle {
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.margins: 1
            height: 1
            color: Qt.rgba(1, 1, 1, 0.06)
            radius: Theme.radius
        }

        // ── Content slot ────────────────────────────────────────────
        Item {
            id: contentArea
            anchors.fill: parent
        }
    }
}
