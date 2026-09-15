import QtQuick
import QtQuick.Layouts
import "../ui_items"

Item {
    id: root

    property var entry: ({ name: "", format: "", path: "" })
    property bool selected: false
    signal activated()

    height: 54
    Layout.fillWidth: true

    Keys.onReturnPressed: root.activated()
    Keys.onSpacePressed: root.activated()

    GradientPanel {
        anchors.fill: parent
        radius: 11
        gradientAngle: 120
        gradientStops: root.selected ? Theme.selectionStops
                        : (mouse.containsMouse ? [0, Theme.surface, 1, Theme.surface]
                                               : [0, "transparent", 1, "transparent"])
        strokeColor: root.activeFocus ? Theme.accent : "transparent"
        strokeWidth: 1

        ModelCells { entry: root.entry }

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
