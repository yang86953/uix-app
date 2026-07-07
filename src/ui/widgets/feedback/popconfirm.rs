use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, FillRule, PathBuilder, Radius};
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::SnapshotFields;
use crate::ui::{EventResult, SystemEvent, WidgetTree};

/// Popconfirm 弹出位置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PopconfirmPlacement {
    Top,
    TopLeft,
    TopRight,
    Bottom,
    BottomLeft,
    BottomRight,
}

component! {
    pub struct Popconfirm {
        title: String,
        confirm_text: String,
        cancel_text: String,
        visible: bool,
        placement: PopconfirmPlacement,
        arrow: bool,
        icon: bool,
        transition: TransitionPlayer,
        closing: bool,
        transition_dirty: bool,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if let SystemEvent::PointerDown { pos, .. } = event {
            if pos.x >= 0.0 && pos.x <= 80.0 && pos.y >= 0.0 && pos.y <= 28.0 {
                if self.visible {
                    self.close();
                } else {
                    self.open();
                }
                return EventResult::Handled;
            }
            if self.is_present() {
                let (pw, ph) = (200.0, 110.0);
                let (px, py) = self.popup_pos(pw, ph);
                let pop_rect = Rect::new(px, py, pw, ph);
                // 点击弹窗外关闭
                if !pop_rect.contains(*pos) {
                    self.close();
                    return EventResult::Handled;
                }
                // 确认按钮
                let confirm_rect = Rect::new(px + 12.0, py + ph - 36.0, 80.0, 26.0);
                let cancel_rect = Rect::new(px + pw - 92.0, py + ph - 36.0, 80.0, 26.0);
                if confirm_rect.contains(*pos) {
                    self.close();
                    return EventResult::Handled;
                }
                if cancel_rect.contains(*pos) {
                    self.close();
                    return EventResult::Handled;
                }
            }
        }
        EventResult::NotHandled
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        popconfirm_dirty_rect(self.placement, self.arrow, frame)
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let loc = crate::ui::locale::use_locale();
        let bg = ctx.tokens().color_bg_elevated();
        let border = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let primary = ctx.tokens().color_primary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));

        let trigger_y = ctx.visual_center_y(frame, 13.0);
        ctx.draw_text(loc.delete_text, Point::new(frame.x + 20.0, trigger_y),
            ctx.tokens().color_error(), 13.0);

        if self.is_present() {
            let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
            let popup_bg = fade_color(bg, opacity);
            let popup_border = fade_color(border, opacity);
            let popup_text = fade_color(text_color, opacity);
            let popup_primary = fade_color(primary, opacity);
            let popup_warning = fade_color(ctx.tokens().color_warning(), opacity);
            let (pw, ph) = (200.0, 110.0);
            let (px, py) = self.popup_pos(pw, ph);
            let pop_rect = Rect::new(px, py, pw, ph);
            ctx.fill_rect(pop_rect, popup_bg, r);
            ctx.stroke_rect(pop_rect, popup_border, 1.0, r);

            if self.arrow {
                draw_popconfirm_arrow(ctx, frame, pop_rect, self.placement, popup_bg);
            }

            // 图标 + 标题
            let title_x = if self.icon { px + 36.0 } else { px + 12.0 };
            if self.icon {
                ctx.draw_text("⚠", Point::new(px + 12.0, py + 14.0), popup_warning, 16.0);
            }
            let loc = crate::ui::locale::use_locale();
            let title = if self.title.is_empty() { loc.popconfirm_title } else { &self.title };
            ctx.draw_text(title, Point::new(title_x, py + 16.0), popup_text, 13.0);

            // 确认按钮
            let btn_r = Some(Radius::uniform(4.0));
            ctx.fill_rect(Rect::new(px + 12.0, py + ph - 36.0, 80.0, 26.0), popup_primary, btn_r);
            let confirm = if self.confirm_text.is_empty() { loc.popconfirm_ok } else { &self.confirm_text };
            ctx.text_center(confirm, Rect::new(px + 12.0, py + ph - 36.0, 80.0, 26.0), fade_color(Color::white(), opacity), 12.0);

            ctx.stroke_rect(Rect::new(px + pw - 92.0, py + ph - 36.0, 80.0, 26.0), popup_border, 1.0, btn_r);
            let cancel = if self.cancel_text.is_empty() { loc.popconfirm_cancel } else { &self.cancel_text };
            ctx.text_center(cancel, Rect::new(px + pw - 92.0, py + ph - 36.0, 80.0, 26.0), popup_text, 12.0);
        }
    }

    overlay_entry => (&self, id: crate::ui::ComponentId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        if !self.is_present() {
            return None;
        }

        let (pw, ph) = (200.0, 110.0);
        let (px, py) = popconfirm_position(frame, self.placement, self.arrow, pw, ph);
        Some(
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Popover)
                .bounds(Rect::new(px, py, pw, ph))
                .z_index(950)
                .managed(true),
        )
    }

    update_animation => (&mut self, dt: f64) -> bool {
        if !self.is_present() || self.transition.finished {
            self.transition_dirty = false;
            return false;
        }

        self.transition.update(dt);
        self.transition_dirty = true;

        if self.closing && self.transition.finished {
            self.visible = false;
            self.closing = false;
        }

        self.is_present() && !self.transition.finished
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.transition_dirty {
            popconfirm_dirty_rect(self.placement, self.arrow, frame)
        } else {
            Rect::zero()
        }
    }
}

