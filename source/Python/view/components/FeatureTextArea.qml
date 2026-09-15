import QtQuick
import QtQuick.Layouts
import "../ui_items"

Rectangle {
    id: root

    property string label: ""
    property string placeholder: ""
    property alias value: field.text

    color: "transparent"

    ColumnLayout {
        anchors.fill: parent
        spacing: 6

        Text {
            text: root.label
            color: Theme.muted
            font.pixelSize: Theme.rem * 0.86
        }

        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true

            GradientPanel {
                anchors.fill: parent
                radius: 10
                gradientAngle: 145
                gradientStops: Theme.fieldStops
                strokeColor: Theme.border
                strokeWidth: 1
            }

            TextEdit {
                id: field
                anchors.fill: parent
                anchors.margins: 1
                color: Theme.text
                font.pixelSize: Theme.rem * 0.86
                wrapMode: TextEdit.Wrap
                clip: true
                padding: 10
            }

            Text {
                visible: field.text === "" && !field.activeFocus
                anchors.fill: parent
                anchors.margins: 12
                anchors.topMargin: 10
                text: root.placeholder
                color: Theme.subdued
                font.pixelSize: Theme.rem * 0.82
                wrapMode: Text.Wrap
            }
        }
    }
}
