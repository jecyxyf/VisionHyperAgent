import QtQuick
import QtQuick.Layouts
import "../ui_items"

Rectangle {
    id: root

    property string title: ""
    property string icon: ""
    default property alias actions: actionsRow.data

    height: 51
    color: "transparent"

    RowLayout {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        height: 34
        spacing: 8

        Rectangle {
            Layout.preferredWidth: 30
            Layout.preferredHeight: 30
            radius: 10
            color: Theme.accentSurface
            border.color: Theme.glassEdge
            border.width: 1

            ThemedIcon {
                anchors.centerIn: parent
                width: 18
                height: 18
                source: root.icon
                color: Theme.accent
            }
        }

        Text {
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignVCenter
            text: root.title
            color: Theme.headerInk
            font.pixelSize: Theme.rem * 1.2
            font.weight: Font.DemiBold
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
        }

        RowLayout {
            id: actionsRow
            spacing: 8
        }
    }

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: 1
        color: Theme.border
    }
}
