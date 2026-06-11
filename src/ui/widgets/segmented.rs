//! Segmented widget — 分段选择器。

use crate::define_widget;
use crate::base::{Rect, Size};
use crate::graphics::Radius;
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, WidgetEvent, WidgetTree};

define_widget! {
    /// Segmented — 水平分段选择器。
    pub struct Segmented {
        options: Vec<String>,
        selected: usize,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        let w = self.options.iter().map(|o| o.len() as f32 * 9.0 + 24.0).sum::<f32>();
        Size::new(w, 32.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if let WidgetEvent::MouseDown { pos, .. } = event {
            let mut cum_x = 0.0f32;
            for (i, opt) in self.options.iter().enumerate() {
                let seg_w = opt.len() as f32 * 9.0 + 24.0;
                if pos.x >= cum_x && pos.x <= cum_x + seg_w && pos.y >= 0.0 && pos.y <= 32.0 {
                    self.selected = i;
                    return EventResult::Handled;
                }
                cum_x += seg_w;
            }
        }
        EventResult::NotHandled
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let fill = ctx.tokens().color_fill_tertiary();
        let primary = ctx.tokens().color_primary();
        let text_secondary = ctx.tokens().color_text_secondary();
        let bg = ctx.tokens().color_bg_elevated();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));

        // 整体背景
        ctx.fill_rect(frame, fill, r);

        let mut x = frame.x;

        for (i, opt) in self.options.iter().enumerate() {
            let seg_w = opt.len() as f32 * 9.0 + 24.0;
            if i == self.selected {
                // 选中项：白色背景 + 主色文字
                ctx.fill_rect(Rect::new(x + 2.0, frame.y + 2.0, seg_w - 4.0, 28.0), bg, Some(Radius::uniform(3.0)));
                ctx.text_center(opt, Rect::new(x, frame.y, seg_w, 32.0), primary, 13.0);
            } else {
                ctx.text_center(opt, Rect::new(x, frame.y, seg_w, 32.0), text_secondary, 13.0);
            }
            x += seg_w;
        }
    }
}

impl Default for Segmented { fn default() -> Self { Self::new() } }

impl Segmented {
    pub fn new() -> Self {
        Self { options: Vec::new(), selected: 0 }
    }
    pub fn options(mut self, opts: Vec<impl Into<String>>) -> Self {
        self.options = opts.into_iter().map(|s| s.into()).collect();
        self
    }
    pub fn selected(mut self, idx: usize) -> Self { self.selected = idx; self }
}
