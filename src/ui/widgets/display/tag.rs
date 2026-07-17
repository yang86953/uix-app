//! Tag widget — 彩色标签/徽标，支持关闭与勾选。

use std::cell::Cell;

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::core::widget::WidgetTree;
use crate::ui::SnapshotFields;
use crate::ui::{ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent};

/// 预设标签类型。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TagColor {
    Default,
    Success,
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TagAction {
    Closed,
    Checked,
    Unchecked,
}

component! {
    pub struct Tag {
        text: String,
        color: TagColor,
        closable: bool,
        font_size: f32,
        custom_color: Option<Color>,
        checkable: bool,
        checked: bool,
        visible: bool,
        focused: bool,
        last_size: Cell<Size>,
        layout_requested: Cell<bool>,
        pending_action: Cell<Option<TagAction>>,
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
        i32::from(self.visible && (self.closable || self.checkable))
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if !self.visible || (!self.closable && !self.checkable) {
            return EventResult::NotHandled;
        }
        match event {
            SystemEvent::PointerDown { pos, button: MouseButton::Left, .. }
                if self.closable && self.close_rect().contains(*pos) =>
            {
                self.dismiss();
                EventResult::Handled
            }
            SystemEvent::PointerDown { button: MouseButton::Left, .. } if self.checkable => {
                self.toggle_checked();
                EventResult::Handled
            }
            SystemEvent::KeyDown {
                key: KeyCode::Enter | KeyCode::Space,
                ..
            } => {
                if self.checkable {
                    self.toggle_checked();
                } else {
                    self.dismiss();
                }
                EventResult::Handled
            }
            SystemEvent::KeyDown {
                key: KeyCode::Delete | KeyCode::Escape,
                ..
            } if self.closable => {
                self.dismiss();
                EventResult::Handled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_action.replace(None).map(|action| {
            let payload = match action {
                TagAction::Closed => "closed",
                TagAction::Checked => "checked",
                TagAction::Unchecked => "unchecked",
            };
            SemanticEvent::change(id, payload)
        })
    }

    picture_policy => (&self) -> crate::draw::compositor::PicturePolicy {
        crate::draw::compositor::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.last_size.set(Size::new(frame.w, frame.h));
        let (bg, fg) = if let Some(cc) = self.custom_color {
            (cc, Color::white())
        } else {
            match self.color {
                TagColor::Default => (ctx.tokens().color_fill_tertiary(), ctx.tokens().color_text()),
                TagColor::Success => (ctx.tokens().color_success_bg(), ctx.tokens().color_success()),
                TagColor::Info    => (ctx.tokens().color_info_bg(), ctx.tokens().color_info()),
                TagColor::Warning => (ctx.tokens().color_warning_bg(), ctx.tokens().color_warning()),
                TagColor::Error   => (ctx.tokens().color_error_bg(), ctx.tokens().color_error()),
            }
        };
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
        ctx.fill_rect(frame, bg, r);
        if self.checked {
            ctx.stroke_rect(frame, fg, 1.5, r);
        }
        if self.focused {
            ctx.stroke_rect(frame, ctx.tokens().color_primary(), 2.0, r);
        }
        let checked_w = if self.checkable { 14.0 } else { 0.0 };
        if self.checkable && self.checked {
            crate::ui::widgets::icon::paint_icon_in_frame(
                ctx,
                "check",
                Rect::new(frame.x + 4.0, frame.y, 14.0, frame.h),
                fg,
                10.0,
            );
        }
        let text_x = frame.x + 8.0 + checked_w;
        let text_w = frame.w - 16.0 - checked_w - if self.closable { 20.0 } else { 0.0 };
        let content = Rect::new(text_x, frame.y, text_w, frame.h);
        let tw = ctx.measure_text(&self.text, self.font_size).w;
        let th = ctx.line_box_height(self.font_size);
        let text_rect = Rect::new(
            content.x + (content.w - tw) * 0.5,
            content.y + (content.h - th) * 0.5,
            tw.max(0.0),
            th.max(0.0),
        );
        ctx.draw_text(
            &self.text,
            crate::core::Point::new(text_rect.x, text_rect.y),
            fg,
            self.font_size,
        );
        if self.closable {
            let cx = frame.x + frame.w - 14.0;
            let close_h = ctx.line_box_height(10.0);
            let cy = frame.y + (frame.h - close_h) * 0.5;
            ctx.draw_text("✕", crate::core::Point::new(cx, cy), fg, 10.0);
        }
    }
}

impl Default for Tag {
    fn default() -> Self {
        Self::new("")
    }
}

impl Tag {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            color: TagColor::Default,
            closable: false,
            font_size: 12.0,
            custom_color: None,
            checkable: false,
            checked: false,
            visible: true,
            focused: false,
            last_size: Cell::new(Size::zero()),
            layout_requested: Cell::new(false),
            pending_action: Cell::new(None),
        }
    }
    pub fn color(mut self, c: TagColor) -> Self {
        self.color = c;
        self
    }
    pub fn custom_color(mut self, c: Color) -> Self {
        self.custom_color = Some(c);
        self
    }
    pub fn closable(mut self) -> Self {
        self.closable = true;
        self
    }
    pub fn checkable(mut self, v: bool) -> Self {
        self.checkable = v;
        if !v {
            self.checked = false;
        }
        self
    }
    /// 设置可勾选 Tag 的初始状态；用户交互后的状态在 reconcile 中保留。
    pub fn default_checked(mut self, checked: bool) -> Self {
        self.checkable = true;
        self.checked = checked;
        self
    }
    pub fn font_size(mut self, s: f32) -> Self {
        self.font_size = s;
        self
    }

    fn intrinsic_size(&self) -> Size {
        let w = self.text.len() as f32 * (self.font_size * 0.6)
            + 16.0
            + if self.checkable { 14.0 } else { 0.0 }
            + if self.closable { 20.0 } else { 0.0 };
        Size::new(w, self.font_size + 8.0)
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn is_checked(&self) -> bool {
        self.checked
    }

    pub fn set_checked(&mut self, checked: bool) {
        if self.checkable {
            self.checked = checked;
        }
    }

    pub fn close(&mut self) {
        if self.visible {
            self.visible = false;
            self.focused = false;
            self.layout_requested.set(true);
        }
    }

    pub fn open(&mut self) {
        if !self.visible {
            self.visible = true;
            self.layout_requested.set(true);
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Tag {
            text: self.text.clone(),
            color: self.color,
            closable: self.closable,
            font_size: self.font_size,
            custom_color: self.custom_color,
            checkable: self.checkable,
            checked: self.checked,
            visible: self.visible,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let was_checkable = self.checkable;
        let runtime_checked = self.checked;
        self.text = next.text;
        self.color = next.color;
        self.closable = next.closable;
        self.font_size = next.font_size;
        self.custom_color = next.custom_color;
        self.checkable = next.checkable;
        self.checked = if !self.checkable {
            false
        } else if was_checkable {
            runtime_checked
        } else {
            next.checked
        };
    }

    fn dismiss(&mut self) {
        self.close();
        self.pending_action.set(Some(TagAction::Closed));
    }

    fn toggle_checked(&mut self) {
        self.checked = !self.checked;
        self.pending_action.set(Some(if self.checked {
            TagAction::Checked
        } else {
            TagAction::Unchecked
        }));
    }

    fn close_rect(&self) -> Rect {
        let size = self.last_size.get();
        let size = if size.w > 0.0 && size.h > 0.0 {
            size
        } else {
            self.intrinsic_size()
        };
        Rect::new((size.w - 28.0).max(0.0), 0.0, size.w.min(28.0), size.h)
    }
}
