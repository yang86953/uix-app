//! Tabs widget — Ant Design style tab bar with content panels.

use crate::define_widget;
use crate::graphics::{Point, Radius, Rect, Size};
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, WidgetEvent, WidgetTree};

/// A single tab definition.
#[derive(Clone)]
pub struct Tab {
    pub label: String,
    pub key: String,
}

/// Tab bar position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabPosition {
    Top,
    Bottom,
}

define_widget! {
    /// Tabs widget with a tab bar and content switching.
    pub struct Tabs {
        tabs: Vec<Tab>,
        active_index: usize,
        position: TabPosition,
        tab_height: f32,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        Size::new(self.fixed_width.unwrap_or(400.0), self.fixed_height.unwrap_or(200.0))
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                let tab_count = self.tabs.len();
                if tab_count == 0 { return EventResult::NotHandled; }
                let click_x = pos.x - 16.0;
                if click_x >= 0.0 {
                    let idx = (click_x / (100.0 + 8.0)) as usize;
                    if idx < tab_count { self.active_index = idx; return EventResult::Handled; }
                }
                EventResult::NotHandled
            }
            _ => EventResult::NotHandled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let bg_container = ctx.tokens().color_bg_container();
        let border_secondary = ctx.tokens().color_border_secondary();
        let primary = ctx.tokens().color_primary();
        let text_secondary = ctx.tokens().color_text_secondary();

        let tab_bar_h = self.tab_height;
        let tab_bar_y = match self.position {
            TabPosition::Top => frame.y,
            TabPosition::Bottom => frame.y + frame.h - tab_bar_h,
        };

        ctx.fill_rect(Rect::new(frame.x, tab_bar_y, frame.w, tab_bar_h), bg_container, None);
        ctx.fill_rect(Rect::new(frame.x, tab_bar_y + tab_bar_h - 2.0, frame.w, 2.0), border_secondary, None);

        let mut cursor_x = frame.x + 16.0;
        for (i, tab) in self.tabs.iter().enumerate() {
            let is_active = i == self.active_index;
            let text_color = if is_active { primary } else { text_secondary };

            ctx.draw_text(&tab.label, Point::new(cursor_x + 8.0, tab_bar_y + (tab_bar_h - 14.0) * 0.5), text_color, 14.0);

            if is_active {
                let indicator_x = cursor_x + (100.0 - 60.0) * 0.5;
                let indicator_y = match self.position {
                    TabPosition::Top => tab_bar_y + tab_bar_h - 2.0,
                    TabPosition::Bottom => tab_bar_y - 2.0,
                };
                ctx.fill_rect(Rect::new(indicator_x, indicator_y, 60.0, 2.0), primary, Some(Radius::uniform(1.0)));
            }
            cursor_x += 100.0 + 8.0;
        }

        let content_y = match self.position {
            TabPosition::Top => tab_bar_y + tab_bar_h,
            TabPosition::Bottom => frame.y,
        };
        ctx.fill_rect(Rect::new(frame.x, content_y, frame.w, frame.h - tab_bar_h), bg_container, None);
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::widget::WidgetId], _tree: &WidgetTree)
        -> Vec<(crate::ui::widget::WidgetId, Rect)>
    {
        if children.is_empty() { return Vec::new(); }
        let tab_bar_h = self.tab_height;
        let content_y = match self.position {
            TabPosition::Top => frame.y + tab_bar_h,
            TabPosition::Bottom => frame.y,
        };
        let content_h = frame.h - tab_bar_h;
        if self.active_index < children.len() {
            let cid = children[self.active_index];
            vec![(cid, Rect::new(frame.x + 16.0, content_y + 8.0, frame.w - 32.0, content_h - 16.0))]
        } else {
            Vec::new()
        }
    }
}

impl Default for Tabs {
    fn default() -> Self { Self::new() }
}

impl Tabs {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(), active_index: 0,
            position: TabPosition::Top, tab_height: 40.0,
            fixed_width: None, fixed_height: None,
        }
    }

    pub fn tab(mut self, label: &str, key: &str) -> Self {
        self.tabs.push(Tab { label: label.to_string(), key: key.to_string() });
        self
    }
    pub fn tabs(mut self, tabs: Vec<Tab>) -> Self { self.tabs = tabs; self }
    pub fn active(mut self, index: usize) -> Self { self.active_index = index.min(self.tabs.len().saturating_sub(1)); self }
    pub fn position(mut self, pos: TabPosition) -> Self { self.position = pos; self }
    pub fn size(mut self, w: f32, h: f32) -> Self { self.fixed_width = Some(w); self.fixed_height = Some(h); self }
}
