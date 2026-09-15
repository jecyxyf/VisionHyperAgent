pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../ui_items"

Item {
    id: root

    property var model: []
    property int currentIndex: 0
    signal activated(int index)

    height: 34
    implicitWidth: 220

    GradientPanel {
        anchors.fill: parent
        radius: 10
        gradientAngle: 145
        gradientStops: Theme.fieldStops
        strokeColor: popup.visible ? Theme.accent : Theme.border
        strokeWidth: 1
    }

    Text {
        anchors.left: parent.left
        anchors.leftMargin: 10
        anchors.right: chevron.left
        anchors.rightMargin: 6
        anchors.verticalCenter: parent.verticalCenter
        text: root.model[root.currentIndex] ?? ""
        color: Theme.text
        font.pixelSize: Theme.rem * 0.86
        elide: Text.ElideRight
    }

    ThemedIcon {
        id: chevron
        anchors.right: parent.right
        anchors.rightMargin: 8
        anchors.verticalCenter: parent.verticalCenter
        width: 14
        height: 14
        source: "../resources/ui/icons/chevron.svg"
        color: Theme.muted
    }

    MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: popup.visible ? popup.close() : popup.open()
    }

    Popup {
        id: popup
        y: root.height + 4
        width: root.width
        padding: 4
        background: Rectangle {
            radius: 10
            color: Theme.glassStrong
            border.color: Theme.border
            border.width: 1
        }

        contentItem: ColumnLayout {
            spacing: 2
            Repeater {
                model: root.model
                delegate: Rectangle {
                    id: optionDelegate
                    required property string modelData
                    required property int index
                    readonly property bool highlighted: index === root.currentIndex
                    Layout.fillWidth: true
                    height: 30
                    radius: 8
                    color: highlighted ? Theme.accentSurface
                           : (itemMouse.containsMouse ? Theme.surface : "transparent")

                    Text {
                        anchors.fill: parent
                        anchors.leftMargin: 10
                        anchors.rightMargin: 10
                        text: parent.modelData
                        color: optionDelegate.highlighted ? Theme.accent : Theme.muted
                        font.pixelSize: Theme.rem * 0.84
                        verticalAlignment: Text.AlignVCenter
                        elide: Text.ElideRight
                    }

                    MouseArea {
                        id: itemMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            root.currentIndex = optionDelegate.index
                            root.activated(optionDelegate.index)
                            popup.close()
                        }
                    }
                }
            }
        }
    }
}
