//! Alert widget — 警示条，支持类型、图标、关闭。

use std::cell::Cell;

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Radius;
use crate::native::traits::system::StatusLevel;
use crate::ui::core::widget::WidgetTree;
use crate::ui::SnapshotFields;
use crate::ui::{ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlertPress {
    Pointer,
    Key(KeyCode),
}

#[derive(Debug, Clone, Copy)]
struct AlertLayout {
    frame: Rect,
    accent: Rect,
    icon: Rect,
    message: Rect,
    description: Rect,
    close: Rect,
}

component! {
    /// Alert — 带类型颜色的警示条。
    pub struct Alert {
        message: String,
        description: String,
        type_: StatusLevel,
        closable: bool,
        show_icon: bool,
        visible: bool,
        focused: bool,
        close_hovered: Cell<bool>,
        pressed: Cell<Option<AlertPress>>,
        last_size: Cell<Size>,
        layout_requested: Cell<bool>,
        pending_close: Cell<bool>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        if self.visible {
            constraints.clamp(self.intrinsic_size())
        } else {
            Size::zero()
        }
    }

    visible => (&self) -> bool { self.visible }

    tab_index => (&self) -> i32 { i32::from(self.visible && self.closable) }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if !self.visible || !self.closable {
            return EventResult::NotHandled;
        }
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } if self.close_rect().contains(*pos) => {
                self.close_hovered.set(true);
                self.pressed.set(Some(AlertPress::Pointer));
                EventResult::Handled
            }
            SystemEvent::PointerUp {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let Some(AlertPress::Pointer) = self.pressed.replace(None) else {
                    return EventResult::NotHandled;
                };
                let released_inside = self.close_rect().contains(*pos);
                self.close_hovered.set(released_inside);
                if released_inside {
                    self.dismiss_from_input();
                }
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                let hovered = self.close_rect().contains(*pos);
                if self.close_hovered.replace(hovered) != hovered {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerLeave => {
                let had_pointer_press = self.pressed.get() == Some(AlertPress::Pointer);
                if had_pointer_press {
                    self.pressed.set(None);
                }
                let changed = self.close_hovered.replace(false) | had_pointer_press;
                if changed {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.close_hovered.set(false);
                self.pressed.set(None);
                EventResult::Handled
            }
            SystemEvent::KeyDown {
                key: key @ (KeyCode::Enter | KeyCode::Space),
                ..
            } => {
                if self.pressed.get().is_none() {
                    self.pressed.set(Some(AlertPress::Key(*key)));
                }
                EventResult::Handled
            }
            SystemEvent::KeyUp {
                key: key @ (KeyCode::Enter | KeyCode::Space),
                ..
            } => {
                if self.pressed.replace(None) == Some(AlertPress::Key(*key)) {
                    self.dismiss_from_input();
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::KeyDown {
                key: KeyCode::Escape,
                ..
            } => {
                self.dismiss_from_input();
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_close
            .replace(false)
            .then(|| SemanticEvent::change(id, "closed"))
    }

    picture_policy => (&self) -> crate::draw::compositor::PicturePolicy {
        crate::draw::compositor::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let layout = self.layout(frame);
        self.last_size.set(Size::new(layout.frame.w, layout.frame.h));
        if layout.frame.w <= 0.0 || layout.frame.h <= 0.0 {
            return;
        }
        let (bg, border, fg) = match self.type_ {
            StatusLevel::Success => (ctx.tokens().color_success_bg(), ctx.tokens().color_success(), ctx.tokens().color_success()),
            StatusLevel::Info    => (ctx.tokens().color_info_bg(), ctx.tokens().color_info(), ctx.tokens().color_info()),
            StatusLevel::Warning => (ctx.tokens().color_warning_bg(), ctx.tokens().color_warning(), ctx.tokens().color_warning()),
            StatusLevel::Error   => (ctx.tokens().color_error_bg(), ctx.tokens().color_error(), ctx.tokens().color_error()),
        };
        let r = Some(Radius::uniform(ctx.tokens().border_radius()));
        ctx.push_clip(layout.frame);
        ctx.fill_rect(layout.frame, bg, r);
        if layout.accent.w > 0.0 && layout.accent.h > 0.0 {
            ctx.fill_rect(layout.accent, border, Some(Radius::uniform(1.5)));
        }

        if self.show_icon && layout.icon.w > 0.0 && layout.icon.h > 0.0 {
            let icon_name = match self.type_ {
                StatusLevel::Success => "check-circle",
                StatusLevel::Info => "info",
                StatusLevel::Warning => "alert-triangle",
                StatusLevel::Error => "x-circle",
            };
            crate::ui::widgets::icon::paint_icon_in_frame(
                ctx,
                icon_name,
                layout.icon,
                fg,
                14.0,
            );
        }
        Self::paint_elided_text(
            ctx,
            &self.message,
            layout.message,
            ctx.tokens().color_text(),
            14.0,
        );
        if !self.description.is_empty() {
            Self::paint_elided_text(
                ctx,
                &self.description,
                layout.description,
                ctx.tokens().color_text_secondary(),
                12.0,
            );
        }
        if self.closable && layout.close.w > 0.0 && layout.close.h > 0.0 {
            let close_button = Self::inset_rect(layout.close, 4.0);
            if self.pressed.get().is_some() {
                ctx.fill_rect(
                    close_button,
                    ctx.tokens().color_fill_secondary(),
                    Some(Radius::uniform(ctx.tokens().border_radius_sm())),
                );
            } else if self.close_hovered.get() {
                ctx.fill_rect(
                    close_button,
                    ctx.tokens().color_fill_tertiary(),
                    Some(Radius::uniform(ctx.tokens().border_radius_sm())),
                );
            }
            if self.focused && tree.keyboard_focus_visible() {
                ctx.stroke_rect(
                    close_button,
                    ctx.tokens().color_primary(),
                    2.0,
                    Some(Radius::uniform(ctx.tokens().border_radius_sm())),
                );
            }
            crate::ui::widgets::icon::paint_icon_in_frame(
                ctx,
                "x",
                close_button,
                ctx.tokens().color_text_secondary(),
                14.0,
            );
        }
        ctx.pop_clip();
    }
}

impl Default for Alert {
    fn default() -> Self {
        Self::new("")
    }
}

impl Alert {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            description: String::new(),
            type_: StatusLevel::Info,
            closable: false,
            show_icon: true,
            visible: true,
            focused: false,
            close_hovered: Cell::new(false),
            pressed: Cell::new(None),
            last_size: Cell::new(Size::new(300.0, 36.0)),
            layout_requested: Cell::new(false),
            pending_close: Cell::new(false),
        }
    }
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = d.into();
        self
    }
    pub fn type_(mut self, t: StatusLevel) -> Self {
        self.type_ = t;
        self
    }
    pub fn closable(mut self) -> Self {
        self.closable = true;
        self
    }
    pub fn show_icon(mut self, show: bool) -> Self {
        self.show_icon = show;
        self
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn open(&mut self) {
        if !self.visible {
            self.visible = true;
            self.layout_requested.set(true);
        }
    }

    pub fn close(&mut self) {
        if self.visible {
            self.visible = false;
            self.focused = false;
            self.close_hovered.set(false);
            self.pressed.set(None);
            self.layout_requested.set(true);
        }
    }

    fn dismiss_from_input(&mut self) {
        self.close();
        self.pending_close.set(true);
    }

    fn close_rect(&self) -> Rect {
        let size = self.last_size.get();
        let width = Self::normalize_dimension(size.w);
        let height = Self::normalize_dimension(size.h);
        let close_width = width.min(36.0);
        Rect::new((width - close_width).max(0.0), 0.0, close_width, height)
    }

    fn layout(&self, frame: Rect) -> AlertLayout {
        let frame = Self::normalize_frame(frame);
        let close_width = if self.closable {
            frame.w.min(36.0)
        } else {
            0.0
        };
        let content_right = frame.x + (frame.w - close_width - 8.0).max(0.0);
        let icon_width = if self.show_icon {
            (content_right - frame.x).clamp(0.0, 28.0)
        } else {
            0.0
        };
        let content_left = (frame.x
            + if self.show_icon {
                icon_width + 8.0
            } else {
                14.0
            })
        .min(content_right);
        let content = Rect::new(
            content_left,
            frame.y + 4.0_f32.min(frame.h * 0.25),
            (content_right - content_left).max(0.0),
            (frame.h - 8.0_f32.min(frame.h * 0.5)).max(0.0),
        );
        let (message, description) = if self.description.is_empty() {
            (
                content,
                Rect::new(content.x, content.y + content.h, content.w, 0.0),
            )
        } else {
            let message_height = (content.h * 0.52).max(0.0);
            (
                Rect::new(content.x, content.y, content.w, message_height),
                Rect::new(
                    content.x,
                    content.y + message_height,
                    content.w,
                    (content.h - message_height).max(0.0),
                ),
            )
        };
        AlertLayout {
            frame,
            accent: Rect::new(
                frame.x + 2.0_f32.min(frame.w),
                frame.y + 4.0_f32.min(frame.h * 0.5),
                3.0_f32.min((frame.w - 2.0).max(0.0)),
                (frame.h - 8.0).max(0.0),
            ),
            icon: Rect::new(frame.x + 8.0_f32.min(frame.w), frame.y, icon_width, frame.h),
            message,
            description,
            close: Rect::new(
                frame.x + frame.w - close_width,
                frame.y,
                close_width,
                frame.h,
            ),
        }
    }

    fn normalize_frame(frame: Rect) -> Rect {
        Rect::new(
            if frame.x.is_finite() { frame.x } else { 0.0 },
            if frame.y.is_finite() { frame.y } else { 0.0 },
            Self::normalize_dimension(frame.w),
            Self::normalize_dimension(frame.h),
        )
    }

    fn normalize_dimension(value: f32) -> f32 {
        if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        }
    }

    fn inset_rect(frame: Rect, inset: f32) -> Rect {
        let inset_x = inset.min(frame.w * 0.5);
        let inset_y = inset.min(frame.h * 0.5);
        Rect::new(
            frame.x + inset_x,
            frame.y + inset_y,
            (frame.w - inset_x * 2.0).max(0.0),
            (frame.h - inset_y * 2.0).max(0.0),
        )
    }

    fn paint_elided_text(
        ctx: &mut PaintContext<'_>,
        value: &str,
        frame: Rect,
        color: crate::draw::Color,
        font_size: f32,
    ) {
        let Some(value) = Self::elide_single_line(ctx, value, font_size, frame.w) else {
            return;
        };
        if frame.h <= 0.0 {
            return;
        }
        ctx.push_clip(frame);
        let text_y = ctx.visual_center_y(frame, font_size);
        ctx.draw_text(
            &value,
            crate::core::Point::new(frame.x, text_y),
            color,
            font_size,
        );
        ctx.pop_clip();
    }

    fn elide_single_line(
        ctx: &mut PaintContext<'_>,
        value: &str,
        font_size: f32,
        max_width: f32,
    ) -> Option<String> {
        if !max_width.is_finite() || max_width <= 0.0 {
            return None;
        }
        let value = value.replace(['\r', '\n'], " ");
        if Self::text_width(ctx, &value, font_size) <= max_width {
            return Some(value);
        }
        const ELLIPSIS: &str = "…";
        if Self::text_width(ctx, ELLIPSIS, font_size) > max_width {
            return None;
        }
        let mut visible = String::new();
        for ch in value.chars() {
            visible.push(ch);
            visible.push_str(ELLIPSIS);
            let fits = Self::text_width(ctx, &visible, font_size) <= max_width;
            visible.pop();
            if !fits {
                visible.pop();
                break;
            }
        }
        visible.push_str(ELLIPSIS);
        Some(visible)
    }

    fn text_width(ctx: &mut PaintContext<'_>, value: &str, font_size: f32) -> f32 {
        ctx.measure_text(value, font_size).w.max(
            crate::draw::font::text_backend::estimate_text_metrics(value, f32::INFINITY, font_size)
                .max_line_width,
        )
    }

    fn intrinsic_size(&self) -> Size {
        let h = 36.0
            + if self.description.is_empty() {
                0.0
            } else {
                18.0
            };
        Size::new(300.0, h)
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Alert {
            message: self.message.clone(),
            description: self.description.clone(),
            type_: self.type_,
            closable: self.closable,
            show_icon: self.show_icon,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.message = next.message;
        self.description = next.description;
        self.type_ = next.type_;
        self.closable = next.closable;
        self.show_icon = next.show_icon;
        self.close_hovered.set(false);
        self.pressed.set(None);
        self.pending_close.set(false);
        if !self.closable || !self.visible {
            self.focused = false;
        }
    }
}
