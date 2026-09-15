import QtQuick
import QtQuick.Layouts
import "../ui_items"

Item {
    id: root

    property string text: ""
    property string icon: ""
    property bool selected: false
    property bool nested: false
    signal activated()

    height: 40
    Layout.fillWidth: true

    Keys.onReturnPressed: root.activated()
    Keys.onSpacePressed: root.activated()

    GradientPanel {
        anchors.fill: parent
        radius: 12
        gradientAngle: 125
        gradientStops: root.selected ? Theme.actionStops
                        : (mouse.containsMouse ? [0, Theme.elevated, 1, Theme.elevated]
                                               : [0, "transparent", 1, "transparent"])
        strokeColor: root.activeFocus ? Theme.accent
                    : (root.selected ? Theme.glassEdge : "transparent")
        strokeWidth: 1
        shadowColor: root.selected ? Theme.actionShadow : "transparent"
        shadowBlur: 18
        shadowOffsetY: 2

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: root.nested ? 35 : 12
            anchors.rightMargin: 12
            spacing: 10

            ThemedIcon {
                visible: !root.nested
                Layout.preferredWidth: 17
                Layout.preferredHeight: 17
                source: root.icon
                color: root.selected ? "#ffffffff" : Theme.muted
            }

            Rectangle {
                visible: root.nested
                Layout.preferredWidth: 4
                Layout.preferredHeight: 4
                radius: 2
                color: root.selected ? "#ffffffff" : Theme.border
            }

            Text {
                Layout.fillWidth: true
                text: root.text
                color: root.selected ? "#ffffffff" : Theme.muted
                font.pixelSize: Theme.rem
                font.weight: root.selected ? Font.DemiBold : Font.Medium
                elide: Text.ElideRight
            }

            Rectangle {
                visible: root.selected
                Layout.preferredWidth: 3
                Layout.preferredHeight: 14
                radius: 2
                color: "#cfffffff"
            }
        }

        MouseArea {
            id: mouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: {
                root.forceActiveFocus()
                root.activated()
            }
        }
    }
}
