import QtQuick
import QtQuick.Layouts
import "../ui_items"

Rectangle {
    id: root

    property string title: ""
    property string icon: ""
    default property alias actions: actionsRow.data

    height: 51

    RowLayout {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        height: 34
        spacing: 8

        ThemedIcon {
            Layout.preferredWidth: 18
            Layout.preferredHeight: 18
            source: root.icon
            color: Theme.accent
        }

        Text {
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignVCenter
            text: root.title
            color: Theme.text
            font.pixelSize: Theme.rem * 1.1
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
