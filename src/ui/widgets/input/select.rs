use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::{
    EventResult, KeyCode, SemanticEvent, SnapshotFields, SystemEvent, WidgetId, WidgetTree,
};
use std::cell::RefCell;

/// 选项组。
#[derive(Debug, Clone, PartialEq)]
pub struct OptGroup {
    pub label: String,
    pub options: Vec<String>,
}

impl OptGroup {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            options: Vec::new(),
        }
    }

    pub fn add(mut self, opt: &str) -> Self {
        self.options.push(opt.to_string());
        self
    }
}

component! {
    pub struct Select {
        options: Vec<String>,
        optgroups: Vec<OptGroup>,
        selected: usize,
        selected_multi: Vec<usize>,
        open: bool,
        disabled: bool,
        hovered: bool,
        focused: bool,
        transition: TransitionPlayer,
        closing: bool,
        transition_dirty: bool,
        placeholder: String,
        hovered_option: Option<usize>,
        pending_change: RefCell<Option<String>>,
        multiple: bool,
        search: bool,
    }

    tab_index => (&self) -> i32 { 1 }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if self.disabled {
            return EventResult::NotHandled;
        }

        let all_opts: Vec<&str> = self.all_options();
        match event {
            SystemEvent::PointerDown { pos, .. } => {
                if pos.y >= 0.0 && pos.y <= 32.0 {
                    if self.open {
                        self.close();
                    } else {
                        self.open();
                    }
                    self.hovered_option = None;
                    return EventResult::Handled;
                }

                if self.is_present() && pos.y > 32.0 {
                    let idx = ((pos.y - 32.0) / 28.0) as usize;
                    if idx < all_opts.len() {
                        if self.multiple {
                            if let Some(multi_idx) =
                                self.selected_multi.iter().position(|&i| i == idx)
                            {
                                self.selected_multi.remove(multi_idx);
                            } else {
                                self.selected_multi.push(idx);
                            }
                            self.pending_change.replace(Some(self.selected_multi_payload()));
                        } else {
                            if self.selected != idx {
                                self.selected = idx;
                                self.pending_change.replace(Some(idx.to_string()));
                            }
                            self.close();
                        }
                        return EventResult::Handled;
                    }
                }

                self.close();
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if self.is_present() && pos.y > 32.0 {
                    let idx = ((pos.y - 32.0) / 28.0) as usize;
                    self.hovered_option = if idx < all_opts.len() { Some(idx) } else { None };
                } else {
                    self.hovered_option = None;
                }
                self.hovered = pos.y >= 0.0 && pos.y <= 32.0;
                EventResult::Handled
            }
            SystemEvent::PointerEnter => {
                self.hovered = true;
                EventResult::Handled
            }
            SystemEvent::PointerLeave => {
                self.hovered = false;
                self.hovered_option = None;
                EventResult::Handled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.close();
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => match key {
                KeyCode::Down => {
                    if self.open {
                        let next = self.selected + 1;
                        if next < all_opts.len() {
                            self.selected = next;
                            self.pending_change.replace(Some(next.to_string()));
                        }
                    } else {
                        self.open();
                    }
                    EventResult::Handled
                }
                KeyCode::Up => {
                    if self.open && self.selected > 0 {
                        let prev = self.selected - 1;
                        self.selected = prev;
                        self.pending_change.replace(Some(prev.to_string()));
                    }
                    EventResult::Handled
                }
                KeyCode::Enter | KeyCode::Space => {
                    if self.open {
                        self.close();
                    } else {
                        self.open();
                    }
                    EventResult::Handled
                }
                KeyCode::Escape => {
                    self.close();
                    EventResult::Handled
                }
                KeyCode::Backspace => {
                    if self.multiple && !self.selected_multi.is_empty() {
                        self.selected_multi.pop();
                        self.pending_change.replace(Some(self.selected_multi_payload()));
                    }
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|value| SemanticEvent::change(id, value))
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_bg_elevated();
        let border = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let primary = ctx.tokens().color_primary();
        let fill_quaternary = ctx.tokens().color_fill_quaternary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));

        let box_rect = Rect::new(frame.x, frame.y, frame.w, 32.0);
        let box_bg = if self.disabled {
            fill_quaternary
        } else if self.hovered || self.open {
            ctx.tokens().color_bg_container()
        } else {
            bg
        };
        ctx.fill_rect(box_rect, box_bg, r);
        let border_color = if self.focused {
            primary
        } else if self.disabled {
            ctx.tokens().color_border_secondary()
        } else {
            border
        };
        ctx.stroke_rect(box_rect, border_color, if self.focused { 2.0 } else { 1.0 }, r);

        let box_rect_v = Rect::new(frame.x, frame.y, frame.w, 32.0);
        let draw_y = ctx.visual_center_y(box_rect_v, 13.0);
        if self.multiple && !self.selected_multi.is_empty() {
            let all_opts: Vec<&str> = self.all_options();
            let mut x = frame.x + 8.0;
            for &idx in &self.selected_multi {
                if idx < all_opts.len() {
                    let tag = all_opts[idx];
                    let tag_w = tag.len() as f32 * 7.0 + 16.0;
                    ctx.fill_rect(
                        Rect::new(x, frame.y + 4.0, tag_w, 24.0),
                        ctx.tokens().color_fill_tertiary(),
                        Some(Radius::uniform(4.0)),
                    );
                    ctx.draw_text(tag, Point::new(x + 4.0, draw_y), text_color, 12.0);
                    ctx.draw_text(
                        "✕",
                        Point::new(x + tag_w - 14.0, draw_y),
                        text_secondary,
                        10.0,
                    );
                    x += tag_w + 4.0;
                }
            }
        } else {
            let display_text = if self.selected < self.all_options().len() {
                self.all_options()[self.selected]
            } else {
                ""
            };
            let (disp, color) = if display_text.is_empty() && !self.placeholder.is_empty() {
                (&self.placeholder as &str, text_secondary)
            } else if display_text.is_empty() {
                ("", text_secondary)
            } else {
                (display_text, text_color)
            };
            ctx.draw_text(disp, Point::new(frame.x + 10.0, draw_y), color, 13.0);
        }

        let arrow = if self.is_present() { "▲" } else { "▼" };
        let arrow_y = ctx.visual_center_y(box_rect_v, 10.0);
        ctx.draw_text(
            arrow,
            Point::new(frame.x + frame.w - 18.0, arrow_y),
            text_secondary,
            10.0,
        );

        if !self.is_present() {
            return;
        }

        let all_opts: Vec<&str> = self.all_options();
        let flat_labels: Vec<String> = self.flat_labels();
        if all_opts.is_empty() {
            return;
        }

        let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
        let bg = fade_color(ctx.tokens().color_bg_elevated(), opacity);
        let border = fade_color(ctx.tokens().color_border(), opacity);
        let text_color = fade_color(ctx.tokens().color_text(), opacity);
        let primary = fade_color(ctx.tokens().color_primary(), opacity);
        let primary_bg = fade_color(ctx.tokens().color_primary_bg(), opacity);
        let fill_tertiary = fade_color(ctx.tokens().color_fill_tertiary(), opacity);
        let text_sec = fade_color(ctx.tokens().color_text_secondary(), opacity);
        let group_header = fade_color(ctx.tokens().color_fill_quaternary(), opacity);

        let list_y = frame.y + 32.0;
        let list_h = flat_labels.len() as f32 * 28.0;
        let list_rect = Rect::new(frame.x, list_y, frame.w, list_h);
        let shadow = ctx.tokens().box_shadow_secondary();
        ctx.draw_box_shadow(
            list_rect,
            shadow.layer_1.2,
            shadow.layer_1.0,
            shadow.layer_1.1,
            shadow.layer_1.3,
            Some(Radius::uniform(ctx.tokens().border_radius_sm())),
        );
        ctx.fill_rect(list_rect, bg, Some(Radius::uniform(ctx.tokens().border_radius_sm())));
        ctx.stroke_rect(
            list_rect,
            border,
            1.0,
            Some(Radius::uniform(ctx.tokens().border_radius_sm())),
        );

        let mut idx = 0;
        if self.optgroups.is_empty() {
            for (i, opt) in all_opts.iter().enumerate() {
                self.render_option(
                    frame,
                    list_y,
                    i,
                    opt,
                    i,
                    ctx,
                    text_color,
                    primary,
                    primary_bg,
                    fill_tertiary,
                );
            }
        } else {
            for group in &self.optgroups {
                let group_y = list_y + idx as f32 * 28.0;
                ctx.fill_rect(Rect::new(frame.x, group_y, frame.w, 28.0), group_header, None);
                let gy = ctx.visual_center_y(Rect::new(frame.x, group_y, frame.w, 28.0), 12.0);
                ctx.draw_text(&group.label, Point::new(frame.x + 10.0, gy), text_sec, 12.0);
                idx += 1;

                for opt in &group.options {
                    let opt_idx = self.option_index(&group.label, opt);
                    self.render_option(
                        frame,
                        list_y,
                        idx,
                        opt,
                        opt_idx,
                        ctx,
                        text_color,
                        primary,
                        primary_bg,
                        fill_tertiary,
                    );
                    idx += 1;
                }
            }
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        select_dirty_rect(frame, self.flat_labels().len())
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
            select_dirty_rect(frame, self.flat_labels().len())
        } else {
            Rect::zero()
        }
    }
}

