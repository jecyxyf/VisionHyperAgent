pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../components"
import "../ui_items"

Rectangle {
    id: root

    property string directory: ""
    property var entries: []
    property int selectedIndex: -1
    property real detailsHeight: 112
    signal previewAction(string message)

    readonly property bool hasSelection: selectedIndex >= 0 && selectedIndex < entries.length

    color: "transparent"

    ColumnLayout {
        anchors.fill: parent
        spacing: 16

        WorkspaceHeader {
            Layout.fillWidth: true
            icon: "../resources/ui/icons/train.svg"
            title: "模型"

            IconButton {
                label: "选择模型目录"
                icon: "../resources/ui/icons/folder.svg"
                onActivated: root.previewAction("暂时无法选择模型目录。")
            }
            IconButton {
                label: "刷新模型列表"
                icon: "../resources/ui/icons/refresh.svg"
                onActivated: root.previewAction("暂时无法读取模型目录。")
            }
        }

        GlassPanel {
            Layout.fillWidth: true
            Layout.preferredHeight: 48
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
                    color: Theme.muted
                }
                Text {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    text: root.directory === "" ? "未选择模型目录" : root.directory
                    color: Theme.muted
                    font.pixelSize: Theme.rem * 0.9
                    elide: Text.ElideRight
                }
                Text {
                    text: root.entries.length + " 个模型"
                    color: Theme.subdued
                    font.pixelSize: Theme.rem * 0.8
                }
            }
        }

        SplitView {
            id: modelsBody
            Layout.fillWidth: true
            Layout.fillHeight: true
            orientation: Qt.Vertical
            handle: PaneSplitHandle {
                thickness: root.hasSelection ? 12 : 0
                vertical: modelsBody.orientation === Qt.Vertical
            }

            GlassPanel {
                SplitView.fillWidth: true
                SplitView.fillHeight: true
                inset: true

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 16
                    spacing: 12

                    ModelCells {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 24
                        header: true
                        entry: ({ name: "模型名称", format: "格式", path: "文件位置" })
                    }
                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 1
                        color: Theme.border
                    }

                    EmptyState {
                        visible: root.entries.length === 0
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        icon: "../resources/ui/icons/train.svg"
                        title: "暂无模型"
                    }

                    Flickable {
                        visible: root.entries.length > 0
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        contentWidth: width
                        contentHeight: modelRows.implicitHeight
                        clip: true

                        ScrollBar.vertical: ScrollBar {}

                        ColumnLayout {
                            id: modelRows
                            width: parent.width
                            spacing: 4

                            Repeater {
                                model: root.entries
                                ModelRow {
                                    required property var modelData
                                    required property int index
                                    Layout.fillWidth: true
                                    entry: modelData
                                    selected: root.selectedIndex === index
                                    onActivated: root.selectedIndex = index
                                }
                            }
                            Item { Layout.fillHeight: true }
                        }
                    }
                }
            }

            GlassPanel {
                visible: root.hasSelection
                enabled: root.hasSelection
                SplitView.fillWidth: true
                SplitView.minimumHeight: root.hasSelection ? 112 : 0
                SplitView.preferredHeight: root.hasSelection ? root.detailsHeight : 0
                onHeightChanged: if (root.hasSelection) root.detailsHeight = height
                inset: true

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 16
                    spacing: 10

                    Text {
                        Layout.fillWidth: true
                        text: root.hasSelection ? root.entries[root.selectedIndex].name : ""
                        color: Theme.text
                        font.pixelSize: Theme.rem * 0.95
                        font.weight: Font.Medium
                        elide: Text.ElideRight
                    }
                    Text {
                        Layout.fillWidth: true
                        text: root.hasSelection ? root.entries[root.selectedIndex].path : ""
                        color: Theme.muted
                        font.pixelSize: Theme.rem * 0.84
                        elide: Text.ElideRight
                    }
                    Text {
                        Layout.fillWidth: true
                        text: root.hasSelection ? "格式  ·  " + root.entries[root.selectedIndex].format : ""
                        color: Theme.subdued
                        font.pixelSize: Theme.rem * 0.78
                    }
                }
            }
        }
    }
}
