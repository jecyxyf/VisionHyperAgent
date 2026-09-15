import QtQuick
import "../ui_items"

Item {
    id: root

    default property alias content: content.data

    GradientPanel {
        anchors.fill: parent
        radius: 14
        gradientAngle: 145
        gradientStops: Theme.canvasStops
        strokeColor: Theme.border
        strokeWidth: 1
    }

    Canvas {
        id: grid
        anchors.fill: parent
        onPaint: {
            let ctx = getContext("2d")
            ctx.reset()
            ctx.save()
            ctx.beginPath()
            let r = 14
            ctx.moveTo(r, 0)
            ctx.arcTo(width, 0, width, height, r)
            ctx.arcTo(width, height, 0, height, r)
            ctx.arcTo(0, height, 0, 0, r)
            ctx.arcTo(0, 0, width, 0, r)
            ctx.closePath()
            ctx.clip()
            ctx.strokeStyle = String(Theme.grid)
            ctx.lineWidth = 1
            for (let i = 0; i < 36; i++) {
                let x = i * 32
                ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, height); ctx.stroke()
            }
            for (let j = 0; j < 36; j++) {
                let y = j * 32
                ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(width, y); ctx.stroke()
            }
            ctx.restore()
        }
        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()
    }

    Item {
        id: content
        anchors.fill: parent
        clip: true
    }
}