impl Select {
    fn intrinsic_size(&self) -> Size {
        if self.options.is_empty() && self.optgroups.is_empty() {
            return Size::new(120.0, 32.0);
        }

        let all_opts: Vec<&str> = self.all_options();
        let w = all_opts
            .iter()
            .map(|o| o.len() as f32 * 9.0 + 32.0)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(150.0)
            .max(120.0);
        Size::new(w, 32.0)
    }

    fn render_option(
        &self,
        frame: Rect,
        list_y: f32,
        idx: usize,
        label: &str,
        opt_idx: usize,
        ctx: &mut PaintContext,
        text_color: Color,
        primary: Color,
        primary_bg: Color,
        fill_tertiary: Color,
    ) {
        let item_y = list_y + idx as f32 * 28.0;
        let item_rect = Rect::new(frame.x, item_y, frame.w, 28.0);
        let is_hovered = self.hovered_option == Some(idx);
        let is_selected = if self.multiple {
            self.selected_multi.contains(&opt_idx)
        } else {
            opt_idx == self.selected
        };

        if is_hovered || is_selected {
            let highlight = if is_hovered {
                fill_tertiary
            } else {
                primary_bg
            };
            ctx.fill_rect(item_rect, highlight, None);
        }

        let tc = if is_selected { primary } else { text_color };
        let draw_y = ctx.visual_center_y(item_rect, 13.0);
        if self.multiple {
            let check = if is_selected { "☑ " } else { "☐ " };
            ctx.draw_text(
                &format!("{}{}", check, label),
                Point::new(frame.x + 10.0, draw_y),
                tc,
                13.0,
            );
        } else {
            ctx.draw_text(label, Point::new(frame.x + 10.0, draw_y), tc, 13.0);
        }
    }

