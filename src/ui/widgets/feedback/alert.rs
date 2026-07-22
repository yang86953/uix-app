//! Alert widget — 警示条，支持类型、图标、关闭。

use std::cell::Cell;
use std::rc::Rc;

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Radius;
use crate::native::traits::system::StatusLevel;
use crate::ui::core::widget::WidgetTree;
use crate::ui::SnapshotFields;
use crate::ui::{ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlertTarget {
    Action,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlertPress {
    Pointer(AlertTarget),
    Key(KeyCode, AlertTarget),
}

#[derive(Debug, Clone, Copy)]
struct AlertLayout {
    frame: Rect,
    accent: Rect,
    icon: Rect,
    message: Rect,
    description: Rect,
    action: Rect,
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
        action_label: String,
        action_callback: Option<Rc<dyn Fn()>>,
        banner: bool,
        focused: bool,
        action_hovered: Cell<bool>,
        close_hovered: Cell<bool>,
        pressed: Cell<Option<AlertPress>>,
        last_size: Cell<Size>,
        layout_requested: Cell<bool>,
        pending_close: Cell<bool>,
        pending_action: Cell<bool>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        if self.visible {
            constraints.clamp(self.intrinsic_size())
        } else {
            Size::zero()
        }
    }

    visible => (&self) -> bool { self.visible }

    tab_index => (&self) -> i32 {
        i32::from(self.visible && (self.closable || !self.action_label.is_empty()))
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if !self.visible || (!self.closable && self.action_label.is_empty()) {
            return EventResult::NotHandled;
        }
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let Some(target) = self.target_at(*pos) else {
                    return EventResult::NotHandled;
                };
                self.action_hovered.set(target == AlertTarget::Action);
                self.close_hovered.set(target == AlertTarget::Close);
                self.pressed.set(Some(AlertPress::Pointer(target)));
                EventResult::Handled
            }
            SystemEvent::PointerUp {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let Some(AlertPress::Pointer(target)) = self.pressed.replace(None) else {
                    return EventResult::NotHandled;
                };
                let released = self.target_at(*pos);
                self.action_hovered
                    .set(released == Some(AlertTarget::Action));
                self.close_hovered
                    .set(released == Some(AlertTarget::Close));
                let released_inside = released == Some(target);
                if released_inside {
                    self.activate_target(target);
                }
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                let target = self.target_at(*pos);
                let action_changed = self
                    .action_hovered
                    .replace(target == Some(AlertTarget::Action))
                    != (target == Some(AlertTarget::Action));
                let close_changed = self
                    .close_hovered
                    .replace(target == Some(AlertTarget::Close))
                    != (target == Some(AlertTarget::Close));
                if action_changed || close_changed {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerLeave => {
                let had_pointer_press = matches!(self.pressed.get(), Some(AlertPress::Pointer(_)));
                if had_pointer_press {
                    self.pressed.set(None);
                }
                let changed = self.action_hovered.replace(false)
                    | self.close_hovered.replace(false)
                    | had_pointer_press;
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
                self.action_hovered.set(false);
                self.close_hovered.set(false);
                self.pressed.set(None);
                EventResult::Handled
            }
            SystemEvent::KeyDown {
                key: key @ (KeyCode::Enter | KeyCode::Space),
                ..
            } => {
                let Some(target) = self.keyboard_target() else {
                    return EventResult::NotHandled;
                };
                if self.pressed.get().is_none() {
                    self.pressed.set(Some(AlertPress::Key(*key, target)));
                }
                EventResult::Handled
            }
            SystemEvent::KeyUp {
                key: key @ (KeyCode::Enter | KeyCode::Space),
                ..
            } => {
                let pressed = self.pressed.replace(None);
                match pressed {
                    Some(AlertPress::Key(pressed_key, target)) if pressed_key == *key => {
                        self.activate_target(target);
                        EventResult::Handled
                    }
                    _ => EventResult::NotHandled,
                }
            }
            SystemEvent::KeyDown {
                key: KeyCode::Escape,
                ..
            } if self.closable => {
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
        if self.pending_action.replace(false) {
            Some(SemanticEvent::submit(id, self.action_label.clone()))
        } else {
            self.pending_close
                .replace(false)
                .then(|| SemanticEvent::change(id, "closed"))
        }
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
        let r = (!self.banner).then(|| Radius::uniform(ctx.tokens().border_radius()));
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
            crate::ui::widgets::icon::Icon::paint_in_frame(
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
        if !self.action_label.is_empty() {
            if self.pressed.get().is_some_and(|pressed| {
                matches!(
                    pressed,
                    AlertPress::Pointer(AlertTarget::Action)
                        | AlertPress::Key(_, AlertTarget::Action)
                )
            }) {
                ctx.fill_rect(
                    Self::inset_rect(layout.action, 3.0),
                    ctx.tokens().color_fill_secondary(),
                    Some(Radius::uniform(ctx.tokens().border_radius_sm())),
                );
            } else if self.action_hovered.get() {
                ctx.fill_rect(
                    Self::inset_rect(layout.action, 3.0),
                    ctx.tokens().color_fill_tertiary(),
                    Some(Radius::uniform(ctx.tokens().border_radius_sm())),
                );
            }
            Self::paint_elided_text(ctx, &self.action_label, layout.action, fg, 13.0);
        }
        if self.closable && layout.close.w > 0.0 && layout.close.h > 0.0 {
            let close_button = Self::inset_rect(layout.close, 4.0);
            if self.pressed.get().is_some_and(|pressed| {
                matches!(
                    pressed,
                    AlertPress::Pointer(AlertTarget::Close)
                        | AlertPress::Key(_, AlertTarget::Close)
                )
            }) {
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
            crate::ui::widgets::icon::Icon::paint_in_frame(
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
            action_label: String::new(),
            action_callback: None,
            banner: false,
            focused: false,
            action_hovered: Cell::new(false),
            close_hovered: Cell::new(false),
            pressed: Cell::new(None),
            last_size: Cell::new(Size::new(300.0, 36.0)),
            layout_requested: Cell::new(false),
            pending_close: Cell::new(false),
            pending_action: Cell::new(false),
        }
    }

    pub fn success(message: impl Into<String>) -> Self {
        Self::new(message).type_(StatusLevel::Success)
    }

    pub fn info(message: impl Into<String>) -> Self {
        Self::new(message).type_(StatusLevel::Info)
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(message).type_(StatusLevel::Warning)
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::new(message).type_(StatusLevel::Error)
    }

    pub fn action<F>(mut self, label: impl Into<String>, action: F) -> Self
    where
        F: Fn() + 'static,
    {
        self.action_label = label.into();
        self.action_callback = Some(Rc::new(action));
        self
    }

    pub fn banner(mut self, banner: bool) -> Self {
        self.banner = banner;
        self
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
            self.pending_close.set(false);
            self.pending_action.set(false);
            self.layout_requested.set(true);
        }
    }

    pub fn close(&mut self) {
        if self.visible {
            self.visible = false;
            self.focused = false;
            self.action_hovered.set(false);
            self.close_hovered.set(false);
            self.pressed.set(None);
            self.layout_requested.set(true);
        }
    }

    fn dismiss_from_input(&mut self) {
        self.close();
        self.pending_close.set(true);
    }

    fn activate_target(&mut self, target: AlertTarget) {
        match target {
            AlertTarget::Action => {
                if let Some(callback) = self.action_callback.as_ref() {
                    callback();
                }
                self.pending_action.set(true);
            }
            AlertTarget::Close => self.dismiss_from_input(),
        }
    }

    fn keyboard_target(&self) -> Option<AlertTarget> {
        if self.closable {
            Some(AlertTarget::Close)
        } else if !self.action_label.is_empty() {
            Some(AlertTarget::Action)
        } else {
            None
        }
    }

    fn target_at(&self, point: crate::core::Point) -> Option<AlertTarget> {
        if self.closable && self.close_rect().contains(point) {
            Some(AlertTarget::Close)
        } else if !self.action_label.is_empty() && self.action_rect().contains(point) {
            Some(AlertTarget::Action)
        } else {
            None
        }
    }

    fn action_rect(&self) -> Rect {
        let size = self.last_size.get();
        self.layout(Rect::new(0.0, 0.0, size.w, size.h)).action
    }

    fn close_rect(&self) -> Rect {
        let size = self.last_size.get();
        self.layout(Rect::new(0.0, 0.0, size.w, size.h)).close
    }

    fn layout(&self, frame: Rect) -> AlertLayout {
        let frame = Self::normalize_frame(frame);
        let close_width = if self.closable {
            frame.w.min(36.0)
        } else {
            0.0
        };
        let close_left = frame.x + frame.w - close_width;
        let action_right = (close_left - if self.closable { 4.0 } else { 8.0 }).max(frame.x);
        let action_width = if self.action_label.is_empty() {
            0.0
        } else {
            64.0_f32.min((action_right - frame.x).max(0.0))
        };
        let action_left = action_right - action_width;
        let content_right = if action_width > 0.0 {
            (action_left - 8.0).max(frame.x)
        } else {
            (close_left - 8.0).max(frame.x)
        };
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
            action: Rect::new(
                action_left,
                frame.y,
                action_width,
                if action_width > 0.0 { frame.h } else { 0.0 },
            ),
            close: Rect::new(
                close_left,
                frame.y,
                close_width,
                if close_width > 0.0 { frame.h } else { 0.0 },
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
        self.action_label = next.action_label;
        self.action_callback = next.action_callback;
        self.banner = next.banner;
        self.action_hovered.set(false);
        self.close_hovered.set(false);
        self.pressed.set(None);
        self.pending_close.set(false);
        self.pending_action.set(false);
        if (!self.closable && self.action_label.is_empty()) || !self.visible {
            self.focused = false;
        }
    }
}
