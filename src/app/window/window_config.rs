use crate::ui::view::ViewNode;

pub type WindowRootFactory = Box<dyn Fn() -> ViewNode + Send + Sync + 'static>;

/// Runtime configuration for a secondary application window.
pub struct WindowConfig {
    pub title: String,
    pub width: i32,
    pub height: i32,
    pub custom_title_bar: bool,
    pub root: WindowRootFactory,
}

impl WindowConfig {
    pub fn new<F>(title: impl Into<String>, width: i32, height: i32, root: F) -> Self
    where
        F: Fn() -> ViewNode + Send + Sync + 'static,
    {
        Self {
            title: title.into(),
            width,
            height,
            custom_title_bar: false,
            root: Box::new(root),
        }
    }

    /// 隐藏系统标题栏，由窗口根 View 提供标题栏外观与交互区域。
    pub fn custom_title_bar(mut self, enabled: bool) -> Self {
        self.custom_title_bar = enabled;
        self
    }
}
