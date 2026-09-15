pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../components"
import "../ui_items"

Rectangle {
    id: root

    property bool gridMode: true
    property real listWidth: 116
    signal previewAction(string message)

    color: "transparent"

    ColumnLayout {
        anchors.fill: parent
        spacing: 16

        WorkspaceHeader {
            Layout.fillWidth: true
            icon: "../resources/ui/icons/edit.svg"
            title: "标注"

            IconButton {
                label: "打开图片目录"
                icon: "../resources/ui/icons/folder.svg"
                onActivated: root.previewAction("暂时无法打开图片目录。")
            }
            ToolbarDivider {}
            IconButton {
                label: "批量浏览"
                icon: "../resources/ui/icons/grid.svg"
                selected: root.gridMode
                onActivated: root.gridMode = true
            }
            IconButton {
                label: "逐图标注"
                icon: "../resources/ui/icons/edit.svg"
                selected: !root.gridMode
                onActivated: root.gridMode = false
            }
            ToolbarDivider {}
            IconButton {
                label: "Agent 生成初标"
                icon: "../resources/ui/icons/spark.svg"
                onActivated: root.previewAction("Agent 未连接，无法生成标注。")
            }
            IconButton {
                label: "确认标注"
                icon: "../resources/ui/icons/check.svg"
                onActivated: root.previewAction("没有可确认的标注。")
            }
        }

        GlassPanel {
            Layout.fillWidth: true
            Layout.preferredHeight: 46
            inset: true

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 16
                anchors.rightMargin: 16
                spacing: 10

                ThemedIcon {
                    Layout.preferredWidth: 16
                    Layout.preferredHeight: 16
                    source: "../resources/ui/icons/folder.svg"
                    color: Theme.subdued
                }
                Text {
                    Layout.fillWidth: true
                    text: "未打开目录"
                    color: Theme.muted
                    font.pixelSize: Theme.rem * 0.83
                }
                Text {
                    text: "0 张图片"
                    color: Theme.subdued
                    font.pixelSize: Theme.rem * 0.8
                }
            }
        }

        GlassPanel {
            visible: root.gridMode
            Layout.fillWidth: true
            Layout.fillHeight: true
            inset: true

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 18
                spacing: 16

                Text {
                    Layout.fillWidth: true
                    text: "图片浏览"
                    color: Theme.text
                    font.pixelSize: Theme.rem * 0.9
                    font.weight: Font.Medium
                }
                EmptyState {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    icon: "../resources/ui/icons/grid.svg"
                    title: "打开图片目录"
                }
                Text {
                    Layout.fillWidth: true
                    text: "待标注  0     ·     待审核  0     ·     已确认  0"
                    color: Theme.subdued
                    font.pixelSize: Theme.rem * 0.78
                }
            }
        }

        SplitView {
            id: singleBody
            visible: !root.gridMode
            Layout.fillWidth: true
            Layout.fillHeight: true
            orientation: Qt.Horizontal
            handle: PaneSplitHandle { thickness: 12; vertical: singleBody.orientation === Qt.Vertical }

            GlassPanel {
                SplitView.minimumWidth: 108
                SplitView.preferredWidth: root.listWidth
                SplitView.fillHeight: true
                onWidthChanged: root.listWidth = width
                inset: true

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 12
                    spacing: 12

                    Text {
                        Layout.fillWidth: true
                        text: "图片列表"
                        color: Theme.muted
                        font.pixelSize: Theme.rem * 0.8
                    }
                    Item { Layout.fillHeight: true }

                    RowLayout {
                        Layout.fillWidth: true
                        IconButton {
                            label: "上一张"
                            icon: "../resources/ui/icons/previous.svg"
                            onActivated: root.previewAction("当前没有图片。")
                        }
                        Item { Layout.fillWidth: true }
                        IconButton {
                            label: "下一张"
                            icon: "../resources/ui/icons/next.svg"
                            onActivated: root.previewAction("当前没有图片。")
                        }
                    }
                }
            }

            GlassPanel {
                SplitView.fillWidth: true
                SplitView.fillHeight: true
                inset: true

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 14
                    spacing: 12

                    Text {
                        Layout.fillWidth: true
                        text: "标注画布"
                        color: Theme.text
                        font.pixelSize: Theme.rem * 0.9
                        font.weight: Font.Medium
                    }
                    GridCanvas {
                        Layout.fillWidth: true
                        Layout.fillHeight: true

                        EmptyState {
                            anchors.fill: parent
                            icon: "../resources/ui/icons/edit.svg"
                            title: "选择图片"
                        }
                    }
                    Text {
                        Layout.fillWidth: true
                        text: "实例  —       类别  —       审核  待开始"
                        color: Theme.subdued
                        font.pixelSize: Theme.rem * 0.76
                    }
                }
            }
        }
    }
}
