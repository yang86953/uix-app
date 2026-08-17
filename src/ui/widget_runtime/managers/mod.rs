//! Manager 系统 — 注入 WidgetTree，为布局、渲染与事件处理提供横切能力。
//! 遵循组合优于继承：各 manager 通过接口对外暴露能力。

use std::collections::HashMap;

use crate::ui::ComponentId;

mod drag_manager;
mod focus_manager;
mod interaction_manager;
mod state_manager;
mod style_manager;
mod text_manager;

pub use drag_manager::*;
pub use focus_manager::*;
pub use interaction_manager::*;
pub use state_manager::*;
pub(crate) use style_manager::*;
pub use text_manager::*;

/// 聚合所有 manager 系统。
/// 支持 per-component override：个别 component 可使用独立实例，否则回退到树级默认。
#[derive(Default)]
pub struct WidgetManagers {
    /// 树级默认的类型化组件状态槽管理器。
    pub state: StateManager,
    /// Legacy style preset manager. Prefer `ui::style::Style` on components.
    pub style: StyleManager,
    /// 树级默认的文本内容与绘制属性管理器。
    pub text: TextManager,
    /// 组件树的指针交互状态管理器。
    pub interaction: InteractionManager,
    /// 组件树的键盘焦点与 Tab 顺序管理器。
    pub focus: FocusManager,
    /// 组件树的拖放手势状态管理器。
    pub drag: DragManager,
    /// Per-component manager overrides keyed by ComponentId.
    overrides: HashMap<ComponentId, Box<WidgetManagersOverrides>>,
}

/// 可按 component 单独 override 的 manager 子集。
#[derive(Default)]
struct WidgetManagersOverrides {
    state: Option<StateManager>,
    style: Option<StyleManager>,
    text: Option<TextManager>,
}

impl WidgetManagers {
    /// 创建全部子管理器均为默认状态且没有组件级覆写的聚合器。
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the state manager for a specific component.
    pub fn override_state(&mut self, component_id: ComponentId, mgr: StateManager) {
        self.overrides.entry(component_id).or_default().state = Some(mgr);
    }

    /// Override the legacy style manager for a specific component.
    pub fn override_style(&mut self, component_id: ComponentId, mgr: StyleManager) {
        self.overrides.entry(component_id).or_default().style = Some(mgr);
    }

    /// Override the text manager for a specific component.
    pub fn override_text(&mut self, component_id: ComponentId, mgr: TextManager) {
        self.overrides.entry(component_id).or_default().text = Some(mgr);
    }

    /// Remove all overrides for a component.
    pub fn remove_overrides(&mut self, component_id: ComponentId) {
        self.overrides.remove(&component_id);
    }

    /// Remove every per-component override from this tree.
    pub fn clear_overrides(&mut self) {
        self.overrides.clear();
    }

    /// Get the effective state manager for a component (override or default).
    pub fn state_for(&self, component_id: ComponentId) -> &StateManager {
        self.overrides
            .get(&component_id)
            .and_then(|o| o.state.as_ref())
            .unwrap_or(&self.state)
    }

    /// Get the effective legacy style manager for a component (override or default).
    pub fn style_for(&self, component_id: ComponentId) -> &StyleManager {
        self.overrides
            .get(&component_id)
            .and_then(|o| o.style.as_ref())
            .unwrap_or(&self.style)
    }

    /// Get the effective text manager for a component (override or default).
    pub fn text_for(&self, component_id: ComponentId) -> &TextManager {
        self.overrides
            .get(&component_id)
            .and_then(|o| o.text.as_ref())
            .unwrap_or(&self.text)
    }
}
