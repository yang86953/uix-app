use crate::define_widget;
use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetId, WidgetTree};
use uix_platform::{KeyCode, KeyMod, Rect, Size};
use std::collections::HashSet;

define_widget! {
    /// FocusTrap — 将键盘 Tab/Shift+Tab 焦点限制在子树内。
    ///
    /// 用于 Modal、Drawer 等弹出式容器，确保焦点不会逃逸到遮罩后方。
    pub struct FocusTrap {
        /// 是否激活焦点锁定。
        active: bool,
        /// 上次渲染时捕获的焦点 widgets。
        focusable_ids: HashSet<WidgetId>,
        /// 内部焦点循环偏移量。
        tab_offset: usize,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        Size::new(0.0, 0.0)
    }

    render => (&self, _frame: Rect, _ctx: &mut RenderContext, _tree: &WidgetTree) {}

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if !self.active {
            return EventResult::NotHandled;
        }
        if let WidgetEvent::KeyDown { key, mods } = event {
            if *key == KeyCode::Tab {
                let mut sorted: Vec<WidgetId> = self.focusable_ids.iter().copied().collect();
                sorted.sort();
                if sorted.is_empty() {
                    return EventResult::Handled;
                }
                let shift = mods.contains(KeyMod::SHIFT);
                if shift {
                    self.tab_offset = if self.tab_offset == 0 {
                        sorted.len() - 1
                    } else {
                        self.tab_offset - 1
                    };
                } else {
                    self.tab_offset = (self.tab_offset + 1) % sorted.len();
                }
                return EventResult::Handled;
            }
        }
        EventResult::NotHandled
    }

    on_update => (&mut self, _dt: f64) {}
}

impl Default for FocusTrap {
    fn default() -> Self {
        Self::new()
    }
}

impl FocusTrap {
    pub fn new() -> Self {
        Self {
            active: true,
            focusable_ids: HashSet::new(),
            tab_offset: 0,
        }
    }

    pub fn active(mut self, v: bool) -> Self {
        self.active = v;
        self
    }

    /// 更新可聚焦 widget 列表（由外部在布局后调用）。
    pub fn update_focusable(&mut self, ids: HashSet<WidgetId>) {
        self.focusable_ids = ids;
    }

    pub fn wrap(self, node: crate::widget::WidgetNode) -> crate::widget::WidgetNode {
        crate::widget::WidgetNode::new(Box::new(self), vec![node])
    }
}