    fn all_options(&self) -> Vec<&str> {
        if !self.optgroups.is_empty() {
            self.optgroups
                .iter()
                .flat_map(|g| g.options.iter().map(|s| s.as_str()))
                .collect()
        } else {
            self.options.iter().map(|s| s.as_str()).collect()
        }
    }

    fn flat_labels(&self) -> Vec<String> {
        if !self.optgroups.is_empty() {
            self.optgroups
                .iter()
                .flat_map(|g| {
                    let mut v = vec![format!("[{}]", g.label)];
                    v.extend(g.options.clone());
                    v
                })
                .collect()
        } else {
            self.options.clone()
        }
    }

    fn option_index(&self, _group_label: &str, opt: &str) -> usize {
        self.all_options()
            .iter()
            .position(|&s| s == opt)
            .unwrap_or(0)
    }

    fn selected_multi_payload(&self) -> String {
        self.selected_multi
            .iter()
            .map(|idx| idx.to_string())
            .collect::<Vec<_>>()
            .join(",")
    }
}

impl Default for Select {
    fn default() -> Self {
        Self::new()
    }
}

impl Select {
    pub fn new() -> Self {
        Self {
            options: Vec::new(),
            optgroups: Vec::new(),
            selected: 0,
            selected_multi: Vec::new(),
            open: false,
            disabled: false,
            hovered: false,
            focused: false,
            transition: TransitionPlayer::new(presets::tooltip_enter()),
            closing: false,
            transition_dirty: false,
            placeholder: String::new(),
            hovered_option: None,
            pending_change: RefCell::new(None),
            multiple: false,
            search: false,
        }
    }

    pub fn options(mut self, opts: Vec<impl Into<String>>) -> Self {
        self.options = opts.into_iter().map(|s| s.into()).collect();
        self
    }

    pub fn optgroups(mut self, groups: Vec<OptGroup>) -> Self {
        self.optgroups = groups;
        self
    }

    pub fn selected(mut self, idx: usize) -> Self {
        self.selected = idx;
        self
    }

    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        self.placeholder = p.into();
        self
    }

    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }

    pub fn multiple(mut self, v: bool) -> Self {
        self.multiple = v;
        self
    }

    pub fn search(mut self, v: bool) -> Self {
        self.search = v;
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

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Select {
            options: self.options.clone(),
            optgroups: self.optgroups.clone(),
            disabled: self.disabled,
            placeholder: self.placeholder.clone(),
            multiple: self.multiple,
            search: self.search,
        }
    }
}

fn select_dirty_rect(frame: Rect, item_count: usize) -> Rect {
    let list_h = item_count as f32 * 28.0;
    let list = Rect::new(frame.x, frame.y + 32.0, frame.w, list_h);
    let expanded = frame.union(&list);
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
#[path = "../../../tests/ui/widgets/input/select.rs"]
mod tests;
