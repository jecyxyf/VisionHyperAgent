import QtQuick
import QtQuick.Layouts
import "../ui_items"

Item {
    id: root

    property string text: ""
    property bool checked: false
    signal toggled(bool checked)

    height: 24
    implicitWidth: row.implicitWidth

    RowLayout {
        id: row
        anchors.fill: parent
        spacing: 8

        Item {
            Layout.preferredWidth: 16
            Layout.preferredHeight: 16

            GradientPanel {
                anchors.fill: parent
                radius: 5
                gradientAngle: 125
                gradientStops: root.checked ? Theme.actionStops : Theme.fieldStops
                strokeColor: root.checked ? Theme.accent : Theme.border
                strokeWidth: 1

                Text {
                    anchors.centerIn: parent
                    visible: root.checked
                    text: "✓"
                    color: "#ffffffff"
                    font.pixelSize: 11
                    font.weight: Font.Bold
                }
            }
        }

        Text {
            text: root.text
            color: Theme.text
            font.pixelSize: Theme.rem * 0.86
        }

        Item { Layout.fillWidth: true }
    }

    MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: {
            root.checked = !root.checked
            root.toggled(root.checked)
        }
    }
}
