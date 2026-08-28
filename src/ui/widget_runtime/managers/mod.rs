//! Manager 系统 — 注入 WidgetTree，为布局、渲染与事件处理提供横切能力。
//! 遵循组合优于继承：各 manager 通过接口对外暴露能力。

mod drag_manager;
mod focus_manager;
mod interaction_manager;

pub use drag_manager::*;
pub use focus_manager::*;
pub use interaction_manager::*;

/// 聚合树级横切运行态 manager。
#[derive(Default)]
pub struct WidgetManagers {
    /// 组件树的指针交互状态管理器。
    pub interaction: InteractionManager,
    /// 组件树的键盘焦点与 Tab 顺序管理器。
    pub focus: FocusManager,
    /// 组件树的拖放手势状态管理器。
    pub drag: DragManager,
}

impl WidgetManagers {
    /// 创建全部子管理器均为默认状态的聚合器。
    pub fn new() -> Self {
        Self::default()
    }
}
