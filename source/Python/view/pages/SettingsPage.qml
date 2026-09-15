import QtQuick
import QtQuick.Layouts
import "../components"
import "../ui_items"

Rectangle {
    id: root

    signal previewAction(string message)
    color: "transparent"

    ColumnLayout {
        anchors.fill: parent
        spacing: 16

        WorkspaceHeader {
            Layout.fillWidth: true
            icon: "../resources/ui/icons/settings.svg"
            title: "设置"

            IconButton {
                label: "配置连接"
                icon: "../resources/ui/icons/settings.svg"
                onActivated: root.previewAction("连接配置暂不可用。")
            }
        }

        GlassPanel {
            Layout.fillWidth: true
            Layout.preferredHeight: 282
            inset: true

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 16
                spacing: 4

                Text {
                    Layout.fillWidth: true
                    text: "连接与运行环境"
                    color: Theme.text
                    font.pixelSize: Theme.rem * 0.95
                    font.weight: Font.DemiBold
                }

                InfoRow {
                    Layout.fillWidth: true
                    label: "大模型服务"
                    value: "未配置"
                }
                InfoRow {
                    Layout.fillWidth: true
                    label: "Codex"
                    value: "未连接"
                }
                InfoRow {
                    Layout.fillWidth: true
                    label: "Python 训练环境"
                    value: "未安装"
                }
                InfoRow {
                    Layout.fillWidth: true
                    label: "NVIDIA 设备"
                    value: "未检测"
                }
                InfoRow {
                    Layout.fillWidth: true
                    label: "外部通信"
                    value: "ZeroMQ · 未连接"
                }
            }
        }

        Item { Layout.fillHeight: true }
    }
}
