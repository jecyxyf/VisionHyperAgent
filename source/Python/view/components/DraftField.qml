import QtQuick
import QtQuick.Layouts
import "../ui_items"

Rectangle {
    id: root

    property string label: ""
    property string placeholder: "由 Agent 建议"
    property alias value: field.text

    height: 66
    color: "transparent"

    ColumnLayout {
        anchors.fill: parent
        spacing: 6

        Text {
            text: root.label
            color: Theme.muted
            font.pixelSize: Theme.rem * 0.82
        }

        Item {
            Layout.fillWidth: true
            Layout.preferredHeight: 34

            GradientPanel {
                anchors.fill: parent
                radius: 10
                gradientAngle: 145
                gradientStops: Theme.fieldStops
                strokeColor: field.activeFocus ? Theme.accent : Theme.border
                strokeWidth: 1
            }

            TextInput {
                id: field
                anchors.fill: parent
                anchors.leftMargin: 10
                anchors.rightMargin: 10
                verticalAlignment: TextInput.AlignVCenter
                color: Theme.text
                font.pixelSize: Theme.rem * 0.88
                clip: true
            }

            Text {
                visible: field.text === ""
                anchors.fill: parent
                anchors.leftMargin: 10
                anchors.rightMargin: 10
                verticalAlignment: Text.AlignVCenter
                text: root.placeholder
                color: Theme.subdued
                font.pixelSize: Theme.rem * 0.83
                elide: Text.ElideRight
            }
        }
    }
}
