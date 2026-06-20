//! Select widget — 下拉选择器，支持 hover/disabled/keyboard/placeholder。

use crate::define_widget;
use crate::base::{Point, Rect, Size};
use crate::graphics::{GraphicsEngine, Radius};
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, KeyCode, WidgetEvent, WidgetTree};

define_widget! {
    /// Select — 下拉选择框。
    pub struct Select {
        options: Vec<String>,
        selected: usize,
        open: bool,
        disabled: bool,
        hovered: bool,
        focused: bool,
        placeholder: String,
        hovered_option: Option<usize>,
        on_change: Option<Box<dyn FnMut(usize) + 'static>>,
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        if self.options.is_empty() {
            return Size::new(120.0, 32.0);
        }
        let w = self.options.iter()
            .map(|o| o.len() as f32 * 9.0 + 32.0)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(150.0).max(120.0);
        // 固定高度 32，下拉列表在 post_render 中渲染
        Size::new(w, 32.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if self.disabled { return EventResult::NotHandled; }
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                // 触发区域（选择框）
                if pos.y >= 0.0 && pos.y <= 32.0 {
                    self.open = !self.open;
                    self.hovered_option = None;
                    return EventResult::Handled;
                }
                // 下拉列表点击
                if self.open && pos.y > 32.0 {
                    let idx = ((pos.y - 32.0) / 28.0) as usize;
                    if idx < self.options.len() {
                        self.selected = idx;
                        self.open = false;
                        if let Some(ref mut cb) = self.on_change {
                            cb(idx);
                        }
                        return EventResult::Handled;
                    }
                }
                // 点击其他区域关闭
                self.open = false;
                EventResult::NotHandled
            }
            WidgetEvent::MouseMove { pos } => {
                if self.open && pos.y > 32.0 {
                    let idx = ((pos.y - 32.0) / 28.0) as usize;
                    self.hovered_option = if idx < self.options.len() { Some(idx) } else { None };
                } else {
                    self.hovered_option = None;
                }
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
                        if next < self.options.len() {
                            self.selected = next;
                            if let Some(ref mut cb) = self.on_change { cb(next); }
                        }
                    } else {
                        self.open = true;
                    }
                    EventResult::Handled
                }
                KeyCode::Up => {
                    if self.open {
                        if self.selected > 0 {
                            let prev = self.selected - 1;
                            self.selected = prev;
                            if let Some(ref mut cb) = self.on_change { cb(prev); }
                        }
                    }
                    EventResult::Handled
                }
                KeyCode::Enter | KeyCode::Space => {
                        if self.open {
                            self.open = false;
                        } else {
                            self.open = true;
                        }
                        EventResult::Handled
                    }
                    KeyCode::Escape => {
                        self.open = false;
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

        // 选择框背景
        let box_rect = Rect::new(frame.x, frame.y, frame.w, 32.0);
        let box_bg = if self.disabled { fill_quaternary } else if self.hovered || self.open { ctx.tokens().color_bg_container() } else { bg };
        ctx.fill_rect(box_rect, box_bg, r);

        // 边框：focused 时主色
        let border_color = if self.focused { primary } else if self.disabled { ctx.tokens().color_border_secondary() } else { border };
        ctx.stroke_rect(box_rect, border_color, if self.focused { 2.0 } else { 1.0 }, r);

        // 文本或 placeholder
        let display_text = if self.selected < self.options.len() {
            &self.options[self.selected]
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

        let box_rect_v = Rect::new(frame.x, frame.y, frame.w, 32.0);
        let draw_y = ctx.visual_center_y(box_rect_v, 13.0);
        ctx.draw_text(disp, Point::new(frame.x + 10.0, draw_y), color, 13.0);

        // 下拉箭头
        let arrow = if self.open { "▲" } else { "▼" };
        let arrow_y = ctx.visual_center_y(box_rect_v, 10.0);
        ctx.draw_text(arrow, Point::new(frame.x + frame.w - 18.0, arrow_y), text_secondary, 10.0);
    }

    // ── Post-render: 下拉列表（浮在布局上方，不影响定位）──
    post_render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        if !self.open || self.options.is_empty() { return; }
        let bg = ctx.tokens().color_bg_elevated();
        let border = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let primary = ctx.tokens().color_primary();
        let fill_tertiary = ctx.tokens().color_fill_tertiary();

        let list_y = frame.y + 32.0;
        let list_h = self.options.len() as f32 * 28.0;
        let list_rect = Rect::new(frame.x, list_y, frame.w, list_h);
        ctx.fill_rect(list_rect, bg, Some(Radius::uniform(ctx.tokens().border_radius_sm())));
        ctx.stroke_rect(list_rect, border, 1.0, Some(Radius::uniform(ctx.tokens().border_radius_sm())));

        for (i, opt) in self.options.iter().enumerate() {
            let item_y = list_y + i as f32 * 28.0;
            let item_rect = Rect::new(frame.x, item_y, frame.w, 28.0);
            let is_hovered = self.hovered_option == Some(i);
            let is_selected = i == self.selected;

            if is_hovered || is_selected {
                let highlight = if is_hovered { fill_tertiary } else { ctx.tokens().color_primary_bg() };
                ctx.fill_rect(item_rect, highlight, None);
            }

            let tc = if is_selected { primary } else { text_color };
            let draw_y = ctx.visual_center_y(item_rect, 13.0);
            ctx.draw_text(opt, Point::new(frame.x + 10.0, draw_y), tc, 13.0);
        }
    }

    // 始终包含下拉列表区域，确保 open 切换时无像素残留
    dirty_rect => (&self, frame: Rect) -> Rect {
        let list_h = self.options.len() as f32 * 28.0;
        let list = Rect::new(frame.x, frame.y + 32.0, frame.w, list_h);
        frame.union(&list)
    }
}

impl Default for Select { fn default() -> Self { Self::new() } }

impl Select {
    pub fn new() -> Self {
        Self {
            options: Vec::new(), selected: 0, open: false,
            disabled: false, hovered: false, focused: false,
            placeholder: String::new(), hovered_option: None,
            on_change: None,
        }
    }
    pub fn options(mut self, opts: Vec<impl Into<String>>) -> Self {
        self.options = opts.into_iter().map(|s| s.into()).collect();
        self
    }
    pub fn selected(mut self, idx: usize) -> Self { self.selected = idx; self }
    pub fn placeholder(mut self, p: impl Into<String>) -> Self { self.placeholder = p.into(); self }
    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }
    pub fn on_change<F: FnMut(usize) + 'static>(mut self, f: F) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }
}
