import QtQuick
import QtQuick.Layouts
import "../ui_items"

Rectangle {
    id: root

    property string title: ""
    property string detail: ""

    color: "transparent"

    GradientPanel {
        anchors.fill: parent
        radius: 14
        gradientAngle: 145
        gradientStops: Theme.fieldStops
        strokeColor: Theme.border
        strokeWidth: 1
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 18
        spacing: 16

        RowLayout {
            Layout.fillWidth: true
            Text {
                text: root.title
                color: Theme.text
                font.pixelSize: Theme.rem * 0.88
                font.weight: Font.Medium
            }
            Item { Layout.fillWidth: true }
        }

        Canvas {
            id: plot
            Layout.fillWidth: true
            Layout.fillHeight: true
            onPaint: {
                let ctx = getContext("2d")
                ctx.reset()
                ctx.strokeStyle = String(Theme.grid)
                ctx.lineWidth = 1
                for (let i = 0; i < 5; i++) {
                    let y = i * (height - 1) / 4
                    ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(width, y); ctx.stroke()
                }
                for (let j = 0; j < 7; j++) {
                    let x = j * (width - 1) / 6
                    ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, height); ctx.stroke()
                }
            }
            onWidthChanged: requestPaint()
            onHeightChanged: requestPaint()

            Text {
                anchors.centerIn: parent
                text: root.detail
                color: Theme.subdued
                font.pixelSize: Theme.rem * 0.82
            }
        }

        Text {
            Layout.fillWidth: true
            text: "Epoch"
            color: Theme.subdued
            font.pixelSize: Theme.rem * 0.72
            horizontalAlignment: Text.AlignRight
        }
    }
}
