import QtQuick
import "../ui_items"

Rectangle {
    id: root

    // entry: { name, format, path }
    property var entry: ({ name: "", format: "", path: "" })
    property bool header: false
    readonly property real nameWidth: Math.max(0, (width - 118) * 0.4)

    color: "transparent"

    Text {
        x: 12; y: 0
        width: root.nameWidth; height: parent.height
        text: root.entry.name
        color: root.header ? Theme.muted : Theme.text
        font.pixelSize: root.header ? Theme.rem * 0.83 : Theme.rem * 0.9
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }

    Text {
        x: 28 + root.nameWidth; y: 0
        width: 62; height: parent.height
        text: root.entry.format
        color: Theme.muted
        font.pixelSize: Theme.rem * 0.8
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }

    Text {
        x: 106 + root.nameWidth; y: 0
        width: Math.max(0, root.width - x - 12); height: parent.height
        text: root.entry.path
        color: root.header ? Theme.muted : Theme.subdued
        font.pixelSize: Theme.rem * 0.8
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }
}
