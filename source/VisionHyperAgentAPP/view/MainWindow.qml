pragma ComponentBehavior: Bound
// qmllint disable unqualified
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "./components"
import "./pages"
import "./ui_items"

ApplicationWindow {
    id: root

    readonly property int currentPage: mainWindowViewModel.currentPage
    readonly property bool modelsExpanded: mainWindowViewModel.modelsExpanded
    readonly property var modelEntries: []
    property string modelsDirectory: ""
    property int selectedModel: -1
    property string feedback: ""
    property string chatDraft: ""
    property string lastMessage: ""
    property int sourceChoice: 0
    readonly property string inferenceModelName: ""
    property bool annotationGrid: true
    property string featureAnalysis: ""
    property string featureRules: ""
    property bool featureConfirmed: false

    // 面板布局状态；后续接入 basic 参数持久化。
    property real inferenceRecordsHeight: 138
    property real modelsDetailsHeight: 112
    property real annotationListWidth: 116
    property real preannotationEditorWidth: 300
    property real preannotationAnalysisRatio: 0.5
    property real trainingParametersWidth: 210
    property real trainingLogHeight: 114
    property real trainingLossRatio: 0.5
    property real chatComposerHeight: 132

    function showNotice(message) {
        root.feedback = message
        noticeTimer.restart()
    }

    function requestPage(page) {
        mainWindowViewModel.requestPage(page)
        UiHints.text = ""
    }

    width: 1440
    height: 900
    visible: true
    minimumWidth: 1120
    minimumHeight: 720
    title: "VisionHyperAgent"
    color: Theme.background
    font.family: "Noto Sans CJK SC"
    font.pixelSize: Theme.rem

    Image {
        anchors.fill: parent
        source: "./resources/ui/ambient.svg"
        fillMode: Image.Stretch
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.bottomMargin: 16
        spacing: 0

        AppHeader {
            Layout.fillWidth: true
            Layout.preferredHeight: 104
        }

        SplitView {
            id: mainSplit
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.leftMargin: 16
            Layout.rightMargin: 16
            orientation: Qt.Horizontal
            handle: PaneSplitHandle { thickness: 16; vertical: mainSplit.orientation === Qt.Vertical }

            GlassPanel {
                id: navigationPane
                SplitView.minimumWidth: 140
                SplitView.preferredWidth: Math.max(140, (root.width - 32) / 13)
                SplitView.fillHeight: true
                gradientStops: [0, Theme.surface, 1, Theme.surface]

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 10
                    anchors.topMargin: 16
                    spacing: 6

                    NavItem {
                        Layout.fillWidth: true
                        text: "运行"
                        icon: "../resources/ui/icons/play.svg"
                        selected: root.currentPage === Theme.pageInference
                        onActivated: root.requestPage(Theme.pageInference)
                    }

                    Item { Layout.preferredHeight: 14 }

                    ExpandableNavItem {
                        Layout.fillWidth: true
                        text: "模型"
                        icon: "../resources/ui/icons/train.svg"
                        expanded: root.modelsExpanded
                        selected: root.currentPage === Theme.pageModels ||
                                  (!root.modelsExpanded &&
                                   (root.currentPage === Theme.pagePreannotation ||
                                    root.currentPage === Theme.pageAnnotation ||
                                    root.currentPage === Theme.pagePretraining ||
                                    root.currentPage === Theme.pageTraining))
                        onActivated: root.requestPage(Theme.pageModels)
                        onToggleExpanded: mainWindowViewModel.toggleModels()
                    }

                    NavItem {
                        visible: root.modelsExpanded
                        Layout.fillWidth: true
                        text: "预标注"
                        nested: true
                        selected: root.currentPage === Theme.pagePreannotation
                        onActivated: root.requestPage(Theme.pagePreannotation)
                    }
                    NavItem {
                        visible: root.modelsExpanded
                        Layout.fillWidth: true
                        text: "标注"
                        nested: true
                        selected: root.currentPage === Theme.pageAnnotation
                        onActivated: root.requestPage(Theme.pageAnnotation)
                    }
                    NavItem {
                        visible: root.modelsExpanded
                        Layout.fillWidth: true
                        text: "预训练"
                        nested: true
                        selected: root.currentPage === Theme.pagePretraining
                        onActivated: root.requestPage(Theme.pagePretraining)
                    }
                    NavItem {
                        visible: root.modelsExpanded
                        Layout.fillWidth: true
                        text: "训练"
                        nested: true
                        selected: root.currentPage === Theme.pageTraining
                        onActivated: root.requestPage(Theme.pageTraining)
                    }

                    Item { Layout.fillHeight: true }
                    Item { Layout.preferredHeight: 8 }

                    NavItem {
                        Layout.fillWidth: true
                        text: "设置"
                        icon: "../resources/ui/icons/settings.svg"
                        selected: root.currentPage === Theme.pageSettings
                        onActivated: root.requestPage(Theme.pageSettings)
                    }
                    NavItem {
                        Layout.fillWidth: true
                        text: "关于"
                        icon: "../resources/ui/icons/about.svg"
                        selected: root.currentPage === Theme.pageAbout
                        onActivated: root.requestPage(Theme.pageAbout)
                    }
                }
            }

            GlassPanel {
                SplitView.fillWidth: true
                SplitView.fillHeight: true
                SplitView.minimumWidth: 540

                Loader {
                    id: pageLoader
                    anchors.fill: parent
                    anchors.margins: 18
                    sourceComponent: {
                        switch (root.currentPage) {
                            case Theme.pageModels: return modelsPage
                            case Theme.pagePreannotation: return preannotationPage
                            case Theme.pageAnnotation: return annotationPage
                            case Theme.pagePretraining: return pretrainingPage
                            case Theme.pageTraining: return trainingPage
                            case Theme.pageSettings: return settingsPage
                            case Theme.pageAbout: return aboutPage
                            default: return inferencePage
                        }
                    }
                }
            }

            GlobalChat {
                SplitView.minimumWidth: 260
                SplitView.preferredWidth: Math.max(
                    260,
                    (root.width - 32 - Math.max(140, (root.width - 32) / 13)) / 3
                )
                SplitView.fillHeight: true
                contextLabel: Theme.pageLabel(root.currentPage)
                draft: root.chatDraft
                onDraftChanged: root.chatDraft = draft
                lastMessage: root.lastMessage
                composerHeight: root.chatComposerHeight
                onComposerHeightChanged: root.chatComposerHeight = composerHeight
                onClearHistory: root.lastMessage = ""
                onSendMessage: function(message) {
                    root.lastMessage = message
                    root.chatDraft = ""
                    root.showNotice("Agent 未连接，消息未发送。")
                }
            }
        }
    }

    Component {
        id: inferencePage
        InferencePage {
            anchors.fill: parent
            modelName: root.inferenceModelName
            sourceChoice: root.sourceChoice
            onSourceChoiceChanged: root.sourceChoice = sourceChoice
            recordsHeight: root.inferenceRecordsHeight
            onRecordsHeightChanged: root.inferenceRecordsHeight = recordsHeight
            onPreviewAction: function(message) { root.showNotice(message) }
        }
    }
    Component {
        id: modelsPage
        ModelsPage {
            anchors.fill: parent
            directory: root.modelsDirectory
            entries: root.modelEntries
            selectedIndex: root.selectedModel
            onSelectedIndexChanged: root.selectedModel = selectedIndex
            detailsHeight: root.modelsDetailsHeight
            onDetailsHeightChanged: root.modelsDetailsHeight = detailsHeight
            onPreviewAction: function(message) { root.showNotice(message) }
        }
    }
    Component {
        id: preannotationPage
        PreannotationPage {
            anchors.fill: parent
            draft: featureDraft
            editorWidth: root.preannotationEditorWidth
            analysisRatio: root.preannotationAnalysisRatio
            analysis: root.featureAnalysis
            rules: root.featureRules
            confirmed: root.featureConfirmed
            onRequestAnalysis: {
                if (featureDraft.target === "" || featureDraft.appearance === "") {
                    root.showNotice("请填写识别目标和外观特征。")
                } else {
                    root.showNotice("Agent 分析暂不可用。")
                }
            }
            onConfirmRules: {
                if (root.featureAnalysis === "" || root.featureRules === "") {
                    root.showNotice("请先完成特征分析。")
                } else {
                    root.featureConfirmed = true
                }
            }
            onContinueAnnotation: {
                if (root.featureConfirmed)
                    root.requestPage(Theme.pageAnnotation)
                else
                    root.showNotice("请先确认标注规则。")
            }
        }
    }
    Component {
        id: annotationPage
        AnnotationPage {
            anchors.fill: parent
            gridMode: root.annotationGrid
            onGridModeChanged: root.annotationGrid = gridMode
            listWidth: root.annotationListWidth
            onPreviewAction: function(message) { root.showNotice(message) }
        }
    }
    Component {
        id: pretrainingPage
        PretrainingPage { anchors.fill: parent }
    }
    Component {
        id: trainingPage
        TrainingPage {
            anchors.fill: parent
            draft: trainingDraft
            parametersWidth: root.trainingParametersWidth
            logHeight: root.trainingLogHeight
            lossRatio: root.trainingLossRatio
            onPreviewAction: function(message) { root.showNotice(message) }
        }
    }
    Component {
        id: settingsPage
        SettingsPage {
            anchors.fill: parent
            onPreviewAction: function(message) { root.showNotice(message) }
        }
    }
    Component {
        id: aboutPage
        AboutPage { anchors.fill: parent }
    }

    FeatureDraft {
        id: featureDraft
        onTargetChanged: root.resetFeatureAnalysis()
        onAppearanceChanged: root.resetFeatureAnalysis()
        onDistinctionsChanged: root.resetFeatureAnalysis()
        onExclusionsChanged: root.resetFeatureAnalysis()
    }

    TrainingDraft {
        id: trainingDraft
    }

    function resetFeatureAnalysis() {
        featureAnalysis = ""
        featureRules = ""
        featureConfirmed = false
    }

    Timer {
        id: noticeTimer
        interval: 5000
        onTriggered: root.feedback = ""
    }

    GlassPanel {
        visible: root.feedback !== ""
        width: Math.min(480, root.width - 32)
        height: 56
        x: (root.width - width) / 2
        y: root.height - height - 16
        gradientStops: [0, Theme.glassStrong, 1, Theme.glassStrong]

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 16
            anchors.rightMargin: 10
            spacing: 12

            Text {
                Layout.fillWidth: true
                text: root.feedback
                color: Theme.text
                font.pixelSize: Theme.rem * 0.86
                wrapMode: Text.Wrap
            }
            IconButton {
                label: "关闭提示"
                icon: "../resources/ui/icons/close.svg"
                onActivated: {
                    root.feedback = ""
                    noticeTimer.stop()
                }
            }
        }
    }

    Rectangle {
        visible: UiHints.text !== ""
        x: Math.min(UiHints.x, root.width - width - 12)
        y: Math.min(UiHints.y, root.height - height - 12)
        width: hintLabel.implicitWidth + 20
        height: 28
        radius: 6
        color: Theme.text

        Text {
            id: hintLabel
            anchors.centerIn: parent
            text: UiHints.text
            color: "#ffffffff"
            font.pixelSize: Theme.rem * 0.76
        }
    }
}
