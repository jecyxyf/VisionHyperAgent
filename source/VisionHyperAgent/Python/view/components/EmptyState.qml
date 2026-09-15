import QtQuick
import QtQuick.Layouts
import "../ui_items"

Rectangle {
    id: root

    property string icon: ""
    property string title: ""
    property string detail: ""

    color: "transparent"

    ColumnLayout {
        anchors.fill: parent
        spacing: 12
        enabled: false

        Item { Layout.fillHeight: true }

        Item {
            Layout.fillWidth: true
            Layout.preferredHeight: 60

            GradientPanel {
                anchors.horizontalCenter: parent.horizontalCenter
                width: 60
                height: 60
                radius: 18
                gradientAngle: 120
                gradientStops: Theme.selectionStops
                strokeColor: Theme.glassEdge
                strokeWidth: 1

                ThemedIcon {
                    anchors.centerIn: parent
                    width: 26
                    height: 26
                    source: root.icon
                    color: Theme.accent
                }
            }
        }

        Text {
            Layout.fillWidth: true
            text: root.title
            color: Theme.muted
            font.pixelSize: Theme.rem
            font.weight: Font.Medium
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
        }

        Text {
            visible: root.detail !== ""
            Layout.fillWidth: true
            text: root.detail
            color: Theme.subdued
            font.pixelSize: Theme.rem * 0.82
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
        }

        Item { Layout.fillHeight: true }
    }
}
