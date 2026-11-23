import QtQuick
import QtQuick.Window
import org.kde.layershell as LayerShell
import Jarvis.Shell

/// Base for every menu, popover and dialog the shell opens. One behaviour,
/// everywhere:
///
///   - **Escape closes it**, always;
///   - **clicking outside closes it**;
///   - it **animates in and out** (Hyprland-style pop-in from the edge it
///     hangs from; fast linear exit);
///   - it sits where it belongs (`placement`), on every compositor.
///
/// How: the window is a transparent layer surface covering the whole output,
/// and the visible panel is an item inside it. That is what makes the three
/// behaviours reliable under Wayland —
///   * the surface takes the keyboard exclusively while open, so Escape
///     reaches us no matter which application had focus (a small "on demand"
///     popup only gets keys after the user clicks it);
///   * a click anywhere outside the panel lands on our own surface, so
///     "click outside" is a plain MouseArea instead of a guess based on
///     focus-change events that arrive late or not at all;
///   * the panel can move and fade freely inside a surface that never
///     resizes, which a window-sized popup cannot do.
/// It also means the surface's coordinates ARE screen coordinates, so a
/// GlassPanel child gets its frosted backdrop with `windowX/Y` left at 0.
///
/// Usage — children fill the panel:
///
///   PopupSurface {
///       placement: "top-right"; panelWidth: 380; panelHeight: 480
///       GlassPanel { anchors.fill: parent; backdrop: true; … }
///   }
///
/// Call `present()` / `dismiss()`; read `shown`. Do not assign `visible`.
Window {
    id: root

    default property alias content: panel.data

    /// Where the panel sits: "top-left" | "top-right" (under the menu bar),
    /// "top-center" (launcher), "center" (dialogs).
    property string placement: "center"
    property real panelWidth: 400
    property real panelHeight: 300
    /// Dim the desktop behind the panel (dialogs). Popovers leave it clear.
    property bool dim: false
    /// Layer namespace, for compositor rules.
    property string scope: "jarvis-popup"

    /// True from present() until dismiss(). The window itself stays mapped a
    /// little longer, until the closing animation has finished.
    property bool shown: false

    function present() { shown = true; requestActivate(); }
    function dismiss() { shown = false; }

    visible: shown || panel.opacity > 0.01
    color: "transparent"
    flags: Qt.FramelessWindowHint
    width: Screen.width
    height: Screen.height

    LayerShell.Window.scope: root.scope
    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.anchors: LayerShell.Window.AnchorTop | LayerShell.Window.AnchorBottom
                             | LayerShell.Window.AnchorLeft | LayerShell.Window.AnchorRight
    // -1: cover the menu bar and dock zones too, so this surface is exactly
    // the screen.
    LayerShell.Window.exclusionZone: -1
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityExclusive

    Shortcut {
        sequence: "Escape"
        enabled: root.shown
        onActivated: root.dismiss()
    }

    // Click outside the panel.
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.AllButtons
        onPressed: root.dismiss()
    }

    Rectangle {
        anchors.fill: parent
        visible: root.dim
        color: Theme.scrim
        opacity: root.shown ? 1.0 : 0.0
        Behavior on opacity { NumberAnimation { duration: Theme.animPopOut } }
    }

    Item {
        id: panel
        width: root.panelWidth
        height: root.panelHeight

        readonly property real below: Theme.topBarHeight + 6
        x: root.placement === "top-left"  ? Theme.edge
         : root.placement === "top-right" ? root.width - width - Theme.edge
         : Math.round((root.width - width) / 2)
        y: root.placement === "center"     ? Math.round((root.height - height) / 2)
         : root.placement === "top-center" ? Math.round(root.height * 0.16)
         : below

        // Grow out of the edge the panel hangs from.
        transformOrigin: root.placement === "top-left"  ? Item.TopLeft
                       : root.placement === "top-right" ? Item.TopRight
                       : root.placement === "center"    ? Item.Center
                       : Item.Top

        // Hyprland-style: pop in from 87 % on a front-loaded curve, leave
        // fast and linear (Theme § Motion).
        opacity: root.shown ? 1.0 : 0.0
        scale: root.shown ? 1.0 : Theme.popScale
        Behavior on opacity {
            NumberAnimation {
                duration: root.shown ? Theme.animPopIn : Theme.animPopOut
                easing.type: root.shown ? Easing.BezierSpline : Easing.Linear
                easing.bezierCurve: Theme.curveOut
            }
        }
        Behavior on scale {
            NumberAnimation {
                duration: root.shown ? Theme.animPopIn : Theme.animPopOut
                easing.type: root.shown ? Easing.BezierSpline : Easing.Linear
                easing.bezierCurve: Theme.curveOut
            }
        }

        // Clicks on the panel itself must not reach the "outside" area.
        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.AllButtons
        }
    }
}
