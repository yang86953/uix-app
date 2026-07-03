//! Menu widget — 横向/纵向导航菜单（与 Nav 侧边栏不同）。
//!
//! 支持水平或垂直布局，子菜单项、hover 高亮、active 选中态。

use uix_platform::{Point, Rect, Size};
use crate::define_widget;
use uix_graphics::{Radius};
use crate::render_context::RenderContext;
use crate::widget::{EventResult, KeyCode, WidgetEvent, WidgetTree};
use std::cell::Cell;

/// 菜单方向。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MenuMode {
    Horizontal,
    Vertical,
}

/// 单个菜单项。
#[derive(Debug, Clone)]
pub struct MenuItem {
    pub key: String,
    pub label: String,
    pub icon: String,
    pub disabled: bool,
}

// Menu — 导航菜单组件。
define_widget! {
    pub struct Menu {
        items: Vec<MenuItem>,
        active_key: String,
        mode: MenuMode,
        hovered_idx: Cell<usize>,
        focused: bool,
        item_h: f32,
        on_change: Option<Box<dyn FnMut(String) + 'static>>,
        on_click: Option<Box<dyn FnMut(usize) + 'static>>,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        match self.mode {
            MenuMode::Horizontal => {
                let w = self.items.iter().map(|i| i.label.len() as f32 * 8.0 + 32.0).sum::<f32>();
                Size::new(w.max(100.0), self.item_h)
            }
            MenuMode::Vertical => {
                Size::new(200.0, self.items.len() as f32 * self.item_h)
            }
        }
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                let idx = self.item_at(pos.x, pos.y);
                if let Some(i) = idx {
                    if !self.items[i].disabled {
                        let old_key = self.active_key.clone();
                        self.active_key = self.items[i].key.clone();
                        self.hovered_idx.set(i);
                        if let Some(ref mut cb) = self.on_click { cb(i); }
                        if self.active_key != old_key {
                            if let Some(ref mut cb) = self.on_change { cb(self.active_key.clone()); }
                        }
                        return EventResult::Handled;
                    }
                }
                EventResult::NotHandled
            }
            WidgetEvent::MouseMove { pos, .. } => {
                let idx = self.item_at(pos.x, pos.y);
                if let Some(i) = idx {
                    self.hovered_idx.set(i);
                }
                EventResult::Handled
            }
            WidgetEvent::HoverLeave => {
                self.hovered_idx.set(usize::MAX);
                EventResult::NotHandled
            }
            WidgetEvent::FocusIn => { self.focused = true; EventResult::Handled }
            WidgetEvent::FocusOut => { self.focused = false; EventResult::Handled }
            WidgetEvent::KeyDown { key, .. } => {
                let cur_idx = self.item_index_of_key(&self.active_key).unwrap_or(0);
                match key {
                    KeyCode::Right => {
                        if self.mode == MenuMode::Horizontal {
                            let mut next = cur_idx + 1;
                            while next < self.items.len() && self.items[next].disabled {
                                next += 1;
                            }
                            if next < self.items.len() {
                                self.active_key = self.items[next].key.clone();
                                if let Some(ref mut cb) = self.on_change { cb(self.active_key.clone()); }
                            }
                        } else {
                            if let Some(i) = self.first_non_disabled() {
                                self.active_key = self.items[i].key.clone();
                                if let Some(ref mut cb) = self.on_change { cb(self.active_key.clone()); }
                            }
                        }
                        EventResult::Handled
                    }
                    KeyCode::Left => {
                        if self.mode == MenuMode::Horizontal
                            && cur_idx > 0 {
                                let mut prev = cur_idx - 1;
                                loop {
                                    if !self.items[prev].disabled {
                                        self.active_key = self.items[prev].key.clone();
                                        if let Some(ref mut cb) = self.on_change { cb(self.active_key.clone()); }
                                        break;
                                    }
                                    if prev == 0 { break; }
                                    prev -= 1;
                                }
                            }
                        EventResult::Handled
                    }
                    KeyCode::Down => {
                        if self.mode == MenuMode::Vertical {
                            let mut next = cur_idx + 1;
                            while next < self.items.len() && self.items[next].disabled {
                                next += 1;
                            }
                            if next < self.items.len() {
                                self.active_key = self.items[next].key.clone();
                                if let Some(ref mut cb) = self.on_change { cb(self.active_key.clone()); }
                            }
                        } else {
                            if let Some(i) = self.first_non_disabled() {
                                self.active_key = self.items[i].key.clone();
                                if let Some(ref mut cb) = self.on_change { cb(self.active_key.clone()); }
                            }
                        }
                        EventResult::Handled
                    }
                    KeyCode::Up => {
                        if self.mode == MenuMode::Vertical
                            && cur_idx > 0 {
                                let mut prev = cur_idx - 1;
                                loop {
                                    if !self.items[prev].disabled {
                                        self.active_key = self.items[prev].key.clone();
                                        if let Some(ref mut cb) = self.on_change { cb(self.active_key.clone()); }
                                        break;
                                    }
                                    if prev == 0 { break; }
                                    prev -= 1;
                                }
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
        let primary = ctx.tokens().color_primary();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let fill = ctx.tokens().color_fill_tertiary();
        let active_key = &self.active_key;
        let hovered = self.hovered_idx.get();
        let r = Radius::uniform(ctx.tokens().border_radius_sm());

        match self.mode {
            MenuMode::Horizontal => {
                let mut cx = frame.x;
                for (i, item) in self.items.iter().enumerate() {
                    let iw = item.label.len() as f32 * 8.0 + 32.0;
                    let item_rect = Rect::new(cx, frame.y, iw, self.item_h);
                    let is_active = item.key == *active_key;
                    let is_hover = i == hovered && hovered < self.items.len();
                    let item_c = if item.disabled { text_sec } else if is_active { primary } else { text };
                    if is_active || is_hover {
                        ctx.fill_rect(item_rect, fill, Some(r));
                    }
                    if is_active {
                        ctx.fill_rect(Rect::new(cx + 8.0, frame.y + self.item_h - 2.0, iw - 16.0, 2.0), primary, None);
                    }
                    // 文字水平居中：使用 text_center 替代左对齐
                    ctx.text_center(&item.label, item_rect, item_c, 14.0);
                    cx += iw;
                }
            }
            MenuMode::Vertical => {
                for (i, item) in self.items.iter().enumerate() {
                    let item_rect = Rect::new(frame.x, frame.y + i as f32 * self.item_h, frame.w, self.item_h);
                    let is_active = item.key == *active_key;
                    let is_hover = i == hovered && hovered < self.items.len();
                    let item_c = if item.disabled { text_sec } else if is_active { primary } else { text };
                    if is_active || is_hover {
                        ctx.fill_rect(item_rect, fill, Some(r));
                    }
                    let item_y = frame.y + i as f32 * self.item_h;
                    let item_rect = Rect::new(frame.x, item_y, frame.w, self.item_h);
                    let text_y = ctx.visual_center_y(item_rect, 14.0);
                    if !item.icon.is_empty() {
                        let icon_str = crate::widgets::icon::icon_char(&item.icon);
                        let saved = *ctx.font();
                        if let Some(fh) = crate::widgets::icon::lucide_handle() {
                            ctx.set_font(fh);
                        }
                        ctx.draw_text(icon_str, Point::new(frame.x + 12.0, text_y), item_c, 14.0);
                        ctx.set_font(saved);
                    }
                    let label_x = frame.x + if item.icon.is_empty() { 16.0 } else { 36.0 };
                    ctx.draw_text(&item.label, Point::new(label_x, text_y), item_c, 14.0);
                }
            }
        }

        if self.focused {
            ctx.stroke_rect(frame, primary, 1.5, Some(r));
        }
    }
}

impl Menu {
    fn item_at(&self, px: f32, py: f32) -> Option<usize> {
        match self.mode {
            MenuMode::Horizontal => {
                if py < 0.0 || py > self.item_h {
                    return None;
                }
                let mut cx = 0.0f32;
                for (i, item) in self.items.iter().enumerate() {
                    let iw = item.label.len() as f32 * 8.0 + 32.0;
                    if px >= cx && px < cx + iw {
                        return Some(i);
                    }
                    cx += iw;
                }
                None
            }
            MenuMode::Vertical => {
                let idx = (py / self.item_h) as usize;
                if idx < self.items.len() && py >= 0.0 {
                    Some(idx)
                } else {
                    None
                }
            }
        }
    }

    fn item_index_of_key(&self, key: &str) -> Option<usize> {
        self.items.iter().position(|item| item.key == key)
    }

    fn first_non_disabled(&self) -> Option<usize> {
        self.items.iter().position(|item| !item.disabled)
    }
}

impl Default for Menu {
    fn default() -> Self {
        Self::new()
    }
}

impl Menu {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            active_key: String::new(),
            mode: MenuMode::Horizontal,
            hovered_idx: Cell::new(usize::MAX),
            focused: false,
            item_h: 32.0,
            on_change: None,
            on_click: None,
        }
    }
    pub fn items(mut self, items: Vec<MenuItem>) -> Self { self.items = items; self }
    pub fn add_item(mut self, item: MenuItem) -> Self { self.items.push(item); self }
    pub fn mode(mut self, m: MenuMode) -> Self { self.mode = m; self }
    pub fn active_key(mut self, key: &str) -> Self { self.active_key = key.to_string(); self }
    pub fn get_active_key(&self) -> &str { &self.active_key }
    pub fn set_active_key(&mut self, key: &str) { self.active_key = key.to_string(); }
    pub fn item_height(mut self, h: f32) -> Self { self.item_h = h; self }
    pub fn on_change<F: FnMut(String) + 'static>(mut self, f: F) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }
    pub fn on_click<F: FnMut(usize) + 'static>(mut self, f: F) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}
