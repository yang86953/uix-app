//! Cascader widget - linked multi-level popup selection.

use crate::core::{Constraints, Point, Rect, Size};
use crate::define_widget;
use crate::draw::painting::PaintContext;
use crate::draw::{traits::GraphicsEngine, Color, Radius};
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::{EventResult, KeyCode, SnapshotFields, SystemEvent, WidgetTree};

#[derive(Debug, Clone, PartialEq)]
pub struct CascaderOption {
    pub label: String,
    pub value: String,
    pub children: Vec<CascaderOption>,
    pub disabled: bool,
}

impl CascaderOption {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            children: vec![],
            disabled: false,
        }
    }

    pub fn children(mut self, children: Vec<CascaderOption>) -> Self {
        self.children = children;
        self
    }

    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CascaderValue {
    pub labels: Vec<String>,
    pub values: Vec<String>,
}

define_widget! {
    pub struct Cascader {
        options: Vec<CascaderOption>,
        selected: CascaderValue,
        current_levels: Vec<Vec<CascaderOption>>,
        level_indices: Vec<usize>,
        open: bool,
        transition: TransitionPlayer,
        closing: bool,
        transition_dirty: bool,
        placeholder: String,
        focused: bool,
    }

    tab_index => (&self) -> i32 { 1 }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        self.intrinsic_size()
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown { pos, .. } => {
                self.focused = true;
                if pos.y >= 0.0 && pos.y <= 32.0 {
                    if self.open {
                        self.close();
                    } else {
                        self.open();
                    }
                    return EventResult::Handled;
                }

                if self.is_present() && pos.y > 34.0 {
                    let idx = ((pos.y - 34.0) / 32.0) as usize;
                    let level = self.current_levels.len().saturating_sub(1);
                    self.select_option(level, idx);
                    return EventResult::Handled;
                }

                self.close();
                EventResult::NotHandled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.close();
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => {
                if !self.open {
                    return EventResult::NotHandled;
                }
                match key {
                    KeyCode::Escape => {
                        self.close();
                        EventResult::Handled
                    }
                    KeyCode::Enter => {
                        self.confirm_selection();
                        EventResult::Handled
                    }
                    _ => EventResult::NotHandled,
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let loc = crate::ui::locale::use_locale();
        let primary = ctx.tokens().color_primary();
        let border_color = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let text_tertiary = ctx.tokens().color_text_tertiary();
        let bg_elevated = ctx.tokens().color_bg_elevated();
        let radius = Some(Radius::uniform(ctx.tokens().border_radius_sm()));

        ctx.fill_rect(frame, Color::white(), radius);
        ctx.stroke_rect(
            frame,
            if self.focused { primary } else { border_color },
            if self.focused { 2.0 } else { 1.0 },
            radius,
        );

        let draw_y = ctx.visual_center_y(frame, 14.0);
        if self.selected.labels.is_empty() {
            ctx.draw_text(
                &self.placeholder,
                Point::new(frame.x + 12.0, draw_y),
                text_tertiary,
                14.0,
            );
        } else {
            let display_text = self.selected.labels.join(loc.cascader_separator);
            ctx.draw_text(
                &display_text,
                Point::new(frame.x + 12.0, draw_y),
                text_color,
                14.0,
            );
        }

        let arrow_y = ctx.visual_center_y(frame, 8.0);
        ctx.draw_text(
            if self.is_present() { "▲" } else { "▼" },
            Point::new(frame.x + frame.w - 20.0, arrow_y),
            text_secondary,
            8.0,
        );

        if !self.is_present() {
            return;
        }

        let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
        let popup_w = frame.w.max(200.0);
        let popup = Rect::new(frame.x, frame.y + frame.h + 2.0, popup_w, 200.0);
        let bg_elevated = fade_color(bg_elevated, opacity);
        let border_color = fade_color(border_color, opacity);
        let text_color = fade_color(text_color, opacity);
        let text_secondary = fade_color(text_secondary, opacity);
        let text_tertiary = fade_color(text_tertiary, opacity);
        let primary_bg = fade_color(ctx.tokens().color_primary_bg(), opacity);

        ctx.fill_rect(popup, bg_elevated, radius);
        ctx.stroke_rect(popup, border_color, 1.0, radius);

        let current_opts = if let Some(level) = self.current_levels.last() {
            level
        } else {
            return;
        };

        let item_h = 32.0;
        let visible = (popup.h / item_h) as usize;
        for i in 0..visible.min(current_opts.len()) {
            let y = popup.y + i as f32 * item_h;
            let opt = &current_opts[i];
            let has_children = !opt.children.is_empty();

            if self.level_indices.last() == Some(&i) {
                ctx.fill_rect(Rect::new(popup.x, y, popup.w, item_h), primary_bg, None);
            }

            ctx.draw_text(
                &opt.label,
                Point::new(popup.x + 12.0, y + (item_h - 14.0) * 0.5),
                if opt.disabled { text_tertiary } else { text_color },
                14.0,
            );

            if has_children {
                ctx.draw_text(
                    loc.cascader_arrow,
                    Point::new(popup.x + popup.w - 16.0, y + (item_h - 14.0) * 0.5),
                    text_secondary,
                    14.0,
                );
            }
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        cascader_dirty_rect(frame)
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
            cascader_dirty_rect(frame)
        } else {
            Rect::zero()
        }
    }
}

impl Cascader {
    fn intrinsic_size(&self) -> Size {
        Size::new(120.0, 32.0)
    }

    pub fn new(options: Vec<CascaderOption>, placeholder: impl Into<String>) -> Self {
        Self {
            options: options.clone(),
            selected: CascaderValue {
                labels: vec![],
                values: vec![],
            },
            current_levels: vec![options],
            level_indices: vec![0],
            open: false,
            transition: TransitionPlayer::new(presets::tooltip_enter()),
            closing: false,
            transition_dirty: false,
            placeholder: placeholder.into(),
            focused: false,
        }
    }

    fn init_levels(&mut self) {
        self.current_levels = vec![self.options.clone()];
        self.level_indices = vec![0];
    }

    pub fn select_option(&mut self, level: usize, index: usize) {
        if level >= self.current_levels.len() {
            return;
        }
        let opt = match self.current_levels[level].get(index) {
            Some(o) => o.clone(),
            None => return,
        };
        if opt.disabled {
            return;
        }

        self.level_indices.truncate(level);
        self.level_indices.push(index);
        self.selected.labels.truncate(level);
        self.selected.values.truncate(level);
        self.selected.labels.push(opt.label.clone());
        self.selected.values.push(opt.value.clone());

        self.current_levels.truncate(level + 1);
        if !opt.children.is_empty() {
            self.current_levels.push(opt.children);
        } else {
            self.close();
        }
    }

    fn confirm_selection(&mut self) {
        if !self.selected.values.is_empty() {
            self.close();
        }
    }

    pub fn selected(&self) -> &CascaderValue {
        &self.selected
    }

    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        self.placeholder = p.into();
        self
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn is_present(&self) -> bool {
        self.open || self.closing
    }

    pub fn open(&mut self) {
        self.init_levels();
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

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Cascader {
            options: self.options.clone(),
            placeholder: self.placeholder.clone(),
        }
    }
}

fn cascader_dirty_rect(frame: Rect) -> Rect {
    let popup = Rect::new(frame.x, frame.y + frame.h + 2.0, frame.w.max(200.0), 200.0);
    frame.union(&popup)
}

fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}

#[cfg(test)]
#[path = "../../../tests/ui/widgets/input/cascader.rs"]
mod tests;
