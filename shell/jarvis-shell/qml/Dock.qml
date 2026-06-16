import QtQuick
import QtQuick.Window
import QtQuick.Layouts
import Jarvis.Shell

/// The macOS-style dock: a centered glass pill with the launcher tile, the
/// pinned apps, any running-but-unpinned apps, then a divider and the Lilith
/// orb. A second top-level layer-shell root (objectName "jarvis-dock");
/// main.cpp anchors it to the bottom edge, centered, on the Top layer.
///
/// Tiles show open vs minimized state (DockIcon.runStateValue) and offer a
/// right-click menu (open/focus, pin/unpin, close). Pins persist in
/// com.jarvis.Settings under `dock.pinned`.
Window {
    id: root
    objectName: "jarvis-dock"
    visible: true
    color: "transparent"
    flags: Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint

    width: pill.implicitWidth + 32
    height: pill.implicitHeight + 28

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
        anchors.centerIn: parent
        implicitWidth: dockRow.implicitWidth + 28
        implicitHeight: 64

        RowLayout {
            id: dockRow
            anchors.centerIn: parent
            spacing: 8

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
                Layout.preferredHeight: 36
                Layout.alignment: Qt.AlignVCenter
                color: Theme.border
            }

            LilithOrb {
                Layout.alignment: Qt.AlignVCenter
            }
        }
    }
}
