import QtQuick
import QtQuick.Layouts
import "../components"

Rectangle {
    id: root

    color: "transparent"

    ColumnLayout {
        anchors.fill: parent
        spacing: 16

        WorkspaceHeader {
            Layout.fillWidth: true
            icon: "../resources/ui/icons/train.svg"
            title: "预训练"
        }

        GlassPanel {
            Layout.fillWidth: true
            Layout.fillHeight: true
            inset: true

            EmptyState {
                anchors.fill: parent
                icon: "../resources/ui/icons/train.svg"
                title: "暂无预训练任务"
            }
        }
    }
}
