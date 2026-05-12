import QtQuick
import Jarvis.Greeter

/// 11 vertical bars whose heights drift smoothly with staggered phases.
/// V1 is decorative — real audio reactivity requires capturing the
/// `greeter` user's mic, which means hooking cpal at the greeter
/// level (Phase 2/3 voice-unlock work). Until then the waveform
/// communicates "this mode is listening" visually.
Row {
    id: root
    spacing: 5
    property int barCount: 11
    property int maxHeight: 44
    property int minHeight: 8
    property color barColor: Theme.accent

    Repeater {
        model: root.barCount

        Rectangle {
            width: 4
            radius: 2
            color: root.barColor
            opacity: 0.85

            // Staggered start so the bars don't all rise together.
            property int phase: index * 180

            height: root.minHeight
            anchors.verticalCenter: parent.verticalCenter

            SequentialAnimation on height {
                loops: Animation.Infinite
                running: true
                PauseAnimation { duration: phase }
                NumberAnimation {
                    from: root.minHeight
                    to: root.maxHeight
                    duration: 520
                    easing.type: Easing.InOutSine
                }
                NumberAnimation {
                    from: root.maxHeight
                    to: root.minHeight
                    duration: 520
                    easing.type: Easing.InOutSine
                }
                PauseAnimation { duration: (root.barCount - index) * 80 }
            }
        }
    }
}
