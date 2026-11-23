import QtQuick
import QtQuick.Effects
import Jarvis.Shell

/// A monochrome status glyph from the icon theme (`*-symbolic` names),
/// tinted to `color`. The menu bar, Control Centre and menus use it for
/// Wi-Fi, battery, sound and the like.
///
/// Symbolic icons ship as a single dark shape; the theme does not know our
/// text colour, so the tint is applied here. The source is static, so the
/// effect renders once per change of icon or colour — not per frame.
///
/// When the theme has no icon by that name the item shows `fallback` (a
/// character), so a missing icon degrades to a legible mark, not a hole.
Item {
    id: root
    property string name: ""
    property color color: Theme.text
    property int size: 16
    property string fallback: ""

    implicitWidth: size
    implicitHeight: size

    Image {
        id: img
        anchors.fill: parent
        source: root.name.length ? "image://theme/" + root.name : ""
        sourceSize: Qt.size(root.size * 2, root.size * 2)
        smooth: true
        visible: false
    }
    MultiEffect {
        anchors.fill: img
        source: img
        visible: img.status === Image.Ready
        colorization: 1.0
        colorizationColor: root.color
        brightness: 1.0
    }
    Text {
        anchors.centerIn: parent
        visible: img.status !== Image.Ready && root.fallback.length > 0
        text: root.fallback
        color: root.color
        font.pixelSize: root.size
    }
}
