import QtQuick
import QtQuick.Layouts
import "../ui_items"

Item {
    id: root

    property string text: ""
    property string icon: ""
    property bool selected: false
    property bool expanded: false
    signal activated()
    signal toggleExpanded()

    height: 40
    Layout.fillWidth: true

    readonly property string foldLabel: (root.expanded ? "折叠" : "展开") + root.text + "子页面"

    GradientPanel {
        anchors.fill: parent
        radius: 12
        gradientAngle: 125
        gradientStops: root.selected ? Theme.actionStops
                        : ((bodyHit.containsMouse || arrowHit.containsMouse)
                           ? [0, Theme.elevated, 1, Theme.elevated]
                           : [0, "transparent", 1, "transparent"])
        strokeColor: root.activeFocus ? Theme.accent
                    : (root.selected ? Theme.glassEdge : "transparent")
        strokeWidth: 1
        shadowColor: root.selected ? Theme.actionShadow : "transparent"
        shadowBlur: 18
        shadowOffsetY: 2

        RowLayout {
            anchors.fill: parent
            spacing: 0

            Item {
                id: body
                Layout.fillWidth: true
                Layout.fillHeight: true

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    spacing: 10

                    ThemedIcon {
                        Layout.preferredWidth: 17
                        Layout.preferredHeight: 17
                        source: root.icon
                        color: root.selected ? "#ffffffff" : Theme.muted
                    }

                    Text {
                        Layout.fillWidth: true
                        text: root.text
                        color: root.selected ? "#ffffffff" : Theme.muted
                        font.pixelSize: Theme.rem
                        font.weight: root.selected ? Font.DemiBold : Font.Medium
                        elide: Text.ElideRight
                    }
                }

                MouseArea {
                    id: bodyHit
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        root.forceActiveFocus()
                        root.activated()
                    }
                }
            }

            Item {
                id: disclosure
                Layout.preferredWidth: 30
                Layout.fillHeight: true

                ThemedIcon {
                    anchors.centerIn: parent
                    width: 14
                    height: 14
                    source: root.expanded ? "../resources/ui/icons/chevron.svg"
                                          : "../resources/ui/icons/next.svg"
                    color: root.selected ? "#ffffffff" : Theme.muted
                }

                Rectangle {
                    visible: disclosure.activeFocus
                    anchors.horizontalCenter: parent.horizontalCenter
                    y: parent.height - 7
                    width: 12
                    height: 2
                    radius: 1
                    color: root.selected ? "#ffffffff" : Theme.accent
                }

                MouseArea {
                    id: arrowHit
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onContainsMouseChanged: {
                        if (containsMouse) {
                            UiHints.text = root.foldLabel
                            let position = root.mapToItem(root.Window.contentItem, 0, root.height + 8)
                            UiHints.x = position.x
                            UiHints.y = position.y
                        } else if (UiHints.text === root.foldLabel) {
                            UiHints.text = ""
                        }
                    }
                    onClicked: {
                        disclosure.forceActiveFocus()
                        root.toggleExpanded()
                    }
                }
            }
        }
    }
}
