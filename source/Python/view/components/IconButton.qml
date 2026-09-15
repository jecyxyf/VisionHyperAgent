import QtQuick
import "../ui_items"

Item {
    id: root

    property string label: ""
    property string icon: ""
    property bool primary: false
    property bool selected: false
    signal activated()

    width: 34
    height: 34

    Keys.onReturnPressed: root.activated()
    Keys.onSpacePressed: root.activated()

    GradientPanel {
        id: body
        y: mouse.pressed ? 1 : 0
        width: parent.width
        height: parent.height
        radius: 11
        gradientAngle: 125
        gradientStops: {
            if (root.primary)
                return Theme.actionStops
            if (mouse.pressed || root.selected)
                return Theme.selectionStops
            if (mouse.containsMouse)
                return [0, Theme.elevated, 1, Theme.elevated]
            return [0, Theme.surface, 1, Theme.surface]
        }
        strokeColor: root.activeFocus ? Theme.accent : Theme.glassEdge
        strokeWidth: 1
        shadowColor: (root.primary || root.selected) && !mouse.pressed ? Theme.actionShadow : "transparent"
        shadowBlur: mouse.pressed ? 4 : 13
        shadowOffsetY: mouse.pressed ? 1 : 4
        Behavior on y { NumberAnimation { duration: 80 } }

        ThemedIcon {
            anchors.centerIn: parent
            width: 17
            height: 17
            source: root.icon
            color: root.primary ? "#ffffffff" : (root.selected ? Theme.accent : Theme.muted)
        }
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onContainsMouseChanged: {
            if (containsMouse) {
                UiHints.text = root.label
                let position = root.mapToItem(root.Window.contentItem, 0, root.height + 8)
                UiHints.x = position.x
                UiHints.y = position.y
            } else if (UiHints.text === root.label) {
                UiHints.text = ""
            }
        }
        onClicked: {
            root.forceActiveFocus()
            root.activated()
        }
    }

}
