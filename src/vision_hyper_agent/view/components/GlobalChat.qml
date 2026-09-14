pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../ui_items"

GlassPanel {
    id: root

    property string contextLabel: ""
    property alias draft: input.text
    property string lastMessage: ""
    property real composerHeight: 132

    signal sendMessage(string message)
    signal clearHistory()

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 18
        spacing: 16

        WorkspaceHeader {
            Layout.fillWidth: true
            title: "Agent"
            icon: "../resources/ui/icons/spark.svg"

            IconButton {
                label: "清除聊天记录"
                icon: "../resources/ui/icons/trash.svg"
                onActivated: root.clearHistory()
            }
        }

        SplitView {
            id: body
            Layout.fillWidth: true
            Layout.fillHeight: true
            orientation: Qt.Vertical
            handle: PaneSplitHandle {}

            // 消息历史
            Item {
                SplitView.fillHeight: true
                SplitView.fillWidth: true

                Flickable {
                    id: historyFlick
                    anchors.fill: parent
                    contentWidth: width
                    contentHeight: historyColumn.implicitHeight
                    clip: true

                    ColumnLayout {
                        id: historyColumn
                        width: historyFlick.width
                        spacing: 16

                        Rectangle {
                            visible: root.lastMessage !== ""
                            Layout.fillWidth: true
                            Layout.preferredHeight: bubble.implicitHeight + 24
                            radius: 14
                            color: "transparent"

                            GradientPanel {
                                anchors.fill: parent
                                radius: 14
                                gradientAngle: 120
                                gradientStops: Theme.selectionStops
                            }

                            ColumnLayout {
                                id: bubble
                                anchors.fill: parent
                                anchors.margins: 12
                                spacing: 10

                                Text {
                                    Layout.fillWidth: true
                                    text: root.lastMessage
                                    color: Theme.text
                                    font.pixelSize: Theme.rem * 0.86
                                    wrapMode: Text.Wrap
                                }

                                Text {
                                    text: "未发送"
                                    color: Theme.muted
                                    font.pixelSize: Theme.rem * 0.76
                                }
                            }
                        }

                        Item { Layout.fillHeight: true }
                    }
                }
            }

            // 输入区
            Item {
                SplitView.fillWidth: true
                SplitView.preferredHeight: root.composerHeight
                SplitView.minimumHeight: 108
                onHeightChanged: root.composerHeight = height

                GradientPanel {
                    anchors.fill: parent
                    radius: 16
                    gradientAngle: 145
                    gradientStops: Theme.fieldStops
                    strokeColor: input.activeFocus ? Theme.accent : Theme.border
                    strokeWidth: 1
                }

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 12
                    spacing: 10

                    TextEdit {
                        id: input
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        color: Theme.text
                        font.pixelSize: Theme.rem * 0.87
                        wrapMode: TextEdit.Wrap
                        clip: true

                        Text {
                            visible: input.text === "" && !input.activeFocus
                            text: "输入消息…"
                            color: Theme.subdued
                            font.pixelSize: Theme.rem * 0.86
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        Item { Layout.fillWidth: true }

                        IconButton {
                            label: "发送消息"
                            icon: "../resources/ui/icons/send.svg"
                            primary: true
                            onActivated: {
                                if (input.text !== "")
                                    root.sendMessage(input.text)
                            }
                        }
                    }
                }
            }
        }
    }

    component PaneSplitHandle: Rectangle {
        id: handleRoot
        implicitHeight: 16
        color: "transparent"

        Rectangle {
            anchors.centerIn: parent
            width: 48
            height: 3
            radius: 2
            color: handleRoot.SplitHandle.pressed ? Theme.accent
                   : (handleRoot.SplitHandle.hovered ? Theme.accent : "#526545ce")
        }
    }
}
