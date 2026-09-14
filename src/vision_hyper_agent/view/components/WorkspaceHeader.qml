import QtQuick
import QtQuick.Layouts
import "../ui_items"

Rectangle {
    id: root

    property string title: ""
    property string icon: ""
    default property alias actions: actionsRow.data

    height: 51

    ColumnLayout {
        anchors.fill: parent
        spacing: 16

        RowLayout {
            Layout.fillWidth: true
            Layout.preferredHeight: 34
            spacing: 8

            ThemedIcon {
                Layout.preferredWidth: 18
                Layout.preferredHeight: 18
                source: root.icon
                color: Theme.accent
            }

            Text {
                Layout.fillWidth: true
                text: root.title
                color: Theme.text
                font.pixelSize: Theme.rem * 1.1
                font.weight: Font.DemiBold
                elide: Text.ElideRight
            }

            RowLayout {
                id: actionsRow
                spacing: 8
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 1
            color: Theme.border
        }
    }
}
