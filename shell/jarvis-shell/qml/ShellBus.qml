pragma Singleton
import QtQuick

/// Cross-surface UI intent bus.
///
/// The shell now renders three independent layer-shell root windows
/// (top bar, dock, desktop) in one engine. They can't reference each
/// other's QML objects, but they share this singleton. The dock emits
/// an intent here; Main.qml — which still owns every popup window
/// (Launcher, SettingsPanel, NotificationDrawer, LilithPopup…) —
/// listens and opens the right one.
///
/// Pure signals, no state: this is a message channel, not a store.
QtObject {
    /// Toggle the Lilith conversation popup (dock orb click).
    signal toggleLilith()
    /// Open the app launcher / Launchpad grid (dock Launchpad tile).
    signal openLauncher()
    /// Open the preferences panel (Jarvis menu → Configurações).
    signal openSettings()
    /// Open the notification history drawer.
    signal openNotifications()
}