impl Default for Popconfirm {
    fn default() -> Self {
        Self::new()
    }
}

impl Popconfirm {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            confirm_text: "OK".to_string(),
            cancel_text: "Cancel".to_string(),
            visible: false,
            placement: PopconfirmPlacement::Top,
            arrow: true,
            icon: true,
            transition: TransitionPlayer::new(presets::tooltip_enter()),
            closing: false,
            transition_dirty: false,
        }
    }
    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = t.into();
        self
    }
    pub fn confirm_text(mut self, t: impl Into<String>) -> Self {
        self.confirm_text = t.into();
        self
    }
    pub fn cancel_text(mut self, t: impl Into<String>) -> Self {
        self.cancel_text = t.into();
        self
    }
    pub fn placement(mut self, p: PopconfirmPlacement) -> Self {
        self.placement = p;
        self
    }
    pub fn arrow(mut self, v: bool) -> Self {
        self.arrow = v;
        self
    }
    pub fn icon(mut self, v: bool) -> Self {
        self.icon = v;
        self
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn is_present(&self) -> bool {
        self.visible || self.closing
    }

    pub fn open(&mut self) {
        self.visible = true;
        self.closing = false;
        self.transition = TransitionPlayer::new(presets::tooltip_enter());
        self.transition_dirty = true;
    }

    pub fn close(&mut self) {
        if !self.is_present() {
            self.visible = false;
            self.closing = false;
            self.transition_dirty = false;
            return;
        }
        self.visible = false;
        self.closing = true;
        self.transition = TransitionPlayer::new(presets::tooltip_exit());
        self.transition_dirty = true;
    }

    fn popup_pos(&self, _pw: f32, ph: f32) -> (f32, f32) {
        popconfirm_position(
            Rect::new(0.0, 0.0, 80.0, 28.0),
            self.placement,
            self.arrow,
            200.0,
            ph,
        )
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(80.0, 28.0)
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Popconfirm {
            title: self.title.clone(),
            confirm_text: self.confirm_text.clone(),
            cancel_text: self.cancel_text.clone(),
            placement: self.placement,
            arrow: self.arrow,
            icon: self.icon,
        }
    }
}

fn popconfirm_position(
    frame: Rect,
    placement: PopconfirmPlacement,
    arrow: bool,
    pw: f32,
    ph: f32,
) -> (f32, f32) {
    let gap = if arrow { 10.0 } else { 4.0 };
    match placement {
        PopconfirmPlacement::Top | PopconfirmPlacement::TopLeft => (frame.x, frame.y - ph - gap),
        PopconfirmPlacement::TopRight => (frame.x + frame.w - pw, frame.y - ph - gap),
        PopconfirmPlacement::Bottom | PopconfirmPlacement::BottomLeft => {
            (frame.x, frame.y + frame.h + gap)
        }
        PopconfirmPlacement::BottomRight => (frame.x + frame.w - pw, frame.y + frame.h + gap),
    }
}

fn popconfirm_dirty_rect(placement: PopconfirmPlacement, arrow: bool, frame: Rect) -> Rect {
    let (pw, ph) = (200.0, 110.0);
    let (px, py) = popconfirm_position(frame, placement, arrow, pw, ph);
    frame.union(&Rect::new(px, py, pw, ph))
}

fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}

fn draw_popconfirm_arrow(
    ctx: &mut PaintContext,
    _trigger: Rect,
    popup: Rect,
    placement: PopconfirmPlacement,
    color: Color,
) {
    let arrow_sz = 6.0;
    let (x1, y1, x2, y2, x3, y3) = match placement {
        PopconfirmPlacement::Top | PopconfirmPlacement::TopLeft | PopconfirmPlacement::TopRight => {
            let cx = popup.x + popup.w / 2.0;
            (
                cx - arrow_sz,
                popup.y + popup.h,
                cx + arrow_sz,
                popup.y + popup.h,
                cx,
                popup.y + popup.h + arrow_sz,
            )
        }
        PopconfirmPlacement::Bottom
        | PopconfirmPlacement::BottomLeft
        | PopconfirmPlacement::BottomRight => {
            let cx = popup.x + popup.w / 2.0;
            (
                cx - arrow_sz,
                popup.y,
                cx + arrow_sz,
                popup.y,
                cx,
                popup.y - arrow_sz,
            )
        }
    };
    let mut pb = PathBuilder::new();
    pb.move_to(x1, y1);
    pb.line_to(x2, y2);
    pb.line_to(x3, y3);
    pb.close();
    ctx.fill_path(&pb.build(), color, FillRule::NonZero);
}

#[cfg(test)]
#[path = "../../../tests/ui/widgets/feedback/popconfirm.rs"]
mod tests;
