//! 响应式文本标签 — 文本闭包绑定 State → Layout 失效的框架级组件。
//!
//! # SMC 边界（SMC-04）
//!
//! `DynamicLabel` 是框架级基础设施（原 `view::combinators::DynamicLabel`）：
//! 树（tree_dirty / 无障碍）需要在 build 期识别并绑定其 State 依赖，归
//! widget Module 避免 widget → view / widgets 依赖。

use std::any::Any;

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::resources::font::text_backend::TextLayoutOptions;
use crate::draw::scene::PicturePolicy;
use crate::ui::theme::style::Style;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::traits::{Widget, WidgetCapabilities, WidgetLayout, WidgetRender};
use crate::ui::widget_runtime::widget::WidgetTree;

/// 响应式标签的内部 Widget 实现。
pub(crate) struct DynamicLabel {
    text_fn: Box<dyn Fn() -> String>,
    style: Option<Style>,
    ellipsis: bool,
}

impl DynamicLabel {
    pub(crate) fn new<F: Fn() -> String + 'static>(f: F) -> Self {
        Self {
            text_fn: Box::new(f),
            style: None,
            ellipsis: false,
        }
    }

    pub(crate) fn elided(mut self) -> Self {
        self.ellipsis = true;
        self
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
        let text = if self.ellipsis {
            text.replace(['\r', '\n'], " ")
        } else {
            text
        };
        let fs = super::measurement::with_measurement_tokens::<Self, _>(|tokens| {
            self.style
                .as_ref()
                .map(|s| s.resolve_font_size(tokens))
                .unwrap_or_else(|| tokens.font_size())
        });
        let fs = normalized_font_size(fs);
        // 换行宽度：显式样式宽度优先，否则用布局约束的有限宽度参与估算；
        // 无显式高度时行数随折行增长，保持与绘制同一行距契约。
        let available_width = self
            .style
            .as_ref()
            .and_then(|s| s.width)
            .unwrap_or(constraints.max.w)
            .min(constraints.max.w);
        let wrap_width = (available_width - pad.horizontal()).max(0.0);
        // 行盒与真实字体测量使用和绘制相同的最终样式。
        let line_height = self
            .style
            .as_ref()
            .and_then(|s| s.resolve_line_height(fs))
            .unwrap_or(fs * 1.5);
        let estimated = super::measurement::text_metrics(
            &text,
            wrap_width,
            fs,
            line_height,
            self.style.as_ref().and_then(|s| s.font_family.as_ref()),
            !self.ellipsis,
        );
        let text_height = line_height * estimated.line_count as f32;
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
        if let Some(style) = &self.style {
            crate::ui::theme::style::apply_style(ctx, frame, style);
        }
        if text.is_empty() {
            return;
        }
        let style = self.style.as_ref();
        let color = style
            .map(|s| s.resolve_color(ctx.tokens()))
            .unwrap_or_else(|| ctx.tokens().color_text());
        let font_size = style
            .map(|s| s.resolve_font_size(ctx.tokens()))
            .unwrap_or_else(|| ctx.tokens().font_size());
        let font_size = normalized_font_size(font_size);
        let padding = style.map(|s| s.padding).unwrap_or_default();
        // 约束矩形内自动换行；首行顶左位置与单行绘制保持一致。
        let rect = Rect::new(
            frame.x + padding.left,
            frame.y + padding.top,
            (frame.w - padding.horizontal()).max(0.0),
            (frame.h - padding.vertical()).max(0.0),
        );
        if rect.w <= 0.0 || rect.h <= 0.0 {
            return;
        }
        let options = TextLayoutOptions {
            max_width: rect.w,
            max_height: rect.h,
            font_size,
            line_height: style
                .and_then(|s| s.resolve_line_height(font_size))
                .unwrap_or(font_size * 1.5),
            word_wrap: !self.ellipsis,
            h_align: style
                .map(Style::effective_text_align)
                .unwrap_or_default()
                .to_draw(),
            v_align: crate::draw::VAlign::Top,
        };
        let font = crate::ui::text_family::resolve(ctx, style.and_then(|s| s.font_family.as_ref()));
        // 省略只改变绘制文本；语义和 State 依赖仍保存完整原文。
        let visible = if self.ellipsis {
            let mut natural = options.clone();
            natural.max_width = 0.0;
            natural.h_align = crate::draw::HAlign::Left;
            let Some(visible) =
                super::paint_context::elide_single_line_cow_by(&text, rect.w, |candidate| {
                    ctx.font_service()
                        .layout_text_shared(&font, candidate, &natural)
                        .width
                })
            else {
                return;
            };
            visible
        } else {
            std::borrow::Cow::Borrowed(text.as_str())
        };
        let layout = ctx
            .font_service()
            .layout_text_shared(&font, &visible, &options);
        let origin = Point::new(rect.x, rect.y);
        let decoration = crate::ui::text_decoration::segments(
            &layout,
            origin,
            font_size,
            style
                .map(Style::effective_text_decoration)
                .unwrap_or_default(),
        );
        ctx.push_clip(rect);
        crate::ui::text_weight::paint(
            ctx,
            &layout,
            origin,
            color,
            font_size,
            style.map(Style::effective_font_weight).unwrap_or_default(),
        );
        crate::ui::text_decoration::paint(ctx, &decoration, color);
        ctx.pop_clip();
    }
}

fn normalized_font_size(size: f32) -> f32 {
    if size.is_finite() && size > 0.0 {
        size
    } else {
        14.0
    }
}
