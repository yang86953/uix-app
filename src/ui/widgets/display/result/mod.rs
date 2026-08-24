//! Result widget — 结果页，Ant Design 风格。
//!
//! 用于展示操作结果（成功/错误/警告/信息/404/403/500），
//! 包含图标、标题、副标题、额外操作区域。

use std::borrow::Cow;
use std::cell::Cell;

use crate::core::{Constraints, Rect, Size};
use crate::draw::Color;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::view::{View, ViewNode};
use crate::ui::{EventResult, KeyCode, MouseButton, SnapshotFields, SystemEvent, WidgetTree};
use crate::widget;

// 无主题上下文路径（命中测试/动作区测量）使用的默认字号，与 token font_size 默认值一致。
const RESULT_ACTION_FONT_SIZE: f32 = 14.0;

/// 结果类型。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResultType {
    /// 操作成功。
    Success,
    /// 操作失败。
    Error,
    /// 一般信息。
    Info,
    /// 警告信息。
    Warning,
    /// 资源未找到，对应 HTTP 404 语义。
    NotFound,
    /// 禁止访问，对应 HTTP 403 语义。
    Forbidden,
    /// 服务器错误，对应 HTTP 500 语义。
    ServerError,
}

impl ResultType {
    pub(crate) fn localized_title(self) -> &'static str {
        let loc = crate::ui::widget_runtime::locale::use_locale();
        match self {
            Self::Success => loc.result_success,
            Self::Error => loc.result_error,
            Self::Info => loc.result_info,
            Self::Warning => loc.result_warning,
            Self::NotFound => loc.result_404,
            Self::Forbidden => loc.result_403,
            Self::ServerError => loc.result_500,
        }
    }

    pub(crate) fn localized_subtitle(self) -> &'static str {
        let loc = crate::ui::widget_runtime::locale::use_locale();
        match self {
            Self::NotFound => loc.result_404_desc,
            Self::Forbidden => loc.result_403_desc,
            Self::ServerError => loc.result_500_desc,
            _ => "",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ResultGeometry {
    frame: Rect,
    icon: Rect,
    title: Rect,
    subtitle: Rect,
    action: Rect,
}

// ResultView — 结果页组件。
widget! {
    /// 展示状态图标、标题、副标题和可选操作文本的结果页。
    pub struct ResultView {
        type_: ResultType,
        title: String,
        subtitle: String,
        extra_text: String,
        focused: bool,
        pressed: bool,
        last_action_rect: Cell<Rect>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    tab_index => (&self) -> i32 { i32::from(!self.extra_text.is_empty()) }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        if self.extra_text.is_empty() {
            frame
        } else {
            let local_frame = Rect::new(0.0, 0.0, frame.w, frame.h);
            let action = self.layout(local_frame, RESULT_ACTION_FONT_SIZE).action;
            self.last_action_rect.set(action);
            Rect::new(
                frame.x + action.x,
                frame.y + action.y,
                action.w,
                action.h,
            )
        }
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if self.extra_text.is_empty() {
            return EventResult::NotHandled;
        }
        match event {
            SystemEvent::PointerDown { pos, button: MouseButton::Left, .. }
                if self.action_rect_local().contains(*pos) =>
            {
                self.pressed = true;
                EventResult::Handled
            }
            SystemEvent::PointerUp { pos, button: MouseButton::Left, .. } if self.pressed => {
                let activated = self.action_rect_local().contains(*pos);
                self.pressed = false;
                if activated {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerLeave if self.pressed => {
                self.pressed = false;
                EventResult::Handled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.pressed = false;
                EventResult::Handled
            }
            SystemEvent::KeyDown { key: KeyCode::Enter | KeyCode::Space, .. } => {
                self.pressed = true;
                EventResult::Handled
            }
            SystemEvent::KeyUp { key: KeyCode::Enter | KeyCode::Space, .. } => {
                self.pressed = false;
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        // 动作区布局使用主题字号 token。
        let geometry = self.layout(frame, ctx.tokens().font_size());
        if geometry.frame.w <= 0.0 || geometry.frame.h <= 0.0 {
            self.last_action_rect.set(Rect::zero());
            return;
        }
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let (main_title, sub) = self.effective_content();
        let (icon, icon_color) = match self.type_ {
            ResultType::Success => ("check", ctx.tokens().color_success()),
            ResultType::Error => ("x", ctx.tokens().color_error()),
            ResultType::Info => ("info", ctx.tokens().color_info()),
            ResultType::Warning => ("alert-triangle", ctx.tokens().color_warning()),
            ResultType::NotFound => ("404", text_sec),
            ResultType::Forbidden => ("403", ctx.tokens().color_warning()),
            ResultType::ServerError => ("500", ctx.tokens().color_error()),
        };

        ctx.push_clip(geometry.frame);
        match self.type_ {
            ResultType::NotFound | ResultType::Forbidden | ResultType::ServerError => {
                let font_size = 48.0_f32.min(geometry.icon.h * 0.82);
                ctx.text_center(icon, geometry.icon, icon_color, font_size);
            }
            _ => {
                let radius = geometry.icon.w.min(geometry.icon.h) * 0.5;
                let center_x = geometry.icon.x + geometry.icon.w * 0.5;
                let center_y = geometry.icon.y + geometry.icon.h * 0.5;
                ctx.fill_circle(center_x, center_y, radius, icon_color);
                crate::ui::widgets::icon::Icon::paint_in_frame(
                    ctx,
                    icon,
                    geometry.icon,
                    // 结果图标反白色：白色 token。
                    ctx.tokens().color_white(),
                    (radius * 0.8).max(1.0),
                );
            }
        }

        Self::paint_text_block(ctx, main_title, geometry.title, text, 20.0);
        Self::paint_text_block(ctx, sub, geometry.subtitle, text_sec, 13.0);

        if !self.extra_text.is_empty() {
            let btn_rect = geometry.action;
            self.last_action_rect.set(Rect::new(
                btn_rect.x - geometry.frame.x,
                btn_rect.y - geometry.frame.y,
                btn_rect.w,
                btn_rect.h,
            ));
            let radius_value = 6.0_f32.min(btn_rect.w.min(btn_rect.h) * 0.5);
            let radius = Some(crate::draw::Radius::uniform(radius_value));
            let background = if self.pressed {
                ctx.tokens().color_primary_active()
            } else {
                ctx.tokens().color_primary()
            };
            ctx.fill_rect(btn_rect, background, radius);
            if self.focused && tree.keyboard_focus_visible() {
                let focus = Self::inset(btn_rect, 2.0);
                ctx.stroke_rect(
                    focus,
                    // 聚焦描边：白色 token。
                    ctx.tokens().color_white(),
                    2.0,
                    Some(crate::draw::Radius::uniform(
                        radius_value.min(focus.w.min(focus.h) * 0.5),
                    )),
                );
            }
            Self::paint_action_text(ctx, &self.extra_text, btn_rect);
        } else {
            self.last_action_rect.set(Rect::zero());
        }
        ctx.pop_clip();
    }
}

impl ResultView {
    /// 创建使用指定结果类型及其本地化默认文案的结果页。
    pub fn new(type_: ResultType) -> Self {
        Self {
            type_,
            title: String::new(),
            subtitle: String::new(),
            extra_text: String::new(),
            focused: false,
            pressed: false,
            last_action_rect: Cell::new(Rect::zero()),
        }
    }
    /// 覆盖结果页标题；空文本继续使用本地化默认标题。
    pub fn title(mut self, t: &str) -> Self {
        self.title = t.to_string();
        self
    }
    /// 覆盖结果页副标题；空文本继续使用本地化默认副标题。
    pub fn subtitle(mut self, s: &str) -> Self {
        self.subtitle = s.to_string();
        self
    }
    /// 设置结果页底部的额外操作文本。
    pub fn extra_text(mut self, t: impl Into<String>) -> Self {
        self.extra_text = t.into();
        self
    }

    fn effective_content(&self) -> (&str, &str) {
        let title = if self.title.is_empty() {
            self.type_.localized_title()
        } else {
            &self.title
        };
        let subtitle = if self.subtitle.is_empty() {
            self.type_.localized_subtitle()
        } else {
            &self.subtitle
        };
        (title, subtitle)
    }

    fn layout(&self, frame: Rect, action_font_size: f32) -> ResultGeometry {
        let frame = Self::normalized_frame(frame);
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return ResultGeometry {
                frame,
                icon: Rect::zero(),
                title: Rect::zero(),
                subtitle: Rect::zero(),
                action: Rect::zero(),
            };
        }

        let horizontal_padding = 12.0_f32.min(frame.w * 0.1);
        let vertical_padding = 12.0_f32.min(frame.h * 0.08);
        let inner = Rect::new(
            frame.x + horizontal_padding,
            frame.y + vertical_padding,
            (frame.w - horizontal_padding * 2.0).max(0.0),
            (frame.h - vertical_padding * 2.0).max(0.0),
        );
        let has_action = !self.extra_text.is_empty();
        let action_height = if has_action {
            36.0_f32.min(inner.h)
        } else {
            0.0
        };
        let action_gap = if has_action {
            12.0_f32.min((inner.h - action_height).max(0.0) * 0.12)
        } else {
            0.0
        };
        let content_height = (inner.h - action_height - action_gap).max(0.0);
        let (title, subtitle) = self.effective_content();
        let has_subtitle = !subtitle.is_empty();
        let icon_fraction = if has_subtitle { 0.35 } else { 0.45 };
        let icon_size = 64.0_f32
            .min(inner.w * 0.45)
            .min(content_height * icon_fraction)
            .max(0.0);
        let icon_title_gap = 10.0_f32.min((content_height - icon_size).max(0.0) * 0.16);
        let text_height = (content_height - icon_size - icon_title_gap).max(0.0);
        let title_desired = Self::estimated_text_height(title, inner.w, 20.0, 2);
        let subtitle_desired = Self::estimated_text_height(subtitle, inner.w, 13.0, 3);
        let subtitle_gap = if has_subtitle {
            6.0_f32.min(text_height * 0.1)
        } else {
            0.0
        };
        let available_text = (text_height - subtitle_gap).max(0.0);
        let (title_height, subtitle_height) = if has_subtitle {
            let minimum_title = (20.0_f32 * 1.5).min(available_text).min(title_desired);
            let minimum_subtitle = (13.0_f32 * 1.5)
                .min((available_text - minimum_title).max(0.0))
                .min(subtitle_desired);
            let remaining = (available_text - minimum_title - minimum_subtitle).max(0.0);
            let extra_title = (title_desired - minimum_title)
                .max(0.0)
                .min(remaining * 0.35);
            let title_height = minimum_title + extra_title;
            let subtitle_height = minimum_subtitle
                + (subtitle_desired - minimum_subtitle)
                    .max(0.0)
                    .min(remaining - extra_title);
            (title_height, subtitle_height)
        } else {
            (available_text.min(title_desired), 0.0)
        };

        let used_height = icon_size
            + icon_title_gap
            + title_height
            + subtitle_gap
            + subtitle_height
            + action_gap
            + action_height;
        let mut y = inner.y + (inner.h - used_height).max(0.0) * 0.5;
        let icon = Rect::new(
            inner.x + (inner.w - icon_size) * 0.5,
            y,
            icon_size,
            icon_size,
        );
        y += icon_size + icon_title_gap;
        let title = Rect::new(inner.x, y, inner.w, title_height);
        y += title_height + subtitle_gap;
        let subtitle = Rect::new(inner.x, y, inner.w, subtitle_height);
        y += subtitle_height + action_gap;
        let action = if has_action {
            let action_width = (Self::estimated_text_width(&self.extra_text, action_font_size)
                + 32.0)
                .clamp(0.0, inner.w);
            Rect::new(
                inner.x + (inner.w - action_width) * 0.5,
                y,
                action_width,
                action_height,
            )
        } else {
            Rect::zero()
        };

        ResultGeometry {
            frame,
            icon,
            title,
            subtitle,
            action,
        }
    }

    fn normalized_frame(frame: Rect) -> Rect {
        Rect::new(
            frame.x,
            frame.y,
            if frame.w.is_finite() {
                frame.w.max(0.0)
            } else {
                0.0
            },
            if frame.h.is_finite() {
                frame.h.max(0.0)
            } else {
                0.0
            },
        )
    }

    fn inset(frame: Rect, amount: f32) -> Rect {
        let amount = amount.min(frame.w * 0.5).min(frame.h * 0.5).max(0.0);
        Rect::new(
            frame.x + amount,
            frame.y + amount,
            (frame.w - amount * 2.0).max(0.0),
            (frame.h - amount * 2.0).max(0.0),
        )
    }

    fn estimated_text_width(value: &str, font_size: f32) -> f32 {
        let value = Self::normalized_action_text(value);
        crate::draw::resources::font::text_backend::estimate_text_metrics(
            &value,
            f32::INFINITY,
            font_size,
        )
        .max_line_width
    }

    fn normalized_action_text(value: &str) -> Cow<'_, str> {
        if value.contains(['\r', '\n']) {
            Cow::Owned(value.replace(['\r', '\n'], " "))
        } else {
            Cow::Borrowed(value)
        }
    }

    fn estimated_text_height(value: &str, width: f32, font_size: f32, max_lines: usize) -> f32 {
        if value.is_empty() || width <= 0.0 {
            return 0.0;
        }
        let line_count = crate::draw::resources::font::text_backend::estimate_text_metrics(
            value, width, font_size,
        )
        .line_count
        .clamp(1, max_lines);
        line_count as f32 * font_size * 1.5
    }

    fn paint_text_block(
        ctx: &mut PaintContext,
        value: &str,
        frame: Rect,
        color: Color,
        font_size: f32,
    ) {
        if value.is_empty() || frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let line_height = font_size * 1.5;
        let visible_lines = (frame.h / line_height).floor() as usize;
        if visible_lines == 0 {
            return;
        }
        let metrics = crate::draw::resources::font::text_backend::estimate_text_metrics(
            value, frame.w, font_size,
        );
        ctx.push_clip(frame);
        if metrics.line_count <= 1 {
            ctx.text_center(value, frame, color, font_size);
        } else if visible_lines >= 2 {
            ctx.draw_text_wrapped(value, frame, color, font_size);
        // 复用 UI 绘制上下文拥有的保守单行省略算法。
        } else if let Some(value) = ctx.elide_single_line(value, font_size, frame.w) {
            ctx.text_center(&value, frame, color, font_size);
        }
        ctx.pop_clip();
    }

    fn paint_action_text(ctx: &mut PaintContext, value: &str, frame: Rect) {
        let horizontal_padding = 16.0_f32.min(frame.w * 0.25);
        let content = Rect::new(
            frame.x + horizontal_padding,
            frame.y,
            (frame.w - horizontal_padding * 2.0).max(0.0),
            frame.h,
        );
        // 在借用可变上下文前先解析当前主题字号。
        let font_size = ctx.tokens().font_size();
        // 复用共享省略算法生成结果描述的可见文本。
        if let Some(value) = ctx.elide_single_line(value, font_size, content.w) {
            ctx.push_clip(content);
            // 结果描述文本：白色 token。
            ctx.text_center(
                &value,
                content,
                ctx.tokens().color_white(),
                ctx.tokens().font_size(),
            );
            ctx.pop_clip();
        }
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(400.0, 300.0)
    }

    fn action_rect_local(&self) -> Rect {
        let rendered = self.last_action_rect.get();
        if rendered.w > 0.0 && rendered.h > 0.0 {
            return rendered;
        }
        let size = self.intrinsic_size();
        self.layout(Rect::new(0.0, 0.0, size.w, size.h), RESULT_ACTION_FONT_SIZE)
            .action
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Result {
            result_type: self.type_,
            title: self.title.clone(),
            subtitle: self.subtitle.clone(),
            extra_text: self.extra_text.clone(),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let action_changed = self.extra_text != next.extra_text;
        self.type_ = next.type_;
        self.title = next.title;
        self.subtitle = next.subtitle;
        self.extra_text = next.extra_text;
        if action_changed {
            self.last_action_rect.set(Rect::zero());
        }
        if self.extra_text.is_empty() {
            self.focused = false;
            self.pressed = false;
        }
    }
}

impl Default for ResultView {
    fn default() -> Self {
        Self::new(ResultType::Info)
    }
}

// 把结果页 Rust 交互与绘制内核融合为 UIX 声明的单一叶节点。
fn build_result_view(kernel: ResultView) -> ViewNode {
    ViewNode::leaf(kernel)
}

impl View for ResultView {
    fn build(self) -> ViewNode {
        // UIX 拥有公开组件根，Rust 保留本地化、交互、布局和绘制机制。
        let kernel = self;
        crate::uix!("src/ui/widgets/display/result/result.uix")
    }
}

// 集中验证 UIX 声明壳与 Rust 内核的单节点契约。
#[cfg(test)]
#[path = "../../../../../tests/unit/ui/widgets/display/result__tests.rs"]
mod tests;
