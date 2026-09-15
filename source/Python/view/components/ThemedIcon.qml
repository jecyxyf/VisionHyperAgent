import QtQuick
import QtQuick.Effects

Item {
    id: root

    property string source
    property color color: "#545579"

    Image {
        id: image
        anchors.fill: parent
        source: root.source
        sourceSize: Qt.size(Math.max(1, root.width), Math.max(1, root.height))
        fillMode: Image.PreserveAspectFit
        smooth: true
        visible: false
    }

    MultiEffect {
        anchors.fill: parent
        source: image
        autoPaddingEnabled: false
        colorization: 1
        colorizationColor: root.color
    }
}
