import QtQuick
import QtQuick.Window
import Jarvis.Shell

/// Lilith's embodied presence — a floating, draggable 3D companion window
/// (ADR 0028). Frameless + transparent + always-on-top corner tile.
///
/// The 3D content lives in the shared `LilithAvatarView` (also used by the
/// desktop HUD's Lilith center, ADR 0031). This window has no QtQuick3D import
/// itself; Main.qml wraps this whole window in a Loader, so if QtQuick3D is
/// missing at runtime the view fails *contained* and the rest of the shell is
/// unaffected.
Window {
    id: root
    objectName: "jarvis-lilith-avatar"
    visible: true
    width: 260
    height: 340
    color: "transparent"
    // Qt.Dialog (not Tool): under labwc a Tool window is non-activatable.
    flags: Qt.Dialog | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
    title: qsTr("Lilith")

    // First show: park bottom-right, clear of the floating dock (~90px tall).
    Component.onCompleted: {
        if (Qt.application.screens.length > 0) {
            const s = Qt.application.screens[0];
            x = s.virtualX + s.width - width - 32;
            y = s.virtualY + s.height - height - 120;
        }
    }

    LilithAvatarView { anchors.fill: parent }

    // ── Drag to reposition; click (no drag) opens the conversation ─────
    MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        property point pressPos
        property bool dragging: false
        onPressed: (mouse) => { pressPos = Qt.point(mouse.x, mouse.y); dragging = false; }
        onPositionChanged: (mouse) => {
            const dx = mouse.x - pressPos.x;
            const dy = mouse.y - pressPos.y;
            if (!dragging && Math.hypot(dx, dy) > 6) dragging = true;
            if (dragging) {
                root.x += dx;
                root.y += dy;
            }
        }
        onReleased: { if (!dragging) ShellBus.toggleLilith(); }
    }
}
