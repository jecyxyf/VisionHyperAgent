import QtQuick

Item {
    id: root

    property color shadowColor: "transparent"
    property real shadowBlur: 0
    property real shadowOffsetY: 0
    property real panelWidth: 0
    property real panelHeight: 0
    property real panelRadius: 0

    ShaderEffect {
        id: effect

        anchors.fill: parent
        visible: root.shadowColor.a > 0 && root.shadowBlur > 0

        property color shadowColor: root.shadowColor
        property real blurRadius: Math.max(0.01, root.shadowBlur / 2)
        property real offsetY: root.shadowOffsetY
        property real panelWidth: root.panelWidth
        property real panelHeight: root.panelHeight
        property real panelRadius: root.panelRadius
        property real effectWidth: root.width
        property real effectHeight: root.height
        fragmentShader: "GradientShadow.qsb"
    }
}
