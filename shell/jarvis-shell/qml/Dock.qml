import QtQuick
import QtQuick.Window
import QtQuick.Layouts
import org.kde.layershell as LayerShell
import Jarvis.Shell

/// The dock: a floating glass bar centred at the bottom of the screen with
/// the launcher tile, the pinned apps, any running-but-unpinned apps, then a
/// divider and the Lilith orb. Its own layer-shell surface, anchored to the
/// bottom edge only so the compositor centres it at its natural width.
///
/// Tiles show open vs minimized state (DockIcon.runStateValue) and offer a
/// right-click menu (open/focus, pin/unpin, close). Pins persist in
/// com.jarvis.Settings under `dock.pinned`.
Window {
    id: root
    objectName: "jarvis-dock"
    visible: true
    color: "transparent"
    flags: Qt.FramelessWindowHint

    // The window is taller than the glass bar: `headroom` above it is where
    // a hovered icon grows into and where its name bubble appears; `Theme.edge`
    // below is the gap to the screen edge (and room for the shadow).
    readonly property int headroom: 46
    width: pill.implicitWidth + 40
    height: pill.implicitHeight + headroom + Theme.edge

    LayerShell.Window.scope: "jarvis-dock"
    LayerShell.Window.layer: LayerShell.Window.LayerTop
    LayerShell.Window.anchors: LayerShell.Window.AnchorBottom
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityNone
    // Maximized windows stop just above the glass bar, not above the headroom.
    LayerShell.Window.exclusionZone: pill.implicitHeight + Theme.edge + 6

    // Default pins, used until the user customises `dock.pinned`. `desktopId`
    // is what app.open resolves (Flatpak ids / binary names).
    readonly property var defaultPins: [
        { label: "Firefox",        icon: "firefox",             desktopId: "org.mozilla.firefox" },
        { label: qsTr("Arquivos"), icon: "system-file-manager", desktopId: "org.kde.dolphin" },
        { label: "Zed",            icon: "dev.zed.Zed",         desktopId: "dev.zed.Zed" },
        { label: qsTr("Terminal"), icon: "utilities-terminal",  desktopId: "foot" }
    ]

    RunningAppsModel { id: running }

    // Bumped when pins change so the model rebuilds. SettingsBridge also
    // broadcasts valueChanged, which we fold into the same counter.
    property int _tick: 0
    Connections {
        target: SettingsBridge
        function onValueChanged(key) { if (key === "dock.pinned") root._tick++; }
    }

    // ── Pin storage (JSON array of {desktopId, icon, label} in Settings) ──
    function pins() {
        const raw = SettingsBridge.getString("dock.pinned", "");
        if (!raw || raw.length === 0)
            return defaultPins;
        try {
            const a = JSON.parse(raw);
            return Array.isArray(a) ? a : defaultPins;
        } catch (e) {
            return defaultPins;
        }
    }
    function savePins(arr) {
        SettingsBridge.setString("dock.pinned", JSON.stringify(arr));
        root._tick++;
    }
    function isPinned(desktopId) {
        return pins().some(p => p.desktopId === desktopId);
    }
    function pinApp(desktopId, icon, label) {
        const arr = pins().slice();
        if (!arr.some(p => p.desktopId === desktopId)) {
            arr.push({ desktopId: desktopId, icon: icon || desktopId, label: label || desktopId });
            savePins(arr);
        }
    }
    function unpinApp(desktopId) {
        savePins(pins().filter(p => p.desktopId !== desktopId));
    }

    // ── Model: launcher + pinned + running-unpinned ──────────────────────
    function _lastSeg(s) {
        const i = s.lastIndexOf(".");
        return (i >= 0 ? s.substring(i + 1) : s).toLowerCase();
    }
    function _matchesAny(id, ids) {
        const a = _lastSeg(id);
        return ids.some(p => _lastSeg(p) === a);
    }
    function appTiles() {
        const list = pins().slice();
        const pinnedIds = list.map(p => p.desktopId);
        const runningIds = (running.revision >= 0) ? running.runningAppIds() : [];
        for (const id of runningIds) {
            if (!_matchesAny(id, pinnedIds)) {
                list.push({ desktopId: id, icon: id, label: _lastSeg(id), pinnedTile: false });
            }
        }
        return list;
    }

    // Reactive: re-evaluates when windows open/close (running.revision) or
    // pins change (_tick). The leading reads register the dependencies.
    property var tiles: {
        running.revision;
        root._tick;
        const out = [{ label: qsTr("Apps"), icon: "view-app-grid-symbolic", launcher: true }];
        return out.concat(root.appTiles());
    }

    GlassPanel {
        id: pill
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.edge
        width: implicitWidth
        height: implicitHeight
        implicitWidth: dockRow.implicitWidth + 20
        implicitHeight: Theme.dockIcon + 22
        radius: Theme.radiusDock

        // Frosted glass over the wallpaper. The surface is anchored
        // bottom-centre with no margin, so its screen position is known.
        backdrop: true
        windowX: (Screen.width - root.width) / 2
        windowY: Screen.height - root.height

        RowLayout {
            id: dockRow
            anchors.centerIn: parent
            anchors.verticalCenterOffset: 2
            spacing: 4

            Repeater {
                model: root.tiles
                delegate: DockIcon {
                    iconName: modelData.icon
                    label: modelData.label
                    desktopId: modelData.desktopId || ""
                    isLauncher: modelData.launcher === true
                    isPinned: modelData.launcher === true
                             ? true
                             : (root._tick >= 0 && root.isPinned(modelData.desktopId))
                    runStateValue: modelData.launcher === true
                             ? 0
                             : (running.revision >= 0 ? running.runState(modelData.desktopId) : 0)

                    onActivated: {
                        if (modelData.launcher === true) {
                            ShellBus.openLauncher();
                        } else if (running.runState(modelData.desktopId) > 0) {
                            running.activateApp(modelData.desktopId);
                        } else {
                            ActionBusBridge.dispatch(
                                "app.open", JSON.stringify({ "app": modelData.desktopId }));
                        }
                    }
                    onPinToggle: {
                        if (root.isPinned(modelData.desktopId))
                            root.unpinApp(modelData.desktopId);
                        else
                            root.pinApp(modelData.desktopId, modelData.icon, modelData.label);
                    }
                    onQuitRequested: {
                        // Graceful window close via the Arc 1 window control
                        // (action-bus -> com.jarvis.Shell), selected by app id.
                        ActionBusBridge.dispatch(
                            "window.close", JSON.stringify({ "target": modelData.desktopId }));
                    }
                }
            }

            Rectangle {
                Layout.preferredWidth: 1
                Layout.preferredHeight: Theme.dockIcon - 8
                Layout.alignment: Qt.AlignVCenter
                Layout.leftMargin: 6
                Layout.rightMargin: 6
                color: Theme.separator
            }

            LilithOrb {
                Layout.alignment: Qt.AlignVCenter
            }
        }
    }
}
