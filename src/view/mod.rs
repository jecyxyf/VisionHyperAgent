//! 原生 Slint 窗口入口；当前只承载已有界面，不接入训练、推理或 Agent 服务。
use crate::view_model::{MainPage, MainWindowViewModel, Result, ViewModelError};
use std::rc::Rc;

slint::include_modules!();

/// View 的绑定适配：导航请求交给主窗口 ViewModel，不把 UI 类型带入运行逻辑。
pub fn bind_main_window(window: &MainWindow, view_model: Rc<MainWindowViewModel>) {
    window.set_current_page(to_view_page(view_model.current_page()));
    let weak_window = window.as_weak();
    window.on_page_requested(move |page| {
        let selected = view_model.navigate(to_model_page(page));
        if let Some(window) = weak_window.upgrade() {
            window.set_current_page(to_view_page(selected));
        }
    });
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
            .map_err(|error| ViewModelError::operation("界面事件循环执行失败", error))
    })
}
