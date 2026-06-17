import QtQuick
import QtQuick.Effects
import Jarvis.Shell

/// Glassmorphism surface — a depth-graded translucent card with a soft inner
/// highlight, an accent glow hugging the top edge, and a GPU drop shadow.
/// Drop-in replacement for the `Rectangle { color: Theme.surfaceBright;
/// border... }` pattern every panel used to repeat by hand.
///
/// What it gives every panel for free:
///   - vertical body gradient (lighter top → darker base) for real depth
///   - accent top-glow line + bloom (the shared "header accent" — design C)
///   - 1 px inner top highlight for the soft-glass top edge
///   - layered GPU drop shadow so the card reads as floating
///   - opt-in hover/focus accent ring via `accentGlow`
///
/// What it deliberately doesn't do — and why:
///   - real backdrop blur (desktop behind staying visible but blurred) needs
///     compositor support. labwc exposes no blur protocol; the Jarvis Smithay
///     compositor will. Until then the colour stack stays opaque enough that
///     the absence reads as intentional flat-glass, not a missing effect.
///
/// Usage:
///   GlassPanel {
///       anchors.fill: parent
///       anchors.margins: 8
///       // children paint inside the contentArea
///   }
Item {
    id: root

    /// Inner content slot — set anchors.fill: parent on children.
    default property alias contentChildren: contentArea.children

    /// Optional accent ring strength when hovered/focused. 0 disables.
    property real accentGlow: 0.0

    /// Whether the soft drop shadow underneath is rendered. Off when the
    /// panel hugs a screen edge (a bar) — a shadow that runs off the output
    /// looks like a clipping bug, not a depth cue.
    property bool dropShadow: true

    /// The violet glow along the top edge. The shared accent that ties every
    /// panel together (design language "C"). Off for edge-anchored bars.
    property bool topAccent: true

    /// Corner radius. Defaults to the standard surface radius; raise to
    /// Theme.radiusLarge for the bigger cards (Settings drawer, launcher).
    property real radius: Theme.radius

    // ── Soft drop shadow ─────────────────────────────────────────────────
    // MultiEffect on the panel gives a real GPU shadow, not the "two stacked
    // translucent rectangles" hack. Darker than the wallpaper so the card
    // reads against it, not only against other dark surfaces.
    layer.enabled: dropShadow
    layer.effect: MultiEffect {
        shadowEnabled: true
        shadowBlur: 1.0
        shadowOpacity: 0.55
        shadowColor: "#000000"
        shadowVerticalOffset: 8
        shadowHorizontalOffset: 0
    }

    // ── Glass body ───────────────────────────────────────────────────────
    Rectangle {
        id: body
        anchors.fill: parent
        radius: root.radius
        border.width: 1
        border.color: root.accentGlow > 0.01
            ? Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b,
                      0.25 + 0.55 * root.accentGlow)
            : Theme.border

        // Depth gradient — the single biggest lift over the old flat fill.
        gradient: Gradient {
            GradientStop { position: 0.0; color: Theme.surfaceTop }
            GradientStop { position: 1.0; color: Theme.surfaceBottom }
        }

        Behavior on border.color {
            ColorAnimation { duration: Theme.animFast }
        }

        // ── Accent bloom along the top edge (design "C") ─────────────────
        // A soft violet wash hugging the top, plus a crisp 1 px accent line.
        // Both inset 1 px and share the body radius so the rounded corners
        // stay clean; the bloom fades to transparent well before its square
        // bottom, so that edge is never visible.
        Rectangle {
            visible: root.topAccent
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.margins: 1
            height: 56
            radius: root.radius
            gradient: Gradient {
                GradientStop {
                    position: 0.0
                    color: Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b,
                                   0.16 + 0.14 * root.accentGlow)
                }
                GradientStop { position: 1.0; color: "transparent" }
            }
        }
        Rectangle {
            visible: root.topAccent
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.margins: 1
            height: 2
            radius: root.radius
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0.0; color: "transparent" }
                GradientStop {
                    position: 0.5
                    color: Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b,
                                   0.55 + 0.35 * root.accentGlow)
                }
                GradientStop { position: 1.0; color: "transparent" }
            }
        }

        // ── Inner top highlight ──────────────────────────────────────────
        // 1 px strip fakes the soft light source the design language calls
        // for. Sits above the accent so the glass top edge stays crisp.
        Rectangle {
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.margins: 1
            height: 1
            color: Qt.rgba(1, 1, 1, 0.07)
            radius: root.radius
        }

        // ── Content slot ─────────────────────────────────────────────────
        Item {
            id: contentArea
            anchors.fill: parent
        }
    }
}
