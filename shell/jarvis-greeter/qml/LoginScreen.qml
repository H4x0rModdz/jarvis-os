import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import Jarvis.Greeter

/// Three-up SwipeView holding Standard / Lilith / Focus modes.
/// Switching mechanisms:
///   - swipe gesture (touchpad / touch)
///   - Left / Right arrow keys
///   - mouse wheel anywhere on the screen
///   - clicking a specific dot in the indicator row
///
/// Auth always flows through GreetdClient — the mode is purely
/// presentation. See ADR 0012 + jarvis_login_screen.md for the
/// design rationale.
Item {
    id: root
    property string username: GreeterState.username
    signal infoMessage(string text)
    signal errorMessage(string text)

    focus: true
    Keys.onLeftPressed: swipe.currentIndex = Math.max(0, swipe.currentIndex - 1)
    Keys.onRightPressed: swipe.currentIndex = Math.min(2, swipe.currentIndex + 1)

    // Persist the user's preferred mode so the next boot opens
    // straight to it. GreeterState writes to QSettings on persist.
    Connections {
        target: swipe
        function onCurrentIndexChanged() {
            GreeterState.modeIndex = swipe.currentIndex;
            GreeterState.persist();
        }
    }

    // Bubble GreetdClient errors up to Main.qml's Toast.
    Connections {
        target: GreetdClient
        function onStateChanged() {
            if (GreetdClient.error.length > 0) {
                root.errorMessage(GreetdClient.error);
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 18

        Item { Layout.fillHeight: true }

        SwipeView {
            id: swipe
            Layout.fillWidth: true
            Layout.preferredHeight: 560
            clip: true
            currentIndex: GreeterState.modeIndex

            StandardMode {
                username: root.username
                onInfoMessage: function(t) { root.infoMessage(t); }
            }
            LilithMode {
                username: root.username
                onInfoMessage: function(t) { root.infoMessage(t); }
            }
            FocusMode {
                username: root.username
            }
        }

        ModeIndicator {
            Layout.alignment: Qt.AlignHCenter
            count: 3
            currentIndex: swipe.currentIndex
            onIndexChosen: function(i) { swipe.currentIndex = i; }
        }

        // Mode label below the indicators — matches the template
        // ("01 · STANDARD     Secure and familiar…").
        Text {
            Layout.alignment: Qt.AlignHCenter
            text: {
                switch (swipe.currentIndex) {
                    case 0: return qsTr("01 · STANDARD     Secure and familiar. Everything you need, right where you expect it.");
                    case 1: return qsTr("02 · LILITH     An intelligent interface. Welcome to the conversation.");
                    case 2: return qsTr("03 · FOCUS     Minimal. Fast. Distraction-free. For when you just want to get things done.");
                }
                return "";
            }
            color: Theme.textDim
            font.pixelSize: 11
            font.letterSpacing: 1
            wrapMode: Text.WordWrap
            horizontalAlignment: Text.AlignHCenter
            Layout.maximumWidth: parent.width - 80
        }

        Item { Layout.fillHeight: true }
    }

    // Wheel anywhere on the screen switches modes — desktop users
    // without a touchpad get the same affordance as gesture devices.
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.NoButton
        propagateComposedEvents: true
        onWheel: function(wheel) {
            if (wheel.angleDelta.y > 0 || wheel.angleDelta.x > 0) {
                swipe.currentIndex = Math.max(0, swipe.currentIndex - 1);
            } else {
                swipe.currentIndex = Math.min(2, swipe.currentIndex + 1);
            }
        }
    }
}
