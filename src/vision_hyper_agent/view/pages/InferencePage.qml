pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../components"
import "../ui_items"

Rectangle {
    id: root

    property string modelName: ""
    property int sourceChoice: 0
    property real recordsHeight: 138
    property bool sourceConfigOpen: false
    signal previewAction(string message)

    color: "transparent"

    ColumnLayout {
        anchors.fill: parent
        spacing: 16

        WorkspaceHeader {
            Layout.fillWidth: true
            icon: "../resources/ui/icons/play.svg"
            title: "运行"

            IconButton {
                label: "选择模型"
                icon: "../resources/ui/icons/train.svg"
                onActivated: root.previewAction("暂时无法加载模型。")
            }
            ToolbarDivider {}
            IconButton {
                label: "图像源配置"
                icon: "../resources/ui/icons/camera.svg"
                selected: root.sourceConfigOpen
                onActivated: root.sourceConfigOpen = !root.sourceConfigOpen
            }
            ToolbarDivider {}
            IconButton {
                label: "单次推理"
                icon: "../resources/ui/icons/play.svg"
                onActivated: root.previewAction("推理功能暂不可用。")
            }
        }

        SplitView {
            id: resultsBody
            Layout.fillWidth: true
            Layout.fillHeight: true
            orientation: Qt.Vertical
            handle: PaneSplitHandle { thickness: 12; vertical: resultsBody.orientation === Qt.Vertical }

            GlassPanel {
                SplitView.fillWidth: true
                SplitView.fillHeight: true
                inset: true

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 16
                    spacing: 12

                    RowLayout {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 24
                        Text {
                            Layout.fillWidth: true
                            text: "图像结果"
                            color: Theme.text
                            font.pixelSize: Theme.rem * 0.9
                            font.weight: Font.Medium
                        }
                    }

                    GridCanvas {
                        Layout.fillWidth: true
                        Layout.fillHeight: true

                        EmptyState {
                            anchors.fill: parent
                            icon: "../resources/ui/icons/scan.svg"
                            title: "等待图像"
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 26
                        spacing: 16

                        Text {
                            text: "图像源  ·  " + (root.sourceChoice === 1 ? "相机（未连接）" :
                                               root.sourceChoice === 2 ? "外部图片（未连接）" : "未配置")
                            color: Theme.muted
                            font.pixelSize: Theme.rem * 0.8
                        }
                        Rectangle {
                            Layout.preferredWidth: 1
                            Layout.preferredHeight: 12
                            color: Theme.border
                        }
                        Text {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 90
                            text: "模型  ·  " + (root.modelName === "" ? "未选择" : root.modelName)
                            color: Theme.muted
                            font.pixelSize: Theme.rem * 0.8
                            elide: Text.ElideRight
                        }
                        Text {
                            text: "耗时  —"
                            color: Theme.subdued
                            font.pixelSize: Theme.rem * 0.8
                        }
                        Text {
                            text: "实例  —"
                            color: Theme.subdued
                            font.pixelSize: Theme.rem * 0.8
                        }
                    }
                }
            }

            GlassPanel {
                SplitView.fillWidth: true
                SplitView.minimumHeight: 120
                SplitView.preferredHeight: root.recordsHeight
                onHeightChanged: root.recordsHeight = height
                inset: true

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 16
                    spacing: 10

                    RowLayout {
                        Layout.fillWidth: true
                        Text {
                            Layout.fillWidth: true
                            text: "识别记录"
                            color: Theme.text
                            font.pixelSize: Theme.rem * 0.9
                            font.weight: Font.Medium
                        }
                        Text {
                            text: "网络触发  ·  未连接"
                            color: Theme.subdued
                            font.pixelSize: Theme.rem * 0.78
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        Repeater {
                            model: ["触发来源", "时间", "识别结果", "耗时"]
                            Text {
                                required property string modelData
                                Layout.fillWidth: true
                                text: modelData
                                color: Theme.subdued
                                font.pixelSize: Theme.rem * 0.78
                            }
                        }
                    }
                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 1
                        color: Theme.border
                    }
                    Text {
                        Layout.fillWidth: true
                        text: "暂无记录"
                        color: Theme.subdued
                        font.pixelSize: Theme.rem * 0.8
                        wrapMode: Text.Wrap
                    }
                }
            }
        }
    }

    GlassPanel {
        visible: root.sourceConfigOpen
        x: Math.max(0, root.width - 298)
        y: 62
        width: 298
        height: 204
        gradientStops: [0, "#ffffffff", 1, "#ffffffff"]

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 18
            spacing: 12

            RowLayout {
                Layout.fillWidth: true
                Text {
                    Layout.fillWidth: true
                    text: "图像源配置"
                    color: Theme.text
                    font.pixelSize: Theme.rem
                    font.weight: Font.DemiBold
                }
                IconButton {
                    label: "关闭配置"
                    icon: "../resources/ui/icons/close.svg"
                    onActivated: root.sourceConfigOpen = false
                }
            }

            ThemedComboBox {
                Layout.fillWidth: true
                model: ["未选择", "相机 · OpenCV", "外部图片 · ZeroMQ"]
                currentIndex: root.sourceChoice
                onActivated: function(index) { root.sourceChoice = index }
            }

            InfoRow {
                Layout.fillWidth: true
                label: "状态"
                value: "未连接"
            }
            Item { Layout.fillHeight: true }
        }
    }
}
