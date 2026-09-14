import QtQuick
import "../ui_items"

Item {
    id: root

    property bool inset: false
    property var gradientStops: Theme.glassStops
    property color strokeColor: Theme.glassEdge
    default property alias content: content.data

    GradientPanel {
        anchors.fill: parent
        radius: 22
        gradientAngle: 145
        gradientStops: root.inset ? [0, Theme.surface, 1, Theme.surface] : root.gradientStops
        strokeColor: root.inset ? Theme.border : root.strokeColor
        strokeWidth: 1
        shadowColor: root.inset ? "transparent" : Theme.shadow
        shadowBlur: root.inset ? 0 : 30
        shadowOffsetY: root.inset ? 0 : 8
        topEdgeHighlight: !root.inset
        bottomEdgeHighlight: !root.inset
        edgeStops: Theme.edgeHighlightStops
    }

    Item {
        id: content
        anchors.fill: parent
    }
}
