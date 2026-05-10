import QtQuick
import Jarvis.Shell

/// One tile in the launcher grid. Hovers light up, click fires `clicked`.
Rectangle {
    id: root
    radius: Theme.radius - 4
    color: area.containsMouse ? Qt.rgba(1, 1, 1, 0.08) : "transparent"
    border.color: area.containsMouse ? Theme.border : "transparent"
    border.width: 1
    Behavior on color { ColorAnimation { duration: Theme.animFast } }
    Behavior on border.color { ColorAnimation { duration: Theme.animFast } }

    property string name: ""
    property string comment: ""
    property string iconSource: ""
    signal clicked()

    Column {
        anchors.centerIn: parent
        spacing: 6
        width: parent.width - 12

        // Icon — Qt's Image with QIcon::fromTheme uri-resolver doesn't
        // exist in pure QML, so we use the `image://theme/<name>` provider
        // baked into Qt 6 via QQuickIconImageProvider when present, falling
        // back to the literal path if iconSource looks absolute.
        Item {
            width: 48; height: 48
            anchors.horizontalCenter: parent.horizontalCenter

            Rectangle {
                anchors.fill: parent
                radius: 8
                color: Qt.rgba(1, 1, 1, 0.04)
                visible: !appIcon.source.toString().length || appIcon.status !== Image.Ready
            }

            Image {
                id: appIcon
                anchors.fill: parent
                anchors.margins: 4
                fillMode: Image.PreserveAspectFit
                source: root.iconSource.startsWith("/") ? "file://" + root.iconSource
                      : root.iconSource.length          ? "image://theme/" + root.iconSource
                      : ""
                smooth: true
                visible: status === Image.Ready
            }

            // Fallback monogram when the theme can't resolve the icon.
            Text {
                anchors.centerIn: parent
                visible: appIcon.status !== Image.Ready
                text: root.name.length > 0 ? root.name.charAt(0).toUpperCase() : "?"
                color: Theme.accent
                font.pixelSize: 22
                font.weight: Font.Bold
            }
        }

        Text {
            width: parent.width
            text: root.name
            color: Theme.text
            font.pixelSize: 12
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
            maximumLineCount: 2
            wrapMode: Text.WordWrap
        }
    }

    MouseArea {
        id: area
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        hoverEnabled: true
        onClicked: root.clicked()
    }
}
