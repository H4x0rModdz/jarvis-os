import QtQuick

/// Minimal line graph for the desktop HUD (eDEX-style). Draws `values`
/// (a list of numbers, oldest → newest) as a polyline scaled to the widget.
/// Repaints whenever the data or size changes. `maxValue <= 0` auto-scales
/// to the peak in the window.
Canvas {
    id: root

    property var values: []
    property real maxValue: 100
    property color stroke: "#18ffff"
    property real lineWidth: 1.5
    /// When true, fills the area under the line faintly (traffic look).
    property bool fill: false

    onValuesChanged: requestPaint()
    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()

    onPaint: {
        const ctx = getContext("2d");
        ctx.reset();
        const n = values ? values.length : 0;
        if (n < 2 || width <= 0 || height <= 0) return;

        let mx = maxValue;
        if (mx <= 0) {
            mx = 1;
            for (let i = 0; i < n; i++) mx = Math.max(mx, values[i]);
        }

        const pt = function (i) {
            return {
                x: (i / (n - 1)) * width,
                y: height - (Math.min(values[i], mx) / mx) * height
            };
        };

        if (fill) {
            ctx.beginPath();
            ctx.moveTo(0, height);
            for (let i = 0; i < n; i++) { const p = pt(i); ctx.lineTo(p.x, p.y); }
            ctx.lineTo(width, height);
            ctx.closePath();
            ctx.fillStyle = Qt.rgba(stroke.r, stroke.g, stroke.b, 0.12);
            ctx.fill();
        }

        ctx.strokeStyle = stroke;
        ctx.lineWidth = lineWidth;
        ctx.beginPath();
        for (let i = 0; i < n; i++) {
            const p = pt(i);
            if (i === 0) ctx.moveTo(p.x, p.y); else ctx.lineTo(p.x, p.y);
        }
        ctx.stroke();
    }
}
