import QtQuick

Item {
    id: root
    implicitWidth: label.implicitWidth
    implicitHeight: label.implicitHeight

    property string time: "--:--:--"

    Timer {
        interval: 500
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: root.time = Qt.formatTime(new Date(), "HH:mm:ss")
    }

    Text {
        id: label
        anchors.centerIn: parent
        text: root.time
        // Monospaced as a poor-man's tabular figures — Qt 6.6+ adds
        // font.features for true OpenType `tnum`; we target 6.4.
        font.family: "Inter Mono, JetBrains Mono, monospace"
        font.pixelSize: 22
        font.weight: Font.Medium
        color: Theme.text
    }
}
