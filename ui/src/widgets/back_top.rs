//! BackTop 回到顶部 — 滚动超过阈值时显示返回顶部按钮。

use uix_core::{Point, Rect, Size};
use crate::define_widget;
use uix_graphics::{Color, GraphicsEngine, Radius};
use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetTree};

define_widget! {
    /// BackTop — 回到顶部按钮。
    pub struct BackTop {
        /// 滚动超过此高度才显示
        visibility_height: f32,
        /// 是否可见
        visible: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::new(40.0, 40.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        // 点击回到顶部
        if let WidgetEvent::MouseDown { .. } = event {
            EventResult::Handled
        } else {
            EventResult::NotHandled
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        if !self.visible { return; }

        let primary = ctx.tokens().color_primary();
        let bg_elevated = ctx.tokens().color_bg_elevated();

        // 圆形按钮
        let cx = frame.x + frame.w * 0.5;
        let cy = frame.y + frame.h * 0.5;
        let r = frame.w.min(frame.h) * 0.4;

        ctx.fill_circle(cx, cy, r, primary);
        ctx.fill_circle(cx, cy, r - 2.0, bg_elevated);
        // ↑ 箭头
        ctx.draw_text("↑", Point::new(cx - 5.0, cy - 7.0), primary, 14.0);
    }
}

impl BackTop {
    pub fn new() -> Self {
        Self { visibility_height: 400.0, visible: false }
    }

    /// 由外部 ScrollView 或 on_update 调用，传入当前 scroll_y
    pub fn update_visibility(&mut self, scroll_y: f32) {
        self.visible = scroll_y > self.visibility_height;
    }

    pub fn visibility_height(mut self, v: f32) -> Self { self.visibility_height = v; self }
    pub fn is_visible(&self) -> bool { self.visible }
}
