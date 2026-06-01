import QtQuick
import QtQuick.Window
import Jarvis.Shell

/// The desktop surface — the icon column every other OS puts on an
/// empty desktop (Computador / Pasta Pessoal / Lixeira). It is a second
/// top-level Window alongside Main.qml's bar; main.cpp recognises it by
/// `objectName` and anchors it to all four output edges on the
/// wlr-layer-shell *bottom* layer: above swaybg's wallpaper, below every
/// app window — so opened windows cover the icons like a real desktop.
/// It takes no keyboard focus.
///
/// Icons activate through `app.open` (Action Bus). Computador / Lixeira
/// use the `computer:///` / `trash:///` KIO URIs, which iso/assets/xdg/
/// mimeapps.list pins to Dolphin so they reliably open; Pasta Pessoal
/// opens the real $HOME path (HomePath context property from main.cpp),
/// served by the inode/directory default.
Window {
    id: root
    objectName: "jarvis-desktop"
    visible: true
    color: "transparent"
    flags: Qt.FramelessWindowHint

    // Fallback sizing for the non-layer-shell case (dev desktops, where
    // there is no compositor to stretch us). Under labwc the four-edge
    // anchors in main.cpp drive the real size.
    Component.onCompleted: {
        const s = Qt.application.screens[0];
        if (s) { width = s.width; height = s.height; }
    }

    // Clicking the empty desktop clears any icon selection.
    MouseArea {
        anchors.fill: parent
        onClicked: icons.selected = ""
    }

    Column {
        id: icons
        x: 24
        y: 24
        spacing: 10

        // The label of the currently-selected icon ("" = none). Drives
        // each icon's highlight; single click sets it, empty-desktop
        // click clears it.
        property string selected: ""

        DesktopIcon {
            label: qsTr("Computador")
            iconName: "computer"
            selected: icons.selected === label
            onSelectRequested: icons.selected = label
            // Open the filesystem root. The GVfs `computer:///` URI is a
            // GNOME/Nautilus scheme that Dolphin (our file manager) rejects
            // with "Invalid protocol 'computer'", so we open a real path.
            onActivated: ActionBusBridge.dispatch(
                "app.open", JSON.stringify({ "app": "/" }))
        }

        DesktopIcon {
            label: qsTr("Pasta Pessoal")
            iconName: "user-home"
            selected: icons.selected === label
            onSelectRequested: icons.selected = label
            onActivated: ActionBusBridge.dispatch(
                "app.open", JSON.stringify({ "app": HomePath }))
        }

        DesktopIcon {
            label: qsTr("Lixeira")
            iconName: "user-trash"
            selected: icons.selected === label
            onSelectRequested: icons.selected = label
            onActivated: ActionBusBridge.dispatch(
                "app.open", JSON.stringify({ "app": "trash:///" }))
        }
    }
}
