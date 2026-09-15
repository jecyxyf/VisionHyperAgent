pragma Singleton
import QtQuick

QtObject {
    // 1rem = 14px，与旧 Slint 界面一致
    readonly property real rem: 14

    // 页面枚举（与旧 ViewPage 一致）
    readonly property int pageInference: 0
    readonly property int pageModels: 1
    readonly property int pagePreannotation: 2
    readonly property int pageAnnotation: 3
    readonly property int pagePretraining: 4
    readonly property int pageTraining: 5
    readonly property int pageSettings: 6
    readonly property int pageAbout: 7

    // Prism glass tokens（Slint Theme.slint 原样迁移）
    readonly property color background: "#eeebfa"
    readonly property color glassStrong: "#f2ffffff"
    readonly property color glassEdge: "#dbffffff"
    readonly property color surface: "#54ffffff"
    readonly property color elevated: "#bdffffff"
    readonly property color border: "#286552ab"
    readonly property color text: "#29294e"
    readonly property color muted: "#545579"
    readonly property color subdued: "#686b89"
    readonly property color accent: "#6545ce"
    readonly property color accentSurface: "#9ce3d5ff"
    readonly property color shadow: "#23493982"
    readonly property color grid: "#166651a7"
    readonly property color actionShadow: "#4a7754cd"
    readonly property color headerInk: "#30214e"
    readonly property color headerAccent: "#a032b7"
    readonly property color headerEdge: "#e8ffffff"
    readonly property color headerShadow: "#2e68438f"
    readonly property color headerBadgeEdge: "#dce6cbff"
    readonly property color headerBadgeShadow: "#5c8251ce"

    // 渐变 stops：[position, color] 平铺数组，供 GradientPanel 使用
    readonly property var glassStops: [0.0, "#a6ffffff", 0.52, "#68ffffff", 1.0, "#85ffffff"]
    readonly property var actionStops: [0.0, "#ff505be4", 0.58, "#ff8148d5", 1.0, "#ffb54cc1"]
    readonly property var selectionStops: [0.0, "#b8d5caff", 1.0, "#9cf4ccf5"]
    readonly property var fieldStops: [0.0, "#bdffffff", 1.0, "#72ffffff"]
    readonly property var canvasStops: [0.0, "#70ffffff", 1.0, "#42ffffff"]
    readonly property var headerSilverStops: [0.0, "#e0cfdbff", 0.46, "#d1f1d4fd", 1.0, "#e0d6f5f0"]
    readonly property var headerBadgeStops: [0.0, "#ff555cec", 0.56, "#ff9346d8", 1.0, "#ffd44ea9"]
    readonly property var edgeHighlightStops: [0.0, "#00ffffff", 0.36, "#edffffff", 0.72, "#9cd7efff", 1.0, "#00ffffff"]

    function pageLabel(page) {
        switch (page) {
            case pageInference: return "运行"
            case pageModels: return "模型"
            case pagePreannotation: return "预标注"
            case pageAnnotation: return "标注"
            case pagePretraining: return "预训练"
            case pageTraining: return "训练"
            case pageSettings: return "设置"
            default: return "关于"
        }
    }
}
