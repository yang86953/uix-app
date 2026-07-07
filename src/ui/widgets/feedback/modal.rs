use crate::ui::core::widget::WidgetCore;
use std::cell::Cell;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::native::traits::input::ControlSize;
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::SnapshotFields;
use crate::ui::{EventResult, SystemEvent, WidgetTree};

component! {
    /// Modal dialog.
    pub struct Modal {
        title: String,
        visible: bool,
        width: f32,
        height: f32,
        modal_size: ControlSize,
        closable: bool,
        mask_closable: bool,
        footer_visible: bool,
        centered: bool,
        overlay: bool,
        last_win_w: Cell<f32>,
        last_win_h: Cell<f32>,
        transition: TransitionPlayer,
        closing: bool,
        transition_dirty: bool,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    visible => (&self) -> bool { self.is_present() }

    hit_test_frame => (&self, actual_frame: Rect) -> Rect {
        if self.overlay {
            Rect::new(0.0, 0.0, self.last_win_w.get(), self.last_win_h.get())
        } else {
            actual_frame
        }
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if !self.is_present() {
            return EventResult::NotHandled;
        }

        match event {
            SystemEvent::PointerDown { pos, .. } => {
                let dlg_rect = self.dialog_rect_for_event();
                let close_rect = Rect::new(dlg_rect.x + dlg_rect.w - 48.0, dlg_rect.y, 48.0, 48.0);

                if self.closable && close_rect.contains(*pos) {
                    self.close();
                    return EventResult::Handled;
                }
                if self.mask_closable && !dlg_rect.contains(*pos) {
                    self.close();
                    return EventResult::Handled;
                }
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => {
                if *key == crate::ui::KeyCode::Escape && self.closable {
                    self.close();
                    return EventResult::Handled;
                }
                EventResult::Handled
            }
            _ => EventResult::Handled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if !self.is_present() {
            return;
        }

        let border_radius_lg = ctx.tokens().border_radius_lg();
        let bg_container = ctx.tokens().color_bg_container();
        let border_secondary = ctx.tokens().color_border_secondary();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();

        let dialog = if self.overlay {
            let win_w = ctx.canvas_2d().width() as f32;
            let win_h = ctx.canvas_2d().height() as f32;
            self.last_win_w.set(win_w);
            self.last_win_h.set(win_h);
            Rect::new(
                (win_w - self.width) * 0.5,
                (win_h - self.height) * 0.5,
                self.width,
                self.height,
            )
        } else if self.centered {
            Rect::new(
                frame.x + (frame.w - self.width) * 0.5,
                frame.y + (frame.h - self.height) * 0.5,
                self.width,
                self.height,
            )
        } else {
            Rect::new(frame.x, frame.y, self.width, self.height)
        };
        let dialog = self.apply_transition_to_dialog(dialog);
        let overlay_alpha = (128.0 * self.transition_opacity()).round().clamp(0.0, 128.0) as u8;

        ctx.fill_rect(
            Rect::new(-2000.0, -2000.0, 4000.0, 4000.0),
            Color::from_rgba(0, 0, 0, overlay_alpha),
            None,
        );

        let radius = Some(Radius::uniform(border_radius_lg));
        ctx.fill_rect(dialog, bg_container, radius);
        ctx.stroke_rect(dialog, border_secondary, 1.0, radius);

        let title_h = 56.0;
        let footer_h = if self.footer_visible { 56.0 } else { 0.0 };
        let title_rect = Rect::new(dialog.x, dialog.y, dialog.w, title_h);
        let ty = ctx.visual_center_y(title_rect, 16.0);
        ctx.draw_text(&self.title, Point::new(dialog.x + 24.0, ty), text_color, 16.0);
        ctx.fill_rect(Rect::new(dialog.x, dialog.y + title_h, dialog.w, 1.0), border_secondary, None);

        if self.closable {
            ctx.draw_text("x", Point::new(dialog.x + dialog.w - 36.0, ty), text_secondary, 16.0);
        }
        if self.footer_visible {
            ctx.fill_rect(
                Rect::new(dialog.x, dialog.y + dialog.h - footer_h, dialog.w, 1.0),
                border_secondary,
                None,
            );
        }
    }

    overlay_entry => (&self, id: crate::ui::ComponentId, _frame: Rect) -> Option<crate::ui::OverlayEntry> {
        if self.is_present() && self.overlay {
            Some(
                crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Modal)
                    .bounds(Rect::new(-2000.0, -2000.0, 4000.0, 4000.0))
                    .z_index(1000),
            )
        } else {
            None
        }
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::ComponentId], tree: &WidgetTree)
        -> Vec<(crate::ui::ComponentId, Rect)>
    {
        if !self.is_present() || children.is_empty() {
            return Vec::new();
        }

        let dialog = if self.overlay {
            let (win_w, win_h) = tree
                .root_id()
                .and_then(|rid| tree.get(rid))
                .map(|root| (root.frame().w, root.frame().h))
                .filter(|(w, h)| *w > 0.0 && *h > 0.0)
                .unwrap_or((1200.0, 760.0));
            self.last_win_w.set(win_w);
            self.last_win_h.set(win_h);
            Rect::new(
                (win_w - self.width) * 0.5,
                (win_h - self.height) * 0.5,
                self.width,
                self.height,
            )
        } else if self.centered {
            Rect::new(
                frame.x + (frame.w - self.width) * 0.5,
                frame.y + (frame.h - self.height) * 0.5,
                self.width,
                self.height,
            )
        } else {
            Rect::new(frame.x, frame.y, self.width, self.height)
        };
        let dialog = self.apply_transition_to_dialog(dialog);

        let title_h = 56.0;
        let footer_h = if self.footer_visible { 56.0 } else { 0.0 };
        let body_y = dialog.y + title_h;
        let body_h = dialog.h - title_h - footer_h;
        let padding = 24.0;
        children
            .iter()
            .map(|&cid| {
                (
                    cid,
                    Rect::new(
                        dialog.x + padding,
                        body_y + padding,
                        dialog.w - padding * 2.0,
                        body_h - padding * 2.0,
                    ),
                )
            })
            .collect()
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
            frame
        } else {
            Rect::zero()
        }
    }
}

