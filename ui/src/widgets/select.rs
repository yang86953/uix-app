use crate::define_widget;
use uix_core::{Point, Rect, Size};
use uix_graphics::{GraphicsEngine, Radius};
use crate::render_context::RenderContext;
use crate::widget::{EventResult, KeyCode, WidgetEvent, WidgetTree};

/// 选项组。
#[derive(Debug, Clone)]
pub struct OptGroup {
    pub label: String,
    pub options: Vec<String>,
}

impl OptGroup {
    pub fn new(label: &str) -> Self { Self { label: label.to_string(), options: Vec::new() } }
    pub fn add(mut self, opt: &str) -> Self { self.options.push(opt.to_string()); self }
}

define_widget! {
    pub struct Select {
        options: Vec<String>,
        optgroups: Vec<OptGroup>,
        selected: usize,
        selected_multi: Vec<usize>,
        open: bool,
        disabled: bool,
        hovered: bool,
        focused: bool,
        placeholder: String,
        hovered_option: Option<usize>,
        on_change: Option<Box<dyn FnMut(usize) + 'static>>,
        on_change_multi: Option<Box<dyn FnMut(Vec<usize>) + 'static>>,
        multiple: bool,
        search: bool,
        search_text: String,
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        if self.options.is_empty() && self.optgroups.is_empty() {
            return Size::new(120.0, 32.0);
        }
        let all_opts: Vec<&str> = self.all_options();
        let w = all_opts.iter().map(|o| o.len() as f32 * 9.0 + 32.0)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(150.0).max(120.0);
        Size::new(w, 32.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if self.disabled { return EventResult::NotHandled; }
        let all_opts: Vec<&str> = self.all_options();
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                if pos.y >= 0.0 && pos.y <= 32.0 {
                    self.open = !self.open; self.hovered_option = None;
                    return EventResult::Handled;
                }
                if self.open && pos.y > 32.0 {
                    let idx = ((pos.y - 32.0) / 28.0) as usize;
                    if idx < all_opts.len() {
                        if self.multiple {
                            if let Some(multi_idx) = self.selected_multi.iter().position(|&i| i == idx) {
                                self.selected_multi.remove(multi_idx);
                            } else {
                                self.selected_multi.push(idx);
                            }
                            if let Some(ref mut cb) = self.on_change_multi { cb(self.selected_multi.clone()); }
                        } else {
                            self.selected = idx;
                            self.open = false;
                            if let Some(ref mut cb) = self.on_change { cb(idx); }
                        }
                        return EventResult::Handled;
                    }
                }
                self.open = false;
                EventResult::NotHandled
            }
            WidgetEvent::MouseMove { pos } => {
                if self.open && pos.y > 32.0 {
                    let idx = ((pos.y - 32.0) / 28.0) as usize;
                    self.hovered_option = if idx < all_opts.len() { Some(idx) } else { None };
                } else { self.hovered_option = None; }
                self.hovered = pos.y >= 0.0 && pos.y <= 32.0;
                EventResult::Handled
            }
            WidgetEvent::HoverEnter => { self.hovered = true; EventResult::Handled }
            WidgetEvent::HoverLeave => { self.hovered = false; self.hovered_option = None; EventResult::Handled }
            WidgetEvent::FocusIn => { self.focused = true; EventResult::Handled }
            WidgetEvent::FocusOut => { self.focused = false; self.open = false; EventResult::Handled }
            WidgetEvent::KeyDown { key, .. } => {
                match key {
                    KeyCode::Down => {
                        if self.open {
                            let next = self.selected + 1;
                            if next < all_opts.len() { self.selected = next; if let Some(ref mut cb) = self.on_change { cb(next); } }
                        } else { self.open = true; }
                        EventResult::Handled
                    }
                    KeyCode::Up => {
                        if self.open && self.selected > 0 {
                            let prev = self.selected - 1;
                            self.selected = prev;
                            if let Some(ref mut cb) = self.on_change { cb(prev); }
                        }
                        EventResult::Handled
                    }
                    KeyCode::Enter | KeyCode::Space => { self.open = !self.open; EventResult::Handled }
                    KeyCode::Escape => { self.open = false; EventResult::Handled }
                    KeyCode::Backspace => {
                        if self.multiple && !self.selected_multi.is_empty() {
                            self.selected_multi.pop();
                            if let Some(ref mut cb) = self.on_change_multi { cb(self.selected_multi.clone()); }
                        }
                        EventResult::Handled
                    }
                    _ => EventResult::NotHandled,
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_bg_elevated();
        let border = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let primary = ctx.tokens().color_primary();
        let fill_quaternary = ctx.tokens().color_fill_quaternary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));

        let box_rect = Rect::new(frame.x, frame.y, frame.w, 32.0);
        let box_bg = if self.disabled { fill_quaternary } else if self.hovered || self.open { ctx.tokens().color_bg_container() } else { bg };
        ctx.fill_rect(box_rect, box_bg, r);
        let border_color = if self.focused { primary } else if self.disabled { ctx.tokens().color_border_secondary() } else { border };
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
                    ctx.fill_rect(Rect::new(x, frame.y + 4.0, tag_w, 24.0), ctx.tokens().color_fill_tertiary(), Some(Radius::uniform(4.0)));
                    ctx.draw_text(tag, Point::new(x + 4.0, draw_y), text_color, 12.0);
                    ctx.draw_text("✕", Point::new(x + tag_w - 14.0, draw_y), text_secondary, 10.0);
                    x += tag_w + 4.0;
                }
            }
        } else {
            let display_text = if self.selected < self.all_options().len() { self.all_options()[self.selected] } else { "" };
            let (disp, color) = if display_text.is_empty() && !self.placeholder.is_empty() { (&self.placeholder as &str, text_secondary) }
            else if display_text.is_empty() { ("", text_secondary) } else { (display_text, text_color) };
            ctx.draw_text(disp, Point::new(frame.x + 10.0, draw_y), color, 13.0);
        }

        let arrow = if self.open { "▲" } else { "▼" };
        let arrow_y = ctx.visual_center_y(box_rect_v, 10.0);
        ctx.draw_text(arrow, Point::new(frame.x + frame.w - 18.0, arrow_y), text_secondary, 10.0);
    }

    post_render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        if !self.open { return; }
        let all_opts: Vec<&str> = self.all_options();
        let flat_labels: Vec<String> = self.flat_labels();
        if all_opts.is_empty() { return; }

        let bg = ctx.tokens().color_bg_elevated();
        let border = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let primary = ctx.tokens().color_primary();
        let fill_tertiary = ctx.tokens().color_fill_tertiary();
        let text_sec = ctx.tokens().color_text_secondary();
        let group_header = ctx.tokens().color_fill_quaternary();

        let list_y = frame.y + 32.0;
        let list_h = flat_labels.len() as f32 * 28.0;
        let list_rect = Rect::new(frame.x, list_y, frame.w, list_h);
        ctx.fill_rect(list_rect, bg, Some(Radius::uniform(ctx.tokens().border_radius_sm())));
        ctx.stroke_rect(list_rect, border, 1.0, Some(Radius::uniform(ctx.tokens().border_radius_sm())));

        let mut idx = 0;
        if self.optgroups.is_empty() {
            for (i, opt) in all_opts.iter().enumerate() {
                self.render_option(frame, list_y, i, opt, i, ctx, text_color, primary, fill_tertiary);
            }
        } else {
            for group in &self.optgroups {
                // 组标题
                let group_y = list_y + idx as f32 * 28.0;
                ctx.fill_rect(Rect::new(frame.x, group_y, frame.w, 28.0), group_header, None);
                let gy = ctx.visual_center_y(Rect::new(frame.x, group_y, frame.w, 28.0), 12.0);
                ctx.draw_text(&group.label, Point::new(frame.x + 10.0, gy), text_sec, 12.0);
                idx += 1;

                for opt in &group.options {
                    let opt_idx = self.option_index(&group.label, opt);
                    self.render_option(frame, list_y, idx, opt, opt_idx, ctx, text_color, primary, fill_tertiary);
                    idx += 1;
                }
            }
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        let flat: Vec<String> = self.flat_labels();
        let list_h = flat.len() as f32 * 28.0;
        let list = Rect::new(frame.x, frame.y + 32.0, frame.w, list_h);
        frame.union(&list)
    }
}

