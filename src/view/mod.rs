//! 原生 Slint 窗口入口；当前只承载已有界面，不接入训练、推理或 Agent 服务。
use crate::view_model::{LayoutGroup, MainPage, MainWindowViewModel, Result, ViewModelError};
use std::rc::Rc;

slint::include_modules!();

/// View 的绑定适配：导航请求交给主窗口 ViewModel，不把 UI 类型带入运行逻辑。
pub fn bind_main_window(window: &MainWindow, view_model: Rc<MainWindowViewModel>) {
    if let Some(layout) = view_model.layout() {
        apply_layout(window, &layout);
    }
    window.set_current_page(to_view_page(view_model.current_page()));
    let weak_window = window.as_weak();
    window.on_page_requested(move |page| {
        let selected = view_model.navigate(to_model_page(page));
        if let Some(window) = weak_window.upgrade() {
            window.set_current_page(to_view_page(selected));
        }
    });
}

fn apply_layout(window: &MainWindow, layout: &LayoutGroup) {
    window.window().set_size(slint::LogicalSize::new(
        layout.window_width,
        layout.window_height,
    ));
    let mut panes = window.get_pane_layout();
    let width = layout.window_width;
    let height = layout.window_height;
    let restore = |ratio: Option<f32>, extent: f32, default: f32| {
        ratio.map_or(default, |value| value * extent)
    };
    panes.navigation_width = restore(layout.navigation_ratio, width, panes.navigation_width);
    panes.agent_width = restore(layout.agent_ratio, width, panes.agent_width);
    panes.inference_records_height = restore(
        layout.inference_records_ratio,
        height,
        panes.inference_records_height,
    );
    panes.models_details_height = restore(
        layout.models_details_ratio,
        height,
        panes.models_details_height,
    );
    panes.annotation_list_width = restore(
        layout.annotation_list_ratio,
        width,
        panes.annotation_list_width,
    );
    panes.preannotation_editor_width = restore(
        layout.preannotation_editor_ratio,
        width,
        panes.preannotation_editor_width,
    );
    panes.preannotation_analysis_ratio = layout
        .preannotation_analysis_ratio
        .unwrap_or(panes.preannotation_analysis_ratio);
    panes.training_parameters_width = restore(
        layout.training_parameters_ratio,
        width,
        panes.training_parameters_width,
    );
    panes.training_log_height =
        restore(layout.training_log_ratio, height, panes.training_log_height);
    panes.training_loss_ratio = layout
        .training_loss_ratio
        .unwrap_or(panes.training_loss_ratio);
    panes.chat_composer_height = restore(
        layout.chat_composer_ratio,
        height,
        panes.chat_composer_height,
    );
    window.set_pane_layout(panes);
}

/// 将布局映射为与 Slint 无关的参数，不在 View 中读取或写入文件。
pub fn capture_main_window_layout(
    window: &MainWindow,
    view_model: &MainWindowViewModel,
) -> Result<()> {
    let width = window.get_layout_window_width();
    let height = window.get_layout_window_height();
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err(ViewModelError::operation(
            "无法记录窗口布局",
            "窗口尺寸无效",
        ));
    }
    let panes = window.get_pane_layout();
    let ratio = |value: f32, extent: f32| (value >= 0.0).then(|| (value / extent).clamp(0.0, 1.0));
    view_model.set_layout(LayoutGroup {
        window_width: width,
        window_height: height,
        navigation_ratio: ratio(panes.navigation_width, width),
        agent_ratio: ratio(panes.agent_width, width),
        inference_records_ratio: ratio(panes.inference_records_height, height),
        models_details_ratio: ratio(panes.models_details_height, height),
        annotation_list_ratio: ratio(panes.annotation_list_width, width),
        preannotation_editor_ratio: ratio(panes.preannotation_editor_width, width),
        preannotation_analysis_ratio: Some(panes.preannotation_analysis_ratio),
        training_parameters_ratio: ratio(panes.training_parameters_width, width),
        training_log_ratio: ratio(panes.training_log_height, height),
        training_loss_ratio: Some(panes.training_loss_ratio),
        chat_composer_ratio: ratio(panes.chat_composer_height, height),
    })
}

fn to_model_page(page: ViewPage) -> MainPage {
    match page {
        ViewPage::Inference => MainPage::Inference,
        ViewPage::Models => MainPage::Models,
        ViewPage::Preannotation => MainPage::Preannotation,
        ViewPage::Annotation => MainPage::Annotation,
        ViewPage::Pretraining => MainPage::Pretraining,
        ViewPage::Training => MainPage::Training,
        ViewPage::Settings => MainPage::Settings,
        ViewPage::About => MainPage::About,
    }
}

fn to_view_page(page: MainPage) -> ViewPage {
    match page {
        MainPage::Inference => ViewPage::Inference,
        MainPage::Models => ViewPage::Models,
        MainPage::Preannotation => ViewPage::Preannotation,
        MainPage::Annotation => ViewPage::Annotation,
        MainPage::Pretraining => ViewPage::Pretraining,
        MainPage::Training => ViewPage::Training,
        MainPage::Settings => ViewPage::Settings,
        MainPage::About => ViewPage::About,
    }
}

/// View 先创建窗口，再关联运行核心；基础服务就绪后才进入 UI 事件循环。
pub fn run() -> Result<()> {
    // 创建窗口失败时先保留错误，让运行核心建立日志后统一记录，避免静默丢失。
    let window =
        MainWindow::new().map_err(|error| ViewModelError::operation("无法创建主窗口", error));
    let view_model = Rc::new(MainWindowViewModel::new());
    view_model.run(|_| {
        let window = window?;
        bind_main_window(&window, Rc::clone(&view_model));
        window
            .run()
            .map_err(|error| ViewModelError::operation("界面事件循环执行失败", error))?;
        capture_main_window_layout(&window, &view_model)
    })
}
