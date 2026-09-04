//! 响应式文本标签 — 文本闭包绑定 State → Layout 失效的框架级组件。
//!
//! # SMC 边界（SMC-04）
//!
//! `DynamicLabel` 是框架级基础设施（原 `view::combinators::DynamicLabel`）：
//! 树（tree_dirty / 无障碍）需要在 build 期识别并绑定其 State 依赖，归
//! widget Module 避免 widget → view / widgets 依赖。

use std::any::Any;

use crate::core::{Constraints, Rect, Size};
use crate::draw::scene::PicturePolicy;
use crate::ui::theme::style::Style;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::traits::{Widget, WidgetCapabilities, WidgetLayout, WidgetRender};
use crate::ui::widget_runtime::widget::WidgetTree;

/// 响应式标签的内部 Widget 实现。
pub(crate) struct DynamicLabel {
    text_fn: Box<dyn Fn() -> String>,
    style: Option<Style>,
}

impl DynamicLabel {
    pub(crate) fn new<F: Fn() -> String + 'static>(f: F) -> Self {
        Self {
            text_fn: Box::new(f),
            style: None,
        }
    }

    pub(crate) fn set_style(&mut self, style: Style) {
        self.style = Some(style);
    }

    /// layout 后探测闭包依赖：执行一次文本闭包以捕获 `State::get()`。
    pub(crate) fn probe_dependencies(&self) {
        let _ = (self.text_fn)();
    }

    pub(crate) fn semantic_text(&self) -> String {
        (self.text_fn)()
    }
}

impl Widget for DynamicLabel {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
    fn capabilities(&self) -> WidgetCapabilities {
        let mut c = WidgetCapabilities::new();
        c.insert(WidgetCapabilities::LAYOUT);
        c.insert(WidgetCapabilities::RENDER);
        c
    }
    fn picture_policy(&self) -> PicturePolicy {
        PicturePolicy::Never
    }
    fn has_dynamic_content(&self) -> bool {
        true
    }
    fn may_produce_overlay(&self) -> bool {
        false
    }
    fn as_layout(&self) -> Option<&dyn WidgetLayout> {
        Some(self)
    }
    fn as_render(&self) -> Option<&dyn WidgetRender> {
        Some(self)
    }
    fn as_render_mut(&mut self) -> Option<&mut dyn WidgetRender> {
        Some(self)
    }
}

impl WidgetLayout for DynamicLabel {
    fn measure(&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size(constraints))
    }

    fn layout_margin(&self) -> crate::core::EdgeInsets {
        self.style.as_ref().map(|s| s.margin).unwrap_or_default()
    }

    fn align_self(&self) -> Option<crate::ui::layout::AlignItems> {
        self.style.as_ref().and_then(|s| s.align_self)
    }

    fn flex_grow(&self) -> f32 {
        self.style.as_ref().map(|s| s.flex_grow).unwrap_or(0.0)
    }

    fn flex_shrink(&self) -> f32 {
        self.style.as_ref().map(|s| s.flex_shrink).unwrap_or(0.0)
    }
}

impl DynamicLabel {
    fn intrinsic_size(&self, constraints: Constraints) -> Size {
        let pad = self.style.as_ref().map(|s| s.padding).unwrap_or_default();
        if let Some(style) = &self.style {
            if let (Some(w), Some(h)) = (style.width, style.height) {
                return Size::new(w, h);
            }
        }
        let text = (self.text_fn)();
        let fs = self
            .style
            .as_ref()
            .map(|s| s.font_size.default_size())
            .unwrap_or(14.0);
        // 换行宽度：显式样式宽度优先，否则用布局约束的有限宽度参与估算；
        // 无显式高度时行数随折行增长，保持与绘制同一行距契约。
        let available_width = self
            .style
            .as_ref()
            .and_then(|s| s.width)
            .unwrap_or(constraints.max.w);
        let wrap_width = (available_width - pad.horizontal()).max(0.0);
        // 动态文本与静态 Label 共用显式换行估算，避免布局仍按单行占位。
        let estimated = crate::draw::resources::font::text_backend::estimate_text_metrics(
            &text,
            wrap_width,
            fs,
        );
        // 动态标签绘制固定使用 1.5 倍字号行盒，测量保持同一行距契约。
        let text_height = fs * 1.5 * estimated.line_count as f32;
        let h = self
            .style
            .as_ref()
            .and_then(|s| s.height)
            .unwrap_or(text_height + pad.vertical());
        let w = self
            .style
            .as_ref()
            .and_then(|s| s.width)
            .unwrap_or(if estimated.width_wrapped {
                available_width
            } else {
                (estimated.max_line_width + pad.horizontal()).min(available_width)
            });
        Size::new(w, h)
    }
}

impl WidgetRender for DynamicLabel {
    fn render(&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let text = (self.text_fn)();
        if text.is_empty() {
            return;
        }
        let style = self.style.as_ref();
        let color = style
            .map(|s| s.resolve_color(ctx.tokens()))
            .unwrap_or_else(|| ctx.tokens().color_text());
        let font_size = style
            .map(|s| s.resolve_font_size(ctx.tokens()))
            .unwrap_or(14.0);
        let padding = style.map(|s| s.padding).unwrap_or_default();
        // 约束矩形内自动换行；首行顶左位置与单行绘制保持一致。
        let rect = Rect::new(
            frame.x + padding.left,
            frame.y + padding.top,
            (frame.w - padding.horizontal()).max(0.0),
            (frame.h - padding.vertical()).max(0.0),
        );
        ctx.draw_text_wrapped(&text, rect, color, font_size);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::style::Style;

    // 宽松约束保持单行内在尺寸。
    #[test]
    fn single_line_with_unbounded_width() {
        let label = DynamicLabel::new(|| "标题".to_string());
        let size = label.measure(Constraints::loose(Size::infinite()));
        assert_eq!(size.h, 14.0 * 1.5);
        assert!(size.w > 0.0);
    }

    // 有限约束宽度内折行：行数增长、宽度不越界。
    #[test]
    fn wraps_within_finite_constraint() {
        let label = DynamicLabel::new(|| "一首特别特别特别长的歌曲标题超出了侧栏可用宽度".to_string());
        let single = label.measure(Constraints::loose(Size::infinite()));
        let wrapped = label.measure(Constraints::new(
            Size::zero(),
            Size::new(80.0, f32::INFINITY),
            None,
        ));
        assert!(wrapped.h > single.h, "约束变窄后高度应随折行增长");
        assert!(wrapped.w <= 80.0, "折行后宽度不得超出约束");
    }

    // 显式宽高仍然是固定尺寸契约。
    #[test]
    fn explicit_size_wins() {
        let mut style = Style::default();
        style.width = Some(48.0);
        style.height = Some(24.0);
        let mut label = DynamicLabel::new(|| "任意长度文本".to_string());
        label.set_style(style);
        let size = label.measure(Constraints::loose(Size::infinite()));
        assert_eq!(size.w, 48.0);
        assert_eq!(size.h, 24.0);
    }
}
