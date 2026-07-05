//! Tabs widget — Ant Design style tab bar with content panels.

use crate::define_widget;
use crate::widget::scene::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetTree};
use std::cell::RefCell;
use crate::render::Radius;
use crate::platform::{Point, Rect, Size};

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
        /// 每帧 render 时计算的各 tab x 坐标（供 on_event 点击定位使用）
        tab_x_positions: RefCell<Vec<f32>>,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::render::traits::GraphicsEngine>) -> Size {
        Size::new(self.fixed_width.unwrap_or(400.0), self.fixed_height.unwrap_or(200.0))
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                let tab_count = self.tabs.len();
                if tab_count == 0 { return EventResult::NotHandled; }
                // 使用 render 时存储的 tab_x_positions 做点击定位
                let click_x = pos.x;
                let positions = self.tab_x_positions.borrow();
                for (i, &tx) in positions.iter().enumerate() {
                    if i + 1 < positions.len() {
                        if click_x >= tx && click_x < positions[i + 1] {
                            self.active_index = i;
                            return EventResult::Handled;
                        }
                    } else {
                        if click_x >= tx {
                            self.active_index = i;
                            return EventResult::Handled;
                        }
                    }
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

        // 预计算各 tab 宽度和位置（使用 measure_text 确保与实际渲染一致）
        let gap = 12.0;
        let pad = 16.0;
        let mut positions = Vec::with_capacity(self.tabs.len());
        let mut cursor_x = pad;
        for tab in &self.tabs {
            positions.push(cursor_x);
            let tw = ctx.measure_text(&tab.label, 14.0).w + pad * 2.0;
            cursor_x += tw + gap;
        }
        *self.tab_x_positions.borrow_mut() = positions;

        let tab_bar_h = self.tab_height;
        let tab_bar_y = match self.position {
            TabPosition::Top => frame.y,
            TabPosition::Bottom => frame.y + frame.h - tab_bar_h,
        };

        ctx.fill_rect(Rect::new(frame.x, tab_bar_y, frame.w, tab_bar_h), bg_container, None);
        ctx.fill_rect(Rect::new(frame.x, tab_bar_y + tab_bar_h - 2.0, frame.w, 2.0), border_secondary, None);

        let mut cursor_x = frame.x + pad;
        for (i, tab) in self.tabs.iter().enumerate() {
            let is_active = i == self.active_index;
            let text_color = if is_active { primary } else { text_secondary };
            let tw = ctx.measure_text(&tab.label, 14.0).w + pad * 2.0;

            let tab_rect = Rect::new(cursor_x, tab_bar_y, tw, tab_bar_h);
            let tab_text_y = ctx.visual_center_y(tab_rect, 14.0);
            ctx.draw_text(&tab.label,
                Point::new(cursor_x + pad, tab_text_y),
                text_color, 14.0);

            if is_active {
                let indicator_w = tw * 0.6;
                let indicator_x = cursor_x + (tw - indicator_w) * 0.5;
                let indicator_y = match self.position {
                    TabPosition::Top => tab_bar_y + tab_bar_h - 2.0,
                    TabPosition::Bottom => tab_bar_y - 2.0,
                };
                ctx.fill_rect(Rect::new(indicator_x, indicator_y, indicator_w, 2.0), primary, Some(Radius::uniform(1.0)));
            }
            cursor_x += tw + gap;
        }

        let content_y = match self.position {
            TabPosition::Top => tab_bar_y + tab_bar_h,
            TabPosition::Bottom => frame.y,
        };
        ctx.fill_rect(Rect::new(frame.x, content_y, frame.w, frame.h - tab_bar_h), bg_container, None);
    }

    layout_children => (&self, frame: Rect, children: &[crate::widget::WidgetId], _tree: &WidgetTree)
        -> Vec<(crate::widget::WidgetId, Rect)>
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
    fn default() -> Self {
        Self::new()
    }
}

impl Tabs {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_index: 0,
            position: TabPosition::Top,
            tab_height: 40.0,
            fixed_width: None,
            fixed_height: None,
            tab_x_positions: RefCell::new(Vec::new()),
        }
    }

    pub fn tab(mut self, label: &str, key: &str) -> Self {
        self.tabs.push(Tab {
            label: label.to_string(),
            key: key.to_string(),
        });
        self
    }
    pub fn tabs(mut self, tabs: Vec<Tab>) -> Self {
        self.tabs = tabs;
        self
    }
    pub fn active(mut self, index: usize) -> Self {
        self.active_index = index.min(self.tabs.len().saturating_sub(1));
        self
    }
    pub fn position(mut self, pos: TabPosition) -> Self {
        self.position = pos;
        self
    }
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.fixed_width = Some(w);
        self.fixed_height = Some(h);
        self
    }
}