impl Select {
    fn render_option(&self, frame: Rect, list_y: f32, idx: usize, label: &str, opt_idx: usize, ctx: &mut RenderContext, text_color: Color, primary: Color, fill_tertiary: Color) {
        let item_y = list_y + idx as f32 * 28.0;
        let item_rect = Rect::new(frame.x, item_y, frame.w, 28.0);
        let is_hovered = self.hovered_option == Some(idx);
        let is_selected = if self.multiple { self.selected_multi.contains(&opt_idx) } else { opt_idx == self.selected };
        if is_hovered || is_selected {
            let highlight = if is_hovered { fill_tertiary } else { ctx.tokens().color_primary_bg() };
            ctx.fill_rect(item_rect, highlight, None);
        }
        let tc = if is_selected { primary } else { text_color };
        let draw_y = ctx.visual_center_y(item_rect, 13.0);
        if self.multiple {
            let check = if is_selected { "☑ " } else { "☐ " };
            ctx.draw_text(&format!("{}{}", check, label), Point::new(frame.x + 10.0, draw_y), tc, 13.0);
        } else {
            ctx.draw_text(label, Point::new(frame.x + 10.0, draw_y), tc, 13.0);
        }
    }

    fn all_options(&self) -> Vec<&str> {
        if !self.optgroups.is_empty() {
            self.optgroups.iter().flat_map(|g| g.options.iter().map(|s| s.as_str())).collect()
        } else {
            self.options.iter().map(|s| s.as_str()).collect()
        }
    }

