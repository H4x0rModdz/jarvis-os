import QtQuick
import Jarvis.Greeter

/// Time + date stack used in the top-right of the login screen.
/// Updates every second via Timer; locale formatting is whatever the
/// session reports — V2 will let the user pin a preferred locale in
/// Settings, but the greeter runs before that daemon is up.
Column {
    id: root
    spacing: 4

    property string _time: ""
    property string _date: ""

    function refresh() {
        const now = new Date();
        _time = now.toLocaleTimeString(Qt.locale(), "HH:mm");
        _date = now.toLocaleDateString(Qt.locale(), "dddd, dd MMM");
    }

    Component.onCompleted: refresh()

    Timer {
        interval: 1000
        running: true
        repeat: true
        onTriggered: root.refresh()
    }

    Text {
        anchors.right: parent.right
        text: root._time
        color: Theme.text
        font.pixelSize: 44
        font.weight: Font.Bold
        font.letterSpacing: 2
    }

    Text {
        anchors.right: parent.right
        text: root._date
        color: Theme.textDim
        font.pixelSize: 13
        font.letterSpacing: 1
    }
}
