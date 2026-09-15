import QtQuick
import QtQuick.Layouts
import QtQuick.Effects
import "../ui_items"

Rectangle {
    id: root

    height: 104
    color: "transparent"

    readonly property real titleSize: root.width < 1280 ? Theme.rem * 1.86 : Theme.rem * 2

    GradientPanel {
        id: plate
        x: 16
        y: 12
        width: root.width - 32
        height: 78
        radius: 19
        gradientAngle: 110
        gradientStops: Theme.headerSilverStops
        strokeColor: Theme.headerEdge
        strokeWidth: 1
        shadowColor: Theme.headerShadow
        shadowBlur: 22
        shadowOffsetY: 6

        // 顶部虹彩光泽（圆角裁剪）
        Image {
            id: sheen
            x: 1
            y: 1
            width: parent.width - 2
            height: parent.height - 2
            source: "../resources/ui/header-sheen.svg"
            fillMode: Image.Stretch
            visible: false
        }

        Rectangle {
            id: sheenMask
            x: 1
            y: 1
            width: parent.width - 2
            height: parent.height - 2
            radius: 18
            color: "#ffffffff"
            visible: false
        }

        MultiEffect {
            x: 1
            y: 1
            width: sheen.width
            height: sheen.height
            source: sheen
            autoPaddingEnabled: false
            maskEnabled: true
            maskSource: sheenMask
        }

        // 内圈高光描边
        Rectangle {
            x: 2
            y: 2
            width: parent.width - 4
            height: parent.height - 4
            radius: 17
            color: "transparent"
            border.color: Theme.headerEdge
            border.width: 1
        }

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 18
            anchors.rightMargin: 24
            spacing: 18

            // Agent 徽标
            Item {
                Layout.preferredWidth: 122
                Layout.preferredHeight: 50

                GradientPanel {
                    anchors.fill: parent
                    radius: 13
                    gradientAngle: 125
                    gradientStops: Theme.headerBadgeStops
                    strokeColor: Theme.headerBadgeEdge
                    strokeWidth: 1
                    shadowColor: Theme.headerBadgeShadow
                    shadowBlur: 18
                    shadowOffsetY: 5
                }

                Rectangle {
                    x: 1
                    y: 1
                    width: parent.width - 2
                    height: 22
                    radius: 12
                    gradient: Gradient {
                        GradientStop { position: 0; color: "#32ffffff" }
                        GradientStop { position: 1; color: "#00ffffff" }
                    }
                }

                Rectangle {
                    x: 15
                    y: 1
                    width: parent.width - 30
                    height: 1
                    gradient: Gradient {
                        orientation: Gradient.Horizontal
                        GradientStop { position: 0; color: "#00ffffff" }
                        GradientStop { position: 0.5; color: "#f0ffffff" }
                        GradientStop { position: 1; color: "#00ffffff" }
                    }
                }

                Text {
                    anchors.centerIn: parent
                    text: "Agent"
                    color: "#ffffffff"
                    font.family: "Nimbus Sans"
                    font.pixelSize: Theme.rem * 2.05
                    font.weight: Font.Bold
                    font.italic: true
                }
            }

            RowLayout {
                spacing: 0

                Text {
                    text: "驱动的视觉模型"
                    color: Theme.headerInk
                    font.pixelSize: root.titleSize
                    font.weight: Font.Bold
                    font.letterSpacing: 0.3
                }

                Text {
                    text: "自动训练"
                    color: Theme.headerAccent
                    font.pixelSize: root.titleSize
                    font.weight: Font.Bold
                    font.letterSpacing: 0.3
                }

                Text {
                    text: "及部署平台"
                    color: Theme.headerInk
                    font.pixelSize: root.titleSize
                    font.weight: Font.Bold
                    font.letterSpacing: 0.3
                }
            }

            Item { Layout.fillWidth: true }
        }
    }
}
