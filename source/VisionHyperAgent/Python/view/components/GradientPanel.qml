import QtQuick

Item {
    id: root

    default property alias content: content.data

    property real radius: 0
    property real gradientAngle: 145
    property var gradientStops: []
    property color strokeColor: "transparent"
    property real strokeWidth: 0
    property color shadowColor: "transparent"
    property real shadowBlur: 0
    property real shadowOffsetY: 0
    property bool topEdgeHighlight: false
    property bool bottomEdgeHighlight: false
    property var edgeStops: []
    property real edgeInset: 18

    readonly property real shadowPadding: shadowBlur * 2

    onGradientStopsChanged: plate.requestPaint()
    onStrokeColorChanged: plate.requestPaint()

    GradientShadow {
        x: -root.shadowPadding
        y: -root.shadowPadding
        width: root.width + root.shadowPadding * 2
        height: root.height + root.shadowPadding * 2
        shadowColor: root.shadowColor
        shadowBlur: root.shadowBlur
        shadowOffsetY: root.shadowOffsetY
        panelWidth: root.width
        panelHeight: root.height
        panelRadius: root.radius
    }

    Canvas {
        id: plate
        anchors.fill: parent

        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()

        onPaint: {
            let ctx = getContext("2d")
            ctx.reset()
            let w = width, h = height
            if (w <= 0 || h <= 0)
                return

            // 阴影由 GradientShadow 的轻量 SDF shader 绘制，避免 Canvas 软件模糊卡死事件循环。
            ctx.beginPath()
            addRoundRect(ctx, 0, 0, w, h, root.radius)
            ctx.fillStyle = makeGradient(ctx, root.gradientAngle, root.gradientStops)
            ctx.fill()

            if (root.strokeWidth > 0 && root.strokeColor.a > 0) {
                let inset = root.strokeWidth / 2
                ctx.beginPath()
                addRoundRect(ctx, inset, inset, w - root.strokeWidth, h - root.strokeWidth, Math.max(0, root.radius - inset))
                ctx.strokeStyle = String(root.strokeColor)
                ctx.lineWidth = root.strokeWidth
                ctx.stroke()
            }

            if (root.topEdgeHighlight)
                drawEdge(ctx, root.edgeInset, 1, w - root.edgeInset * 2)
            if (root.bottomEdgeHighlight)
                drawEdge(ctx, root.edgeInset, h - 2, w - root.edgeInset * 2)
        }

        function drawEdge(ctx, x, y, lineW) {
            if (lineW <= 0 || root.edgeStops.length === 0)
                return
            ctx.beginPath()
            ctx.moveTo(x, y)
            ctx.lineTo(x + lineW, y)
            ctx.strokeStyle = makeLinear(ctx, x, y, x + lineW, y, root.edgeStops)
            ctx.lineWidth = 1
            ctx.stroke()
        }

        function makeGradient(ctx, angleDeg, stops) {
            // CSS 风格角度：0deg 向上，顺时针。
            let a = (angleDeg * Math.PI) / 180
            let cx = width / 2, cy = height / 2
            let dx = Math.sin(a), dy = -Math.cos(a)
            let len = Math.abs(width * dx) + Math.abs(height * dy)
            return makeLinear(ctx, cx - dx * len / 2, cy + dy * len / 2,
                                     cx + dx * len / 2, cy + dy * len / 2, stops)
        }

        function makeLinear(ctx, x1, y1, x2, y2, stops) {
            let gradient = ctx.createLinearGradient(x1, y1, x2, y2)
            for (let i = 0; i < stops.length; i += 2)
                gradient.addColorStop(stops[i], stops[i + 1])
            return gradient
        }

        function addRoundRect(ctx, x, y, w, h, r) {
            r = Math.min(r, w / 2, h / 2)
            ctx.moveTo(x + r, y)
            ctx.arcTo(x + w, y, x + w, y + h, r)
            ctx.arcTo(x + w, y + h, x, y + h, r)
            ctx.arcTo(x, y + h, x, y, r)
            ctx.arcTo(x, y, x + w, y, r)
            ctx.closePath()
        }
    }

    Item {
        id: content
        anchors.fill: parent
    }
}
