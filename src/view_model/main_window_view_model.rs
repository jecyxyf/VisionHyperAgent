//! 主窗口的导航状态和子模块协调，不代理配置参数操作。
use super::{ConfigViewModel, LayoutGroup, Result, ViewModelError};
use crate::foundation::{self, APP_PARAM, LOGGER};
use std::cell::{Cell, RefCell};

/// 纯 Rust 页面标识，由 View 适配为具体界面的页面类型。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MainPage {
    #[default]
    Inference,
    Models,
    Preannotation,
    Annotation,
    Pretraining,
    Training,
    Settings,
    About,
}

/// 主窗口交互状态按所属线程访问；不持有窗口、控件或 UI 回调。
#[derive(Default)]
pub struct MainWindowViewModel {
    current_page: Cell<MainPage>,
    config_view_model: ConfigViewModel,
    layout: RefCell<Option<LayoutGroup>>,
}

impl MainWindowViewModel {
    /// 构造导航状态与子 ViewModel，不进行文件 I/O。
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current_page(&self) -> MainPage {
        self.current_page.get()
    }

    /// 处理主窗口的页面切换请求，返回应呈现的页面。
    pub fn navigate(&self, page: MainPage) -> MainPage {
        self.current_page.set(page);
        page
    }

    /// 提供子模块以便对应 View 绑定；配置读写由 ConfigViewModel 自身负责。
    pub fn config_view_model(&self) -> &ConfigViewModel {
        &self.config_view_model
    }

    /// 无 UI 的布局状态接口；构造后尚未加载时返回 None。
    pub fn layout(&self) -> Option<LayoutGroup> {
        self.layout.borrow().clone()
    }

    /// View 提交纯数据；文件保存仍由运行期协调，不在拖拽时写盘。
    pub fn set_layout(&self, layout: LayoutGroup) -> Result<()> {
        layout.validate()?;
        *self.layout.borrow_mut() = Some(layout);
        Ok(())
    }

    /// 协调主窗口运行期与子模块初始化；task 也可以是不创建 UI 的调用方。
    /// 日志是共享基础服务，由此作用域自动收尾，不支持嵌套或并发托管。
    pub fn run<T>(&self, task: impl FnOnce(&Self) -> Result<T>) -> Result<T> {
        foundation::with_logging(|| {
            if let Some(notice) = self.config_view_model.init().map_err(|error| error.0)? {
                LOGGER.warning("config", &notice)?;
            }
            if let Some(notice) = APP_PARAM.init()? {
                LOGGER.warning("app_param", &notice)?;
            }
            *self.layout.borrow_mut() = APP_PARAM.layout()?;
            let result = task(self).map_err(|error| error.0)?;
            if let Some(layout) = self.layout() {
                APP_PARAM.set_layout(layout)?;
                APP_PARAM.save()?;
            }
            Ok(result)
        })
        .map_err(ViewModelError::from)
    }
}
