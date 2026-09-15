pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../components"
import "../ui_items"

Rectangle {
    id: root

    property FeatureDraft draft: null
    property real editorWidth: 300
    property real analysisRatio: 0.5
    property string analysis: ""
    property string rules: ""
    property bool confirmed: false

    signal requestAnalysis()
    signal confirmRules()
    signal continueAnnotation()

    color: "transparent"

    ColumnLayout {
        anchors.fill: parent
        spacing: 16

        WorkspaceHeader {
            Layout.fillWidth: true
            icon: "../resources/ui/icons/spark.svg"
            title: "预标注"

            IconButton {
                label: "分析特征"
                icon: "../resources/ui/icons/spark.svg"
                onActivated: root.requestAnalysis()
            }
            IconButton {
                label: "确认标注规则"
                icon: "../resources/ui/icons/check.svg"
                selected: root.confirmed
                onActivated: root.confirmRules()
            }
            ToolbarDivider {}
            IconButton {
                label: "进入标注"
                icon: "../resources/ui/icons/next.svg"
                onActivated: root.continueAnnotation()
            }
        }

        SplitView {
            id: body
            Layout.fillWidth: true
            Layout.fillHeight: true
            orientation: Qt.Horizontal
            handle: PaneSplitHandle { thickness: 12; vertical: body.orientation === Qt.Vertical }

            GlassPanel {
                SplitView.minimumWidth: 220
                SplitView.preferredWidth: root.editorWidth
                SplitView.fillHeight: true
                onWidthChanged: root.editorWidth = width
                inset: true

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 16
                    spacing: 14

                    Text {
                        Layout.fillWidth: true
                        text: "特征描述"
                        color: Theme.text
                        font.pixelSize: Theme.rem
                        font.weight: Font.DemiBold
                    }

                    Flickable {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        contentWidth: width
                        contentHeight: fields.implicitHeight
                        clip: true

                        ScrollBar.vertical: ScrollBar {}

                        ColumnLayout {
                            id: fields
                            width: parent.width
                            spacing: 14

                            DraftField {
                                Layout.fillWidth: true
                                label: "识别目标"
                                placeholder: "需要识别什么对象"
                                value: root.draft ? root.draft.target : ""
                                onValueChanged: if (root.draft) root.draft.target = value
                            }
                            FeatureTextArea {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 140
                                label: "外观与关键特征"
                                placeholder: "形状、颜色、纹理、边界…"
                                value: root.draft ? root.draft.appearance : ""
                                onValueChanged: if (root.draft) root.draft.appearance = value
                            }
                            FeatureTextArea {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 108
                                label: "类别区别"
                                placeholder: "如何区分类似对象"
                                value: root.draft ? root.draft.distinctions : ""
                                onValueChanged: if (root.draft) root.draft.distinctions = value
                            }
                            FeatureTextArea {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 108
                                label: "排除情况"
                                placeholder: "哪些情况不应标注"
                                value: root.draft ? root.draft.exclusions : ""
                                onValueChanged: if (root.draft) root.draft.exclusions = value
                            }
                            Item { Layout.fillHeight: true }
                        }
                    }
                }
            }

            SplitView {
                id: analysisColumn
                SplitView.fillWidth: true
                SplitView.fillHeight: true
                orientation: Qt.Vertical
                handle: PaneSplitHandle { thickness: 12; vertical: analysisColumn.orientation === Qt.Vertical }

                GlassPanel {
                    SplitView.fillWidth: true
                    SplitView.minimumHeight: 180
                    SplitView.preferredHeight: (analysisColumn.height - 12) * root.analysisRatio
                    onHeightChanged: root.analysisRatio = height / Math.max(1, analysisColumn.height - 12)
                    inset: true

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: 16
                        spacing: 12

                        Text {
                            Layout.fillWidth: true
                            text: "Agent 分析"
                            color: Theme.text
                            font.pixelSize: Theme.rem
                            font.weight: Font.DemiBold
                        }

                        EmptyState {
                            visible: root.analysis === ""
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            icon: "../resources/ui/icons/spark.svg"
                            title: "等待分析"
                        }

                        Flickable {
                            visible: root.analysis !== ""
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            contentWidth: width
                            contentHeight: analysisText.implicitHeight + 32
                            clip: true

                            ScrollBar.vertical: ScrollBar {}

                            Text {
                                id: analysisText
                                width: parent.width
                                text: root.analysis
                                color: Theme.text
                                font.pixelSize: Theme.rem * 0.95
                                wrapMode: Text.Wrap
                            }
                        }
                    }
                }

                GlassPanel {
                    SplitView.fillWidth: true
                    SplitView.fillHeight: true
                    SplitView.minimumHeight: 180
                    inset: true

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: 16
                        spacing: 12

                        RowLayout {
                            Layout.fillWidth: true
                            Text {
                                Layout.fillWidth: true
                                text: "标注规则"
                                color: Theme.text
                                font.pixelSize: Theme.rem
                                font.weight: Font.DemiBold
                            }
                            Text {
                                visible: root.confirmed
                                text: "已确认"
                                color: Theme.accent
                                font.pixelSize: Theme.rem * 0.84
                            }
                        }

                        EmptyState {
                            visible: root.rules === ""
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            icon: "../resources/ui/icons/check.svg"
                            title: "等待规则"
                        }

                        Flickable {
                            visible: root.rules !== ""
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            contentWidth: width
                            contentHeight: rulesText.implicitHeight + 32
                            clip: true

                            ScrollBar.vertical: ScrollBar {}

                            Text {
                                id: rulesText
                                width: parent.width
                                text: root.rules
                                color: Theme.text
                                font.pixelSize: Theme.rem * 0.95
                                wrapMode: Text.Wrap
                            }
                        }
                    }
                }
            }
        }
    }
}
