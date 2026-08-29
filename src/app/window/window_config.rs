pub use crate::platform::windowing::{DesktopLayerConfig, WindowSurfaceRole};
use crate::ui::view::ViewNode;

pub(crate) type WindowRootFactory = Box<dyn Fn() -> ViewNode + Send + Sync + 'static>;

/// Runtime configuration for a secondary application window.
pub struct WindowConfig {
    /// 平台窗口标题。
    pub title: String,
    /// 请求的初始客户区宽度。
    pub width: i32,
    /// 请求的初始客户区高度。
    pub height: i32,
    /// 是否隐藏系统标题栏并由根 View 提供标题栏。
    pub custom_title_bar: bool,
    /// 平台窗口或桌面 layer surface 角色。
    pub surface_role: WindowSurfaceRole,
    /// 在窗口建立时创建其根 View 的线程安全工厂。
    pub root: WindowRootFactory,
}

impl WindowConfig {
    /// 创建使用系统标题栏的次窗口运行时配置。
    pub fn new<F>(title: impl Into<String>, width: i32, height: i32, root: F) -> Self
    where
        F: Fn() -> ViewNode + Send + Sync + 'static,
    {
        Self {
            title: title.into(),
            width,
            height,
            custom_title_bar: false,
            surface_role: WindowSurfaceRole::Toplevel,
            root: Box::new(root),
        }
    }

    /// 隐藏系统标题栏，由窗口根 View 提供标题栏外观与交互区域。
    pub fn custom_title_bar(mut self, enabled: bool) -> Self {
        self.custom_title_bar = enabled;
        self
    }

    /// 把次窗口声明为普通 toplevel 或桌面 layer surface。
    pub fn surface_role(mut self, role: WindowSurfaceRole) -> Self {
        self.surface_role = role;
        self
    }

    /// 使用给定配置创建桌面 layer surface。
    pub fn desktop_layer(self, config: DesktopLayerConfig) -> Self {
        self.surface_role(WindowSurfaceRole::DesktopLayer(config))
    }
}
