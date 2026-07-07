//! Dropdown widget.

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::SnapshotFields;
use crate::ui::{EventResult, SystemEvent, WidgetTree};

component! {
    /// Click-triggered dropdown menu.
    pub struct Dropdown {
        label: String,
        items: Vec<String>,
        open: bool,
        transition: TransitionPlayer,
        closing: bool,
        transition_dirty: bool,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if let SystemEvent::PointerDown { pos, .. } = event {
            if pos.y >= 0.0 && pos.y <= 32.0 {
                if self.open {
                    self.close();
                } else {
                    self.open();
                }
                return EventResult::Handled;
            }
            if self.is_present() && pos.y > 32.0 {
                let idx = ((pos.y - 32.0) / 30.0) as usize;
                if idx < self.items.len() {
                    self.close();
                    return EventResult::Handled;
                }
            }
        }
        EventResult::NotHandled
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));

        let btn_rect = Rect::new(frame.x, frame.y, frame.w, 32.0);
        ctx.fill_rect(btn_rect, ctx.tokens().color_primary(), r);
        ctx.text_center(&self.label, btn_rect, crate::draw::Color::white(), 13.0);

        if !self.is_present() {
            return;
        }
        let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
        let bg = fade_color(ctx.tokens().color_bg_elevated(), opacity);
        let border = fade_color(ctx.tokens().color_border(), opacity);
        let text_color = fade_color(ctx.tokens().color_text(), opacity);

        let menu_y = frame.y + 32.0;
        let menu_h = self.items.len() as f32 * 30.0;
        let menu_rect = Rect::new(frame.x, menu_y, frame.w, menu_h);
        let shadow = ctx.tokens().box_shadow_secondary();
        ctx.draw_box_shadow(
            menu_rect,
            shadow.layer_1.2,
            shadow.layer_1.0,
            shadow.layer_1.1,
            shadow.layer_1.3,
            r,
        );
        ctx.fill_rect(menu_rect, bg, r);
        ctx.stroke_rect(menu_rect, border, 1.0, r);

        for (i, item) in self.items.iter().enumerate() {
            let item_rect = Rect::new(frame.x, menu_y + i as f32 * 30.0, frame.w, 30.0);
            ctx.text_center(item, item_rect, text_color, 13.0);
        }
    }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        if self.is_present() {
            let menu_h = self.items.len() as f32 * 30.0;
            Rect::new(frame.x, frame.y, frame.w, 32.0 + menu_h)
        } else {
            frame
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        dropdown_dirty_rect(frame, self.items.len())
    }

    update_animation => (&mut self, dt: f64) -> bool {
        if !self.is_present() || self.transition.finished {
            self.transition_dirty = false;
            return false;
        }

        self.transition.update(dt);
        self.transition_dirty = true;

        if self.closing && self.transition.finished {
            self.open = false;
            self.closing = false;
        }

        self.is_present() && !self.transition.finished
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.transition_dirty {
            dropdown_dirty_rect(frame, self.items.len())
        } else {
            Rect::zero()
        }
    }
}

impl Default for Dropdown {
    fn default() -> Self {
        Self::new("Menu")
    }
}

impl Dropdown {
    fn intrinsic_size(&self) -> Size {
        Size::new(160.0, 32.0)
    }

    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            items: Vec::new(),
            open: false,
            transition: TransitionPlayer::new(presets::tooltip_enter()),
            closing: false,
            transition_dirty: false,
        }
    }

    pub fn items(mut self, items: Vec<impl Into<String>>) -> Self {
        self.items = items.into_iter().map(|s| s.into()).collect();
        self
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn is_present(&self) -> bool {
        self.open || self.closing
    }

    pub fn open(&mut self) {
        self.open = true;
        self.closing = false;
        self.transition = TransitionPlayer::new(presets::tooltip_enter());
        self.transition_dirty = true;
    }

    pub fn close(&mut self) {
        if !self.is_present() {
            self.open = false;
            self.closing = false;
            self.transition_dirty = false;
            return;
        }
        self.open = false;
        self.closing = true;
        self.transition = TransitionPlayer::new(presets::tooltip_exit());
        self.transition_dirty = true;
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.label = next.label;
        self.items = next.items;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Dropdown {
            label: self.label.clone(),
            items: self.items.clone(),
        }
    }
}

fn dropdown_dirty_rect(frame: Rect, item_count: usize) -> Rect {
    let menu_h = item_count as f32 * 30.0;
    let menu = Rect::new(frame.x, frame.y + 32.0, frame.w, menu_h);
    let expanded = frame.union(&menu);
    let expand = 8.0;
    Rect::new(
        expanded.x - expand,
        expanded.y - expand,
        expanded.w + expand * 2.0,
        expanded.h + expand * 2.0,
    )
}

fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}

#[cfg(test)]
#[path = "../../../tests/ui/widgets/navigation/dropdown.rs"]
mod tests;