impl Modal {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            visible: false,
            width: 520.0,
            height: 300.0,
            modal_size: ControlSize::Medium,
            closable: true,
            mask_closable: true,
            footer_visible: true,
            centered: true,
            overlay: false,
            last_win_w: Cell::new(0.0),
            last_win_h: Cell::new(0.0),
            transition: TransitionPlayer::new(presets::modal_enter()),
            closing: false,
            transition_dirty: false,
        }
    }

    pub fn visible(mut self, v: bool) -> Self {
        self.set_visible(v);
        self
    }

    pub fn show(mut self) -> Self {
        self.open();
        self
    }

    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.width = w;
        self.height = h;
        self
    }

    pub fn modal_size(mut self, s: ControlSize) -> Self {
        self.modal_size = s;
        match s {
            ControlSize::Small => {
                self.width = 400.0;
                self.height = 200.0;
            }
            ControlSize::Medium => {
                self.width = 520.0;
                self.height = 300.0;
            }
            ControlSize::Large => {
                self.width = 720.0;
                self.height = 400.0;
            }
        }
        self
    }

    pub fn closable(mut self, v: bool) -> Self {
        self.closable = v;
        self
    }

    pub fn mask_closable(mut self, v: bool) -> Self {
        self.mask_closable = v;
        self
    }

    pub fn footer_visible(mut self, v: bool) -> Self {
        self.footer_visible = v;
        self
    }

    pub fn centered(mut self, v: bool) -> Self {
        self.centered = v;
        self
    }

    pub fn overlay(mut self, v: bool) -> Self {
        self.overlay = v;
        self
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn set_visible(&mut self, v: bool) {
        if v {
            self.open();
        } else {
            self.close();
        }
    }

    pub fn open(&mut self) {
        self.visible = true;
        self.closing = false;
        self.transition = TransitionPlayer::new(presets::modal_enter());
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
        self.transition = TransitionPlayer::new(presets::modal_exit());
        self.transition_dirty = true;
    }

    pub fn confirm(&mut self) {
        self.close();
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.title = next.title;
        self.width = next.width;
        self.height = next.height;
        self.modal_size = next.modal_size;
        self.closable = next.closable;
        self.mask_closable = next.mask_closable;
        self.footer_visible = next.footer_visible;
        self.centered = next.centered;
        self.overlay = next.overlay;
    }

    fn dialog_rect_for_event(&self) -> Rect {
        if self.overlay {
            Rect::new(
                (self.last_win_w.get() - self.width) * 0.5,
                (self.last_win_h.get() - self.height) * 0.5,
                self.width,
                self.height,
            )
        } else {
            Rect::new(0.0, 0.0, self.width, self.height)
        }
    }

    pub(crate) fn is_present(&self) -> bool {
        self.visible || self.closing
    }

    fn transition_opacity(&self) -> f32 {
        self.transition.opacity_progress.clamp(0.0, 1.0)
    }

    fn apply_transition_to_dialog(&self, rect: Rect) -> Rect {
        let scale = self.transition.scale.clamp(0.0, 1.0);
        let w = rect.w * scale;
        let h = rect.h * scale;
        Rect::new(
            rect.x + (rect.w - w) * 0.5 + self.transition.offset.x,
            rect.y + (rect.h - h) * 0.5 + self.transition.offset.y,
            w,
            h,
        )
    }

    fn intrinsic_size(&self) -> Size {
        if self.overlay {
            Size::zero()
        } else if self.is_present() {
            Size::new(self.width, self.height)
        } else {
            Size::zero()
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Modal {
            title: self.title.clone(),
            width: self.width,
            height: self.height,
            modal_size: self.modal_size,
            closable: self.closable,
            mask_closable: self.mask_closable,
            footer_visible: self.footer_visible,
            centered: self.centered,
            overlay: self.overlay,
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/ui/widgets/feedback/modal.rs"]
mod tests;
