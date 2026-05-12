import QtQuick
import QtQuick.Window
import Jarvis.Greeter

/// Greeter root window — fullscreen on the primary output, dark
/// background with the login card floating in the centre. The window
/// is the only thing on the screen; labwc isn't running yet (greetd
/// will exec it after start_session succeeds), so we don't compete
/// for focus and don't need wlr-layer-shell.
Window {
    id: root
    visible: true
    width: 1280
    height: 800
    title: qsTr("Jarvis OS")
    color: Theme.background

    // Maximise on first paint. greetd usually spawns us under a
    // compositor that already gives us the full output, but if not
    // we at least try to cover everything.
    Component.onCompleted: {
        showFullScreen();
    }

    LoginScreen {
        anchors.centerIn: parent
    }
}
