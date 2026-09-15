import QtQuick
import QtQuick.Layouts
import "../components"
import "../ui_items"

Rectangle {
    id: root

    color: "transparent"

    ColumnLayout {
        anchors.fill: parent
        spacing: 16

        WorkspaceHeader {
            Layout.fillWidth: true
            icon: "../resources/ui/icons/about.svg"
            title: "关于"
        }

        GlassPanel {
            Layout.fillWidth: true
            Layout.preferredHeight: 280
            inset: true

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 28
                spacing: 16

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 16

                    Rectangle {
                        Layout.preferredWidth: 48
                        Layout.preferredHeight: 48
                        radius: 14
                        color: Theme.accentSurface

                        ThemedIcon {
                            anchors.centerIn: parent
                            width: 26
                            height: 26
                            source: "../resources/ui/icons/scan.svg"
                            color: Theme.accent
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 5

                        Text {
                            Layout.fillWidth: true
                            text: "VisionHyperAgent"
                            color: Theme.text
                            font.pixelSize: Theme.rem * 1.45
                            font.weight: Font.DemiBold
                        }

                        Text {
                            Layout.fillWidth: true
                            text: "Agent 驱动的视觉模型工作台"
                            color: Theme.muted
                            font.pixelSize: Theme.rem * 0.88
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
                    text: "本地训练 · 离线识别"
                    color: Theme.muted
                    font.pixelSize: Theme.rem * 0.86
                }

                GradientPanel {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    radius: 14
                    gradientAngle: 125
                    gradientStops: Theme.selectionStops
                    strokeColor: Theme.glassEdge
                    strokeWidth: 1

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: 16
                        spacing: 7

                        Item { Layout.fillHeight: true }

                        Text {
                            Layout.fillWidth: true
                            text: "Powered by Qt 6 / PySide6"
                            color: Theme.accent
                            font.pixelSize: Theme.rem * 0.95
                            font.weight: Font.DemiBold
                        }

                        Text {
                            Layout.fillWidth: true
                            text: "声明式 QML 界面"
                            color: Theme.muted
                            font.pixelSize: Theme.rem * 0.8
                        }

                        Item { Layout.fillHeight: true }
                    }
                }
            }
        }

        Item { Layout.fillHeight: true }
    }
}
