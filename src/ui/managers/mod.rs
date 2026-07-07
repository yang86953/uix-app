//! Manager 系统 — 注入 WidgetTree，为布局、渲染与事件处理提供横切能力。
//! 遵循组合优于继承：各 manager 通过接口对外暴露能力。

use std::collections::HashMap;

use crate::ui::WidgetId;

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
pub use style_manager::*;
pub use text_manager::*;

/// 聚合所有 manager 系统。
/// 支持 per-widget override：个别 widget 可使用独立实例，否则回退到树级默认。
#[derive(Default)]
pub struct WidgetManagers {
    pub state: StateManager,
    pub style: StyleManager,
    pub text: TextManager,
    pub interaction: InteractionManager,
    pub focus: FocusManager,
    pub drag: DragManager,
    /// Per-widget manager overrides keyed by WidgetId.
    overrides: HashMap<WidgetId, Box<WidgetManagersOverrides>>,
}

/// 可按 widget 单独 override 的 manager 子集。
#[derive(Default)]
struct WidgetManagersOverrides {
    state: Option<StateManager>,
    text: Option<TextManager>,
}

impl WidgetManagers {
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the state manager for a specific widget.
    pub fn override_state(&mut self, widget_id: WidgetId, mgr: StateManager) {
        self.overrides.entry(widget_id).or_default().state = Some(mgr);
    }

    /// Override the text manager for a specific widget.
    pub fn override_text(&mut self, widget_id: WidgetId, mgr: TextManager) {
        self.overrides.entry(widget_id).or_default().text = Some(mgr);
    }

    /// Remove all overrides for a widget.
    pub fn remove_overrides(&mut self, widget_id: WidgetId) {
        self.overrides.remove(&widget_id);
    }

    /// Remove every per-widget override from this tree.
    pub fn clear_overrides(&mut self) {
        self.overrides.clear();
    }

    /// Get the effective state manager for a widget (override or default).
    pub fn state_for(&self, widget_id: WidgetId) -> &StateManager {
        self.overrides
            .get(&widget_id)
            .and_then(|o| o.state.as_ref())
            .unwrap_or(&self.state)
    }

    /// Get the effective text manager for a widget (override or default).
    pub fn text_for(&self, widget_id: WidgetId) -> &TextManager {
        self.overrides
            .get(&widget_id)
            .and_then(|o| o.text.as_ref())
            .unwrap_or(&self.text)
    }
}
