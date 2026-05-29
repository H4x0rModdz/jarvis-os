import QtQuick
import Jarvis.Shell

/// One labelled icon on the desktop surface (Desktop.qml). Single click
/// selects (accent highlight), double click activates — the universal
/// desktop convention across Windows / macOS / Linux. Visuals mirror
/// the launcher's AppCell but laid out for a vertical desktop column.
Item {
    id: root
    width: 92
    height: 92

    property string label: ""
    /// Freedesktop icon-theme name, resolved via the `image://theme/`
    /// provider (same path AppCell uses). Falls back to a monogram.
    property string iconName: ""
    property bool selected: false

    signal activated()
    signal selectRequested()

    Rectangle {
        anchors.fill: parent
        radius: Theme.radius - 4
        color: root.selected     ? Qt.rgba(0.486, 0.361, 1.0, 0.28)   // accent tint
             : area.containsMouse ? Qt.rgba(1, 1, 1, 0.10)
             :                      "transparent"
        border.color: root.selected ? Theme.accent : "transparent"
        border.width: 1
        Behavior on color { ColorAnimation { duration: Theme.animFast } }
        Behavior on border.color { ColorAnimation { duration: Theme.animFast } }
    }

    Column {
        anchors.centerIn: parent
        spacing: 6
        width: parent.width - 10

        Item {
            width: 44; height: 44
            anchors.horizontalCenter: parent.horizontalCenter

            Image {
                id: img
                anchors.fill: parent
                fillMode: Image.PreserveAspectFit
                source: root.iconName.length ? "image://theme/" + root.iconName : ""
                smooth: true
                visible: status === Image.Ready
            }

            // Monogram fallback when the icon theme can't resolve the name.
            Text {
                anchors.centerIn: parent
                visible: img.status !== Image.Ready
                text: root.label.length ? root.label.charAt(0).toUpperCase() : "?"
                color: Theme.accent
                font.pixelSize: 22
                font.weight: Font.Bold
            }
        }

        Text {
            width: parent.width
            text: root.label
            color: Theme.text
            font.pixelSize: 12
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
            maximumLineCount: 2
            wrapMode: Text.WordWrap
            // Outline so the label stays legible over any wallpaper.
            style: Text.Outline
            styleColor: Qt.rgba(0, 0, 0, 0.65)
        }
    }

    MouseArea {
        id: area
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.selectRequested()
        onDoubleClicked: root.activated()
    }
}
