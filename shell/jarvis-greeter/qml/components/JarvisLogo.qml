import QtQuick

/// The Jarvis "eye" logo image, baked into the binary at
/// `qrc:/branding/jarvis-os-default-icon.png`. Replaces the V1
/// hand-drawn Rectangle + RotationAnimator rings now that we have
/// the proper art.
///
/// `size` controls the square render dimension; the PNG already
/// includes the JARVIS / OS typography so callers don't lay it out
/// separately.
Item {
    id: root
    property real size: 110

    implicitWidth: size
    implicitHeight: size

    Image {
        anchors.fill: parent
        source: "qrc:/branding/jarvis-os-default-icon.png"
        sourceSize.width: parent.size * 2   // 2x for HiDPI crispness
        sourceSize.height: parent.size * 2
        smooth: true
        fillMode: Image.PreserveAspectFit
    }
}