    fn flat_labels(&self) -> Vec<String> {
        if !self.optgroups.is_empty() {
            self.optgroups.iter().flat_map(|g| {
                let mut v = vec![format!("[{}]", g.label)];
                v.extend(g.options.clone());
                v
            }).collect()
        } else {
            self.options.clone()
        }
    }

    fn option_index(&self, _group_label: &str, opt: &str) -> usize {
        self.all_options().iter().position(|&s| s == opt).unwrap_or(0)
    }
}

use uix_graphics::Color;

impl Default for Select { fn default() -> Self { Self::new() } }

impl Select {
    pub fn new() -> Self {
        Self {
            options: Vec::new(), optgroups: Vec::new(),
            selected: 0, selected_multi: Vec::new(),
            open: false, disabled: false, hovered: false, focused: false,
            placeholder: String::new(), hovered_option: None,
            on_change: None, on_change_multi: None,
            multiple: false, search: false, search_text: String::new(),
        }
    }
    pub fn options(mut self, opts: Vec<impl Into<String>>) -> Self {
        self.options = opts.into_iter().map(|s| s.into()).collect(); self
    }
    pub fn optgroups(mut self, groups: Vec<OptGroup>) -> Self { self.optgroups = groups; self }
    pub fn selected(mut self, idx: usize) -> Self { self.selected = idx; self }
    pub fn placeholder(mut self, p: impl Into<String>) -> Self { self.placeholder = p.into(); self }
    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }
    pub fn multiple(mut self, v: bool) -> Self { self.multiple = v; self }
    pub fn search(mut self, v: bool) -> Self { self.search = v; self }
    pub fn on_change<F: FnMut(usize) + 'static>(mut self, f: F) -> Self { self.on_change = Some(Box::new(f)); self }
    pub fn on_change_multi<F: FnMut(Vec<usize>) + 'static>(mut self, f: F) -> Self { self.on_change_multi = Some(Box::new(f)); self }
}
