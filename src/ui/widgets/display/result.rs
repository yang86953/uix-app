//! Result widget — 结果页，Ant Design 风格。
//!
//! 用于展示操作结果（成功/错误/警告/信息/404/403/500），
//! 包含图标、标题、副标题、额外操作区域。

use std::cell::Cell;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Color;
use crate::ui::{EventResult, KeyCode, MouseButton, SnapshotFields, SystemEvent, WidgetTree};

/// 结果类型。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResultType {
    Success,
    Error,
    Info,
    Warning,
    NotFound,    // 404
    Forbidden,   // 403
    ServerError, // 500
}

// ResultView — 结果页组件。
component! {
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
            let action = self.action_rect_local();
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
            SystemEvent::PointerUp { button: MouseButton::Left, .. } if self.pressed => {
                self.pressed = false;
                EventResult::Handled
            }
            SystemEvent::PointerLeave => {
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

    picture_policy => (&self) -> crate::draw::compositor::PicturePolicy {
        crate::draw::compositor::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let loc = crate::ui::locale::use_locale();
        let (icon, icon_color, main_title) = match self.type_ {
            ResultType::Success => ("✓", ctx.tokens().color_success(), if self.title.is_empty() { loc.result_success } else { &self.title }),
            ResultType::Error   => ("✗", ctx.tokens().color_error(), if self.title.is_empty() { loc.result_error } else { &self.title }),
            ResultType::Info    => ("ℹ", ctx.tokens().color_info(), if self.title.is_empty() { loc.result_info } else { &self.title }),
            ResultType::Warning => ("⚠", ctx.tokens().color_warning(), if self.title.is_empty() { loc.result_warning } else { &self.title }),
            ResultType::NotFound => ("404", ctx.tokens().color_text_quaternary(), if self.title.is_empty() { loc.result_404 } else { &self.title }),
            ResultType::Forbidden => ("403", ctx.tokens().color_warning(), if self.title.is_empty() { loc.result_403 } else { &self.title }),
            ResultType::ServerError => ("500", ctx.tokens().color_error(), if self.title.is_empty() { loc.result_500 } else { &self.title }),
        };
        let cx = frame.x + frame.w * 0.5;
        let cy = frame.y + frame.h * 0.4;

        // 大图标（使用 em-box 高度精确居中）
        match self.type_ {
            ResultType::NotFound | ResultType::Forbidden | ResultType::ServerError => {
                let icon_w = ctx.measure_text(icon, 48.0).w;
                ctx.draw_text(icon, Point::new(cx - icon_w * 0.5, cy - 50.0 - 48.0 * 0.5), icon_color, 48.0);
            }
            _ => {
                ctx.fill_circle(cx, cy - 30.0, 32.0, icon_color);
                let icon_w = ctx.measure_text(icon, 24.0).w;
                ctx.draw_text(icon, Point::new(cx - icon_w * 0.5, cy - 42.0 - 24.0 * 0.5), Color::white(), 24.0);
            }
        }

        // 标题（使用精确测量水平居中，em-box 高度垂直定位）
        let title_w = ctx.measure_text(main_title, 20.0).w;
        ctx.draw_text(main_title, Point::new(cx - title_w * 0.5, cy + 10.0), text, 20.0);

        // 副标题（使用精确测量水平居中）
        let sub = if self.subtitle.is_empty() {
            match self.type_ {
                ResultType::NotFound => loc.result_404_desc,
                ResultType::Forbidden => loc.result_403_desc,
                ResultType::ServerError => loc.result_500_desc,
                _ => "",
            }
        } else { &self.subtitle };
        if !sub.is_empty() {
            let sub_w = ctx.measure_text(sub, 13.0).w;
            ctx.draw_text(sub, Point::new(cx - sub_w * 0.5, cy + 40.0), text_sec, 13.0);
        }

        // 额外按钮文字
        if !self.extra_text.is_empty() {
            let btn_w = ctx.measure_text(&self.extra_text, 14.0).w + 32.0;
            let btn_x = cx - btn_w * 0.5;
            let btn_y = cy + 70.0;
            let btn_rect = Rect::new(btn_x, btn_y, btn_w, 36.0);
            self.last_action_rect.set(Rect::new(
                btn_rect.x - frame.x,
                btn_rect.y - frame.y,
                btn_rect.w,
                btn_rect.h,
            ));
            let radius = Some(crate::draw::Radius::uniform(6.0));
            let background = if self.pressed {
                ctx.tokens().color_primary_active()
            } else {
                ctx.tokens().color_primary()
            };
            ctx.fill_rect(btn_rect, background, radius);
            if self.focused {
                ctx.stroke_rect(btn_rect, ctx.tokens().color_primary_border(), 2.0, radius);
            }
            let btn_text_y = ctx.visual_center_y(btn_rect, 14.0);
            ctx.draw_text(&self.extra_text, Point::new(btn_x + 16.0, btn_text_y), Color::white(), 14.0);
        }
    }
}

impl ResultView {
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
    pub fn title(mut self, t: &str) -> Self {
        self.title = t.to_string();
        self
    }
    pub fn subtitle(mut self, s: &str) -> Self {
        self.subtitle = s.to_string();
        self
    }
    pub fn extra_text(mut self, t: impl Into<String>) -> Self {
        self.extra_text = t.into();
        self
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
        let width = (self.extra_text.chars().count() as f32 * 8.0 + 32.0).max(32.0);
        Rect::new((size.w - width) * 0.5, size.h * 0.4 + 70.0, width, 36.0)
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
