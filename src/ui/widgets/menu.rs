//! Menu widget — 横向/纵向导航菜单（与 Nav 侧边栏不同）。
//!
//! 支持水平或垂直布局，子菜单项、hover 高亮、active 选中态。

use crate::base::{Point, Rect, Size};
use crate::define_widget;
use crate::graphics::{Color, Radius};
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, WidgetEvent, WidgetTree};
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

/// Menu — 导航菜单组件。
define_widget! {
    pub struct Menu {
        items: Vec<MenuItem>,
        active_key: String,
        mode: MenuMode,
        hovered_idx: Cell<usize>,
        item_h: f32,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
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
        if let WidgetEvent::MouseDown { pos, .. } = event {
            let idx = match self.mode {
                MenuMode::Horizontal => {
                    let mut cx = 0.0f32;
                    let mut found = None;
                    for (i, item) in self.items.iter().enumerate() {
                        let iw = item.label.len() as f32 * 8.0 + 32.0;
                        if pos.x >= cx && pos.x < cx + iw && !item.disabled {
                            found = Some(i); break;
                        }
                        cx += iw;
                    }
                    found
                }
                MenuMode::Vertical => {
                    let idx = (pos.y / self.item_h) as usize;
                    if idx < self.items.len() && !self.items[idx].disabled { Some(idx) } else { None }
                }
            };
            if let Some(i) = idx {
                self.active_key = self.items[i].key.clone();
                return EventResult::Handled;
            }
        }
        if let WidgetEvent::HoverLeave = event {
            self.hovered_idx.set(usize::MAX);
        }
        EventResult::NotHandled
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
                    let is_hover = i == hovered && i < self.items.len();
                    let item_c = if item.disabled { text_sec } else if is_active { primary } else { text };
                    if is_active || is_hover {
                        ctx.fill_rect(item_rect, fill, Some(r));
                    }
                    if is_active {
                        ctx.fill_rect(Rect::new(cx + 8.0, frame.y + self.item_h - 2.0, iw - 16.0, 2.0), primary, None);
                    }
                    ctx.draw_text(&item.label, Point::new(cx + 12.0, frame.y + 6.0), item_c, 14.0);
                    cx += iw;
                }
            }
            MenuMode::Vertical => {
                for (i, item) in self.items.iter().enumerate() {
                    let item_rect = Rect::new(frame.x, frame.y + i as f32 * self.item_h, frame.w, self.item_h);
                    let is_active = item.key == *active_key;
                    let is_hover = i == hovered && i < self.items.len();
                    let item_c = if item.disabled { text_sec } else if is_active { primary } else { text };
                    if is_active || is_hover {
                        ctx.fill_rect(item_rect, fill, Some(r));
                    }
                    if !item.icon.is_empty() {
                        ctx.draw_text(&item.icon, Point::new(frame.x + 12.0, frame.y + i as f32 * self.item_h + 6.0), item_c, 14.0);
                    }
                    let label_x = frame.x + if item.icon.is_empty() { 16.0 } else { 36.0 };
                    ctx.draw_text(&item.label, Point::new(label_x, frame.y + i as f32 * self.item_h + 6.0), item_c, 14.0);
                }
            }
        }
    }
}

impl Menu {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            active_key: String::new(),
            mode: MenuMode::Horizontal,
            hovered_idx: Cell::new(usize::MAX),
            item_h: 32.0,
        }
    }
    pub fn items(mut self, items: Vec<MenuItem>) -> Self { self.items = items; self }
    pub fn add_item(mut self, item: MenuItem) -> Self { self.items.push(item); self }
    pub fn mode(mut self, m: MenuMode) -> Self { self.mode = m; self }
    pub fn active_key(mut self, key: &str) -> Self { self.active_key = key.to_string(); self }
    pub fn get_active_key(&self) -> &str { &self.active_key }
    pub fn set_active_key(&mut self, key: &str) { self.active_key = key.to_string(); }
    pub fn item_height(mut self, h: f32) -> Self { self.item_h = h; self }
}
