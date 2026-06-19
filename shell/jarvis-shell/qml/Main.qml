import QtQuick
import QtQuick.Window
import Jarvis.Shell

/// Top-level root: the macOS-style **menu bar** pinned to the top edge,
/// plus every sibling popup window the shell owns (Launcher, dialogs,
/// panels, notification surfaces, LilithPopup, the Jarvis menu). The
/// dock and the desktop-icon surface are separate roots (Dock.qml /
/// Desktop.qml); main.cpp anchors all three by objectName.
///
/// Cross-surface UI intents arrive via the ShellBus singleton: the dock
/// emits, this root opens the matching popup (which only it can reach).
Window {
    id: root
    objectName: "jarvis-topbar"
    visible: true
    width: 1280
    height: Theme.topBarHeight
    color: "transparent"
    flags: Qt.FramelessWindowHint

    // ── The menu bar ──────────────────────────────────────────────────
    TopBar {
        id: topbar
        anchors.fill: parent

        onJarvisMenuRequested: {
            const s = Qt.application.screens[0];
            if (s) jarvisMenu.openAt(s.virtualX + 6, s.virtualY + Theme.topBarHeight + 2);
        }
        onNetworksRequested: connectivityPanel.requestOpen()
        onBluetoothRequested: bluetoothPanel.requestOpen()
        onNotificationsRequested: notificationDrawer.requestOpen()
        onSettingsRequested: settingsPanel.requestOpen()
    }

    // ── Cross-surface intents from the dock ───────────────────────────
    Connections {
        target: ShellBus
        function onToggleLilith() { lilithPopup.toggle(); }
        function onOpenLauncher() { launcher.visible ? launcher.close() : launcher.open(); }
        function onOpenSettings() { settingsPanel.requestOpen(); }
        function onOpenNotifications() { notificationDrawer.requestOpen(); }
    }

    // ── Update-check feedback ─────────────────────────────────────────
    // The Jarvis menu's "Atualização do sistema" calls
    // UpdaterBridge.checkNow(). When an update is staged the splash
    // surfaces it via osUpdateAvailable; the no-update / error cases have
    // no splash state, so we tell the user through a notification toast —
    // otherwise the click looks like it did nothing.
    Connections {
        target: UpdaterBridge
        function onUpToDate() {
            ActionBusBridge.dispatch("system.notify", JSON.stringify({
                "title": "Jarvis OS",
                "body": qsTr("Seu sistema já está atualizado.")
            }));
        }
        function onCheckFailed(message) {
            ActionBusBridge.dispatch("system.notify", JSON.stringify({
                "title": "Jarvis OS",
                "body": qsTr("Não foi possível verificar atualizações.")
            }));
        }
    }

    // ── Voice → Lilith ────────────────────────────────────────────────
    // Wake-word and push-to-talk transcripts feed Lilith as if typed —
    // same audit path, same permission gating, same popup. (Errors are
    // surfaced inside LilithPopup now, not here.)
    Connections {
        target: VoiceBridge
        // Hotword match: remainder present → one-shot command; empty →
        // engage the mic so the user speaks the command body.
        function onWakeWordTriggered(fullTranscript, remainder) {
            if (remainder.length > 0) {
                LilithBridge.send(remainder);
            } else {
                VoiceBridge.toggle();
            }
        }
        // Push-to-talk / post-wake transcript → hand to Lilith.
        function onLastTranscriptChanged() {
            const t = VoiceBridge.lastTranscript.trim();
            if (t.length > 0 && !LilithBridge.busy) {
                LilithBridge.send(t);
            }
        }
    }

    // ── Auto-speak Lilith replies ─────────────────────────────────────
    // Whatever Lilith says also comes out the speakers, unless the user
    // turned it off (voice.tts_enabled). No-ops when the voice daemon
    // isn't reachable.
    Connections {
        target: LilithBridge
        function onReplyReceived(replyText, action, resultJson) {
            const enabled = SettingsBridge.getBool("voice.tts_enabled", true);
            if (enabled && VoiceBridge.reachable && replyText.trim().length > 0) {
                VoiceBridge.speak(replyText);
            }
        }
    }

    // ── Sibling popups (owned here, reachable from any surface) ────────

    // The Jarvis menu (Apple-menu analogue) drops from the logo.
    JarvisMenu {
        id: jarvisMenu
        onAboutRequested: aboutDialog.requestOpen()
        onSettingsRequested: settingsPanel.requestOpen()
    }

    AboutDialog { id: aboutDialog }

    Launcher {
        id: launcher
        onVisibleChanged: { /* nothing to refocus — the bar input is gone */ }
    }

    // Approval dialog — opens on top when PermissionBridge has a pending
    // request, closes on user decision.
    ApprovalDialog {}

    // First-boot updater splash.
    UpdaterSplash {}

    // Preferences panel.
    SettingsPanel {
        id: settingsPanel
        onDisplayRequested: displayPanel.requestOpen()
    }

    DisplayPanel { id: displayPanel }

    // Toast for incoming notifications.
    NotificationToast {}

    // History drawer.
    NotificationDrawer { id: notificationDrawer }

    // Conversation popup — floats above the dock. Auto-opens when Lilith
    // goes busy; opened explicitly by the dock's Lilith orb (toggle()).
    LilithPopup { id: lilithPopup }

    // Embodied avatar — Lilith's 3D companion (ADR 0028). A draggable,
    // always-on-top corner window that reacts to her state/emotion/speech.
    // Clicking it opens the conversation popup (same as the dock orb).
    //
    // Loaded through a Loader ON PURPOSE: LilithAvatar imports QtQuick3D, which
    // is absent on a runtime base that predates qt6-qtquick3d. Instantiating it
    // directly would make the import failure cascade up and take the whole
    // top-bar root down with it — the bar, the Lilith chat and every panel
    // (settings/wifi/…) are children here. A Loader contains that failure: a
    // broken/missing QtQuick3D just leaves the avatar absent; the rest of the
    // shell loads normally. The avatar appears once the runtime ships Quick3D.
    Loader {
        id: avatarLoader
        source: Qt.resolvedUrl("LilithAvatar.qml")
        asynchronous: true
        onStatusChanged: {
            if (status === Loader.Error)
                console.warn("Lilith avatar failed to load (QtQuick3D missing?) — shell continues without it");
        }
    }

    // Wi-Fi panel.
    ConnectivityPanel { id: connectivityPanel }

    // Bluetooth panel.
    BluetoothPanel { id: bluetoothPanel }

    // First-boot wizard — self-gates via QSettings; opens once.
    FirstBootWizard {
        id: firstBootWizard
        Component.onCompleted: maybeOpen()
    }
}
