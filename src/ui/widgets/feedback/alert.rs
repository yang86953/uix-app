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
        last_size: Cell<Size>,
        layout_requested: Cell<bool>,
        pending_close: Cell<bool>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
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
                self.dismiss_from_input();
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
            SystemEvent::KeyDown {
                key: KeyCode::Enter | KeyCode::Space | KeyCode::Escape,
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

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.last_size.set(Size::new(frame.w, frame.h));
        let (bg, border, fg) = match self.type_ {
            StatusLevel::Success => (ctx.tokens().color_success_bg(), ctx.tokens().color_success(), ctx.tokens().color_success()),
            StatusLevel::Info    => (ctx.tokens().color_info_bg(), ctx.tokens().color_info(), ctx.tokens().color_info()),
            StatusLevel::Warning => (ctx.tokens().color_warning_bg(), ctx.tokens().color_warning(), ctx.tokens().color_warning()),
            StatusLevel::Error   => (ctx.tokens().color_error_bg(), ctx.tokens().color_error(), ctx.tokens().color_error()),
        };
        let r = Some(Radius::uniform(ctx.tokens().border_radius()));
        ctx.fill_rect(frame, bg, r);
        // 左边框强调线
        ctx.fill_rect(Rect::new(frame.x + 2.0, frame.y + 4.0, 3.0, frame.h - 8.0), border, Some(Radius::uniform(1.5)));

        let text_x = frame.x + if self.show_icon { 36.0 } else { 14.0 };
        let msg_y = ctx.visual_center_y(frame, 14.0);
        if self.show_icon {
            let icon_name = match self.type_ {
                StatusLevel::Success => "check-circle",
                StatusLevel::Info => "info",
                StatusLevel::Warning => "alert-triangle",
                StatusLevel::Error => "x-circle",
            };
            crate::ui::widgets::icon::paint_icon_in_frame(
                ctx,
                icon_name,
                Rect::new(frame.x + 10.0, frame.y, 20.0, frame.h),
                fg,
                14.0,
            );
        }
        ctx.draw_text(&self.message, crate::core::Point::new(text_x, msg_y), fg, 14.0);
        if !self.description.is_empty() {
            let desc_rect = Rect::new(frame.x, frame.y + frame.h * 0.5, frame.w, frame.h * 0.5);
            let desc_y = ctx.visual_center_y(desc_rect, 12.0);
            ctx.draw_text(&self.description, crate::core::Point::new(text_x, desc_y), ctx.tokens().color_text_secondary(), 12.0);
        }
        if self.closable {
            let local_close = self.close_rect();
            let close = Rect::new(
                frame.x + local_close.x,
                frame.y + local_close.y,
                local_close.w,
                local_close.h,
            );
            if self.focused {
                ctx.stroke_rect(
                    Rect::new(
                        close.x + 4.0,
                        close.y + 4.0,
                        (close.w - 8.0).max(0.0),
                        (close.h - 8.0).max(0.0),
                    ),
                    ctx.tokens().color_primary(),
                    2.0,
                    Some(Radius::uniform(ctx.tokens().border_radius_sm())),
                );
            }
            crate::ui::widgets::icon::paint_icon_in_frame(
                ctx,
                "x",
                Rect::new(close.x + 6.0, frame.y, 20.0, frame.h),
                ctx.tokens().color_text_quaternary(),
                14.0,
            );
        }
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
            self.layout_requested.set(true);
        }
    }

    fn dismiss_from_input(&mut self) {
        self.close();
        self.pending_close.set(true);
    }

    fn close_rect(&self) -> Rect {
        let size = self.last_size.get();
        Rect::new((size.w - 40.0).max(0.0), 0.0, size.w.min(40.0), size.h)
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
    }
}
