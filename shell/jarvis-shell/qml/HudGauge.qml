import QtQuick

/// Car-instrument-cluster gauge for the desktop HUD (ADR 0031) — a 270° dial
/// with tick marks, a redline zone, a sweeping needle and a digital readout in
/// the middle. Used for CPU / memory / disk on the SYSTEM panel.
///
/// Drawn with `Canvas` rather than QtQuick.Shapes + MultiEffect on purpose:
/// the glow here is Canvas 2D's native `shadowBlur`, which costs one pass
/// instead of a multi-pass blur shader. LilithOS routinely runs on software
/// rendering (a VM with no GPU), where a shader post-process on every frame is
/// exactly what makes the shell stutter. Same look, a fraction of the cost, and
/// no extra QML import to go missing at runtime.
///
/// Repaints only when the animated value (or the size) changes — not per frame.
Item {
    id: root

    /// Current reading and the top of the scale. `value` is clamped into
    /// [0, maxValue]; everything else is derived from the ratio.
    property real value: 0
    property real maxValue: 100
    /// Big readout in the middle, and the caption under it. The caller formats
    /// the text — the gauge never guesses units.
    property string text: ""
    property string label: ""
    property color accent: "#18ffff"
    property color danger: "#ff5a5a"
    /// Fraction of the scale (0..1) where the red zone starts. >= 1 disables it.
    property real redlineFrom: 0.85
    /// Soft outer glow on the value arc + needle. Off on the "reduced"/"off"
    /// effect tiers (see Desktop.qml's hudEffects).
    property bool glow: true
    /// Draw the needle. Off gives a plain ring, which is cheaper still.
    property bool needle: true
    property real thickness: Math.max(4, Math.min(width, height) * 0.075)

    implicitWidth: 120
    implicitHeight: 120

    readonly property real ratio: maxValue > 0
        ? Math.max(0, Math.min(1, value / maxValue)) : 0
    readonly property bool inRedline: ratio >= redlineFrom
    readonly property color live: inRedline ? danger : accent

    /// What the needle is actually drawn at (0..1). Big moves are eased over
    /// 250ms; small ones snap.
    ///
    /// The snap is a performance decision, not a stylistic one. Qt Quick has no
    /// partial updates — ANY animation repaints the WHOLE window, and this HUD
    /// lives in a fullscreen one. Easing every 1 Hz sample meant ~15 full-screen
    /// repaints per second per gauge, for ever. At 5-8 megapixels that is the
    /// cost that scales with the display, and it is measurable: the same scene
    /// is smooth at 1280x800 and drags at 4K. Idle CPU jitter of a couple of
    /// percent does not need to be animated; a real jump still is.
    property real displayRatio: 0
    property bool easeNextChange: false
    Behavior on displayRatio {
        enabled: root.easeNextChange
        NumberAnimation { duration: 250; easing.type: Easing.OutCubic }
    }
    onRatioChanged: {
        if (bootSweep.running) return;
        easeNextChange = Math.abs(ratio - displayRatio) >= 0.08;
        displayRatio = ratio;
    }

    // Instrument-cluster startup sweep: the needle runs to full scale and back
    // before settling on the live reading, the way a car's cluster self-tests.
    // One-shot on load — never a looping animation.
    SequentialAnimation {
        id: bootSweep
        running: true
        NumberAnimation { target: root; property: "displayRatio"; to: 1.0; duration: 620; easing.type: Easing.OutCubic }
        NumberAnimation { target: root; property: "displayRatio"; to: 0.0; duration: 380; easing.type: Easing.InOutQuad }
        ScriptAction { script: root.displayRatio = root.ratio }
    }

    onGlowChanged: canvas.requestPaint()
    onDisplayRatioChanged: canvas.requestPaint()
    onLiveChanged: canvas.requestPaint()

    Canvas {
        id: canvas
        anchors.fill: parent
        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()

        // 270° dial opening at the bottom: from 135° round to 405° (=45°).
        readonly property real startAngle: Math.PI * 0.75
        readonly property real sweepAngle: Math.PI * 1.5

        onPaint: {
            const ctx = getContext("2d");
            ctx.reset();

            const w = width, h = height;
            if (w <= 0 || h <= 0) return;
            const cx = w / 2, cy = h / 2;
            const t = root.thickness;
            const r = Math.min(w, h) / 2 - t * 0.5 - 6; // leave room for ticks
            if (r <= 0) return;

            const a0 = startAngle;
            const span = sweepAngle;

            // ── Tick marks (major every 10%, minor every 2.5%) ──────────────
            ctx.lineCap = "butt";
            for (let i = 0; i <= 40; i++) {
                const major = (i % 4) === 0;
                const frac = i / 40;
                const a = a0 + span * frac;
                const outer = r + t * 0.5 + 4;
                const len = major ? 5 : 2.5;
                ctx.beginPath();
                ctx.moveTo(cx + Math.cos(a) * outer, cy + Math.sin(a) * outer);
                ctx.lineTo(cx + Math.cos(a) * (outer - len), cy + Math.sin(a) * (outer - len));
                ctx.strokeStyle = frac >= root.redlineFrom
                    ? Qt.rgba(root.danger.r, root.danger.g, root.danger.b, major ? 0.85 : 0.45)
                    : Qt.rgba(1, 1, 1, major ? 0.38 : 0.16);
                ctx.lineWidth = major ? 1.6 : 1;
                ctx.stroke();
            }

            // ── Track ───────────────────────────────────────────────────────
            ctx.lineCap = "round";
            ctx.beginPath();
            ctx.arc(cx, cy, r, a0, a0 + span, false);
            ctx.strokeStyle = Qt.rgba(1, 1, 1, 0.07);
            ctx.lineWidth = t;
            ctx.stroke();

            // Redline zone painted onto the track.
            if (root.redlineFrom < 1) {
                ctx.beginPath();
                ctx.arc(cx, cy, r, a0 + span * root.redlineFrom, a0 + span, false);
                ctx.strokeStyle = Qt.rgba(root.danger.r, root.danger.g, root.danger.b, 0.22);
                ctx.lineWidth = t;
                ctx.stroke();
            }

            // ── Value arc ───────────────────────────────────────────────────
            const v = Math.max(0, Math.min(1, root.displayRatio));
            if (v > 0.001) {
                if (root.glow) {
                    ctx.shadowBlur = t * 1.6;
                    ctx.shadowColor = root.live;
                }
                // Two-stop sweep (dim → live) drawn as short segments, so the
                // arc reads as a gradient without a conical-gradient shader.
                const segments = Math.max(1, Math.round(v * 32));
                for (let s = 0; s < segments; s++) {
                    const f0 = (s / segments) * v;
                    const f1 = ((s + 1) / segments) * v;
                    const k = segments > 1 ? s / (segments - 1) : 1;
                    ctx.beginPath();
                    // Overlap by a hair so the segments don't seam.
                    ctx.arc(cx, cy, r, a0 + span * f0, a0 + span * f1 + 0.004, false);
                    ctx.strokeStyle = Qt.rgba(root.live.r, root.live.g, root.live.b,
                                              0.45 + 0.55 * k);
                    ctx.lineWidth = t;
                    ctx.stroke();
                }
                ctx.shadowBlur = 0;
            }

            // ── Needle ──────────────────────────────────────────────────────
            if (root.needle) {
                const a = a0 + span * v;
                if (root.glow) {
                    ctx.shadowBlur = 8;
                    ctx.shadowColor = root.live;
                }
                ctx.beginPath();
                ctx.moveTo(cx - Math.cos(a) * r * 0.16, cy - Math.sin(a) * r * 0.16);
                ctx.lineTo(cx + Math.cos(a) * (r - t * 0.65), cy + Math.sin(a) * (r - t * 0.65));
                ctx.strokeStyle = root.live;
                ctx.lineWidth = 1.8;
                ctx.lineCap = "round";
                ctx.stroke();
                ctx.shadowBlur = 0;

                // Hub.
                ctx.beginPath();
                ctx.arc(cx, cy, Math.max(2.5, r * 0.055), 0, Math.PI * 2, false);
                ctx.fillStyle = root.live;
                ctx.fill();
            }
        }
    }

    // Digital readout. Real Text items rather than Canvas fillText — proper
    // font hinting and it doesn't force a repaint when only the number changes.
    Column {
        anchors.centerIn: parent
        anchors.verticalCenterOffset: root.height * 0.06
        spacing: 0

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: root.text
            color: root.live
            font.family: "monospace"
            font.pixelSize: Math.max(11, root.height * 0.17)
            font.bold: true
            Behavior on color { ColorAnimation { duration: 250 } }
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: root.label
            color: Qt.rgba(1, 1, 1, 0.42)
            font.family: "monospace"
            font.pixelSize: Math.max(8, root.height * 0.085)
            font.letterSpacing: 1.5
        }
    }
}
