import QtQuick
import QtQuick.Effects
import Jarvis.Shell

/// The glass surface every piece of chrome is made of — dock, menus,
/// popovers, panels. Liquid-Glass-style: a blurred view of what is behind,
/// a dark tint, a bright specular rim along the top edge, and a soft shadow.
///
///   GlassPanel {
///       anchors.fill: parent
///       anchors.margins: Theme.edge
///       // children go into the content slot
///   }
///
/// Where the blur comes from: the shell cannot see through its own window —
/// only the compositor can blur what is behind a surface, and labwc cannot.
/// So the panel draws a pre-blurred copy of the WALLPAPER
/// (`image://backdrop/blur`), cropped to where the panel sits on screen. Over
/// the desktop that is indistinguishable from real blur and costs nothing per
/// frame. It needs to know where its window is, which a Wayland client is not
/// told — the owner passes `windowX` / `windowY` (it knows: it anchored the
/// window). A panel that can float anywhere sets `backdrop: false` and gets
/// the opaque material instead.
Item {
    id: root

    /// Inner content slot — set anchors.fill: parent on children.
    default property alias contentChildren: contentArea.children

    /// Corner radius. Theme.radius for menus/popovers, Theme.radiusLarge for
    /// big panels, Theme.radiusDock for the dock.
    property real radius: Theme.radius

    /// Draw the blurred wallpaper behind the tint (needs windowX/windowY).
    /// Off — the default — gives the opaque material.
    property bool backdrop: false
    /// Screen position of the WINDOW this panel lives in (see header).
    property real windowX: 0
    property real windowY: 0

    /// Soft drop shadow under the panel.
    property bool dropShadow: true

    /// Optional accent ring strength (0..1) for a focused/hovered panel.
    property real accentGlow: 0.0

    /// Kept for callers written against the previous panel; the violet top
    /// bloom it controlled is not part of this design.
    property bool topAccent: false

    // Position of this panel on the screen = window position + position
    // inside the window. Re-evaluated when the panel or the window moves.
    readonly property point screenPos: {
        root.x; root.y; root.width; root.height; root.windowX; root.windowY;
        const p = root.mapToItem(null, 0, 0);
        return Qt.point(root.windowX + p.x, root.windowY + p.y);
    }

    // ── Shadow ───────────────────────────────────────────────────────────
    // One analytic pass behind the body; never re-rendered by content.
    RectangularShadow {
        visible: root.dropShadow
        anchors.fill: parent
        radius: root.radius
        offset.y: 10
        blur: 34
        color: Theme.shadow
    }

    // ── Backdrop: the blurred wallpaper, cropped to this panel ──────────
    // Laid out at screen size and shifted so the part under the panel shows,
    // exactly as the wallpaper itself is placed (cover/crop, like swaybg
    // `fill`). Static, so the rounded-corner mask is rendered once.
    Item {
        id: backdropClip
        anchors.fill: parent
        visible: root.backdrop && !CompositorBlur
        layer.enabled: root.backdrop && !CompositorBlur
        layer.effect: MultiEffect {
            maskEnabled: true
            maskSource: backdropMask
            maskThresholdMin: 0.5
            maskSpreadAtMin: 1.0
        }
        Image {
            x: -root.screenPos.x
            y: -root.screenPos.y
            width: Screen.width
            height: Screen.height
            source: (root.backdrop && !CompositorBlur) ? "image://backdrop/blur" : ""
            fillMode: Image.PreserveAspectCrop
            smooth: true
            cache: true
        }
    }
    Item {
        id: backdropMask
        anchors.fill: parent
        visible: false
        layer.enabled: true
        Rectangle { anchors.fill: parent; radius: root.radius; color: "black" }
    }

    // ── Body: tint + rim ─────────────────────────────────────────────────
    Rectangle {
        id: body
        anchors.fill: parent
        radius: root.radius
        // With compositor blur every panel is real glass, wherever it is.
        color: (root.backdrop || CompositorBlur) ? Theme.glassTint : Theme.glassSolid
        border.width: 1
        border.color: root.accentGlow > 0.01
            ? Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b,
                      0.35 + 0.55 * root.accentGlow)
            : Theme.glassRim
        Behavior on border.color { ColorAnimation { duration: Theme.animFast } }

        // Specular highlight: light catching the top edge of the glass. A
        // hairline that is brightest in the middle and gone before the
        // rounded corners start.
        Rectangle {
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.leftMargin: root.radius
            anchors.rightMargin: root.radius
            height: 1
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0.0; color: "transparent" }
                GradientStop { position: 0.5; color: Theme.glassShine }
                GradientStop { position: 1.0; color: "transparent" }
            }
        }

        Item {
            id: contentArea
            anchors.fill: parent
        }
    }
}
