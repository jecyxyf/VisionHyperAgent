import QtQuick
import QtQuick.Layouts
import "../ui_items"

Rectangle {
    id: root

    property string label: ""
    property string value: ""

    height: 44
    color: "transparent"

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 16
        anchors.rightMargin: 16
        Text {
            Layout.fillWidth: true
            text: root.label
            color: Theme.muted
            font.pixelSize: Theme.rem * 0.88
            elide: Text.ElideRight
        }
        Text {
            text: root.value
            color: Theme.text
            font.pixelSize: Theme.rem * 0.86
        }
    }

    Rectangle {
        x: 16
        y: parent.height - 1
        width: parent.width - 32
        height: 1
        color: Theme.border
    }
}
