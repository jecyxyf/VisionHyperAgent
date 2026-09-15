pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../components"
import "../ui_items"

Rectangle {
    id: root

    property TrainingDraft draft: null
    property real parametersWidth: 210
    property real logHeight: 114
    property real lossRatio: 0.5
    signal previewAction(string message)

    color: "transparent"

    ColumnLayout {
        anchors.fill: parent
        spacing: 16

        WorkspaceHeader {
            Layout.fillWidth: true
            icon: "../resources/ui/icons/train.svg"
            title: "训练"

            IconButton {
                label: "Agent 建议参数"
                icon: "../resources/ui/icons/spark.svg"
                onActivated: root.previewAction("Agent 未连接，无法生成参数建议。")
            }
            ToolbarDivider {}
            IconButton {
                label: "停止训练"
                icon: "../resources/ui/icons/stop.svg"
                onActivated: root.previewAction("没有正在执行的训练任务。")
            }
            IconButton {
                label: "开始训练"
                icon: "../resources/ui/icons/play.svg"
                onActivated: root.previewAction("训练功能暂不可用。")
            }
        }

        SplitView {
            id: body
            Layout.fillWidth: true
            Layout.fillHeight: true
            orientation: Qt.Horizontal
            handle: PaneSplitHandle { thickness: 12; vertical: body.orientation === Qt.Vertical }

            GlassPanel {
                SplitView.minimumWidth: 180
                SplitView.preferredWidth: root.parametersWidth
                SplitView.fillHeight: true
                onWidthChanged: root.parametersWidth = width
                inset: true

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 16
                    spacing: 14

                    Text {
                        Layout.fillWidth: true
                        text: "训练参数"
                        color: Theme.text
                        font.pixelSize: Theme.rem * 0.95
                        font.weight: Font.DemiBold
                    }
                    Text {
                        Layout.fillWidth: true
                        text: "未选择数据集"
                        color: Theme.subdued
                        font.pixelSize: Theme.rem * 0.78
                        wrapMode: Text.Wrap
                    }
                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 1
                        color: Theme.border
                    }

                    Flickable {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        contentWidth: width
                        contentHeight: parameters.implicitHeight
                        clip: true

                        ScrollBar.vertical: ScrollBar {}

                        ColumnLayout {
                            id: parameters
                            width: parent.width
                            spacing: 14

                            DraftField {
                                Layout.fillWidth: true
                                label: "训练轮数  /  Epochs"
                                placeholder: "由 Agent 建议"
                                value: root.draft ? root.draft.epochs : ""
                                onValueChanged: if (root.draft) root.draft.epochs = value
                            }
                            DraftField {
                                Layout.fillWidth: true
                                label: "批次大小  /  Batch"
                                placeholder: "由 Agent 建议"
                                value: root.draft ? root.draft.batch : ""
                                onValueChanged: if (root.draft) root.draft.batch = value
                            }
                            DraftField {
                                Layout.fillWidth: true
                                label: "图像尺寸  /  Image size"
                                placeholder: "由 Agent 建议"
                                value: root.draft ? root.draft.imageSize : ""
                                onValueChanged: if (root.draft) root.draft.imageSize = value
                            }
                            DraftField {
                                Layout.fillWidth: true
                                label: "学习率  /  Learning rate"
                                placeholder: "由 Agent 建议"
                                value: root.draft ? root.draft.learningRate : ""
                                onValueChanged: if (root.draft) root.draft.learningRate = value
                            }
                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 1
                                color: Theme.border
                            }
                            ThemedCheckBox {
                                Layout.fillWidth: true
                                text: "Agent 自动调参"
                                checked: root.draft ? root.draft.automatic : false
                                onToggled: if (root.draft) root.draft.automatic = checked
                            }
                            Item { Layout.fillHeight: true }
                        }
                    }
                }
            }

            SplitView {
                id: effectsColumn
                SplitView.fillWidth: true
                SplitView.fillHeight: true
                orientation: Qt.Vertical
                handle: PaneSplitHandle { thickness: 12; vertical: effectsColumn.orientation === Qt.Vertical }

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
                            Text {
                                Layout.fillWidth: true
                                text: "训练效果"
                                color: Theme.text
                                font.pixelSize: Theme.rem * 0.95
                                font.weight: Font.DemiBold
                            }
                            Text {
                                text: "未开始"
                                color: Theme.subdued
                                font.pixelSize: Theme.rem * 0.78
                            }
                        }

                        SplitView {
                            id: charts
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            orientation: Qt.Vertical
                            handle: PaneSplitHandle { thickness: 12; vertical: charts.orientation === Qt.Vertical }

                            EmptyChart {
                                SplitView.fillWidth: true
                                SplitView.minimumHeight: 130
                                SplitView.preferredHeight: (charts.height - 12) * root.lossRatio
                                onHeightChanged: root.lossRatio = height / Math.max(1, charts.height - 12)
                                title: "损失  /  Loss"
                                detail: "暂无数据"
                            }
                            EmptyChart {
                                SplitView.fillWidth: true
                                SplitView.fillHeight: true
                                SplitView.minimumHeight: 130
                                title: "评估指标"
                                detail: "暂无数据"
                            }
                        }
                    }
                }

                GlassPanel {
                    SplitView.fillWidth: true
                    SplitView.minimumHeight: 96
                    SplitView.preferredHeight: root.logHeight
                    onHeightChanged: root.logHeight = height
                    inset: true

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: 16
                        spacing: 10

                        Text {
                            Layout.fillWidth: true
                            text: "任务日志"
                            color: Theme.text
                            font.pixelSize: Theme.rem * 0.9
                            font.weight: Font.Medium
                        }
                        Text {
                            Layout.fillWidth: true
                            text: "暂无日志"
                            color: Theme.subdued
                            font.pixelSize: Theme.rem * 0.79
                            wrapMode: Text.Wrap
                        }
                    }
                }
            }
        }
    }
}
