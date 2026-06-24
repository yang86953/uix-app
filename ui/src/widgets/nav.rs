//! Navigation components — NavItem and Navigation sidebar.
//!
//! Ant Design style sidebar navigation with indicator bar, icons, labels,
//! hover/active states, and optional subtitle.
//!
//! NavItem 使用共享的 `Rc<Cell<usize>>` 管理选中索引，
//! 点击任一 NavItem 自动更新共享状态，其他项自动取消选中。

use std::cell::Cell;
use std::rc::Rc;

use crate::define_widget;
use uix_graphics::Radius;
use uix_core::{Point, Rect, Size};
use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetTree};

/// 共享的导航选中索引 —— 多个 NavItem 持有同一份 Rc 即可联动。
pub type SharedActive = Rc<Cell<usize>>;

// NavItem — 侧边栏导航项
define_widget! {
    pub struct NavItem {
        label: String,
        icon: String,
        hovered: bool,
        fixed_width: f32,
        fixed_height: f32,
        index: usize,
        active_shared: SharedActive,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        Size::new(self.fixed_width, self.fixed_height)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::HoverEnter => { self.hovered = true; EventResult::Handled }
            WidgetEvent::HoverLeave => { self.hovered = false; EventResult::Handled }
            WidgetEvent::MouseDown { .. } => {
                self.active_shared.set(self.index);
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let active = self.index == self.active_shared.get();
        let primary = ctx.tokens().color_primary();
        let primary_bg = ctx.tokens().color_primary_bg();
        let text = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let fill_tertiary = ctx.tokens().color_fill_tertiary();
        let bg_elevated = ctx.tokens().color_bg_elevated();

        let item_frame = Rect::new(frame.x, frame.y, self.fixed_width, self.fixed_height);
        let indicator_w = 3.0;

        let (bg, text_color) = if active {
            (primary_bg, primary)
        } else if self.hovered {
            (fill_tertiary, text)
        } else {
            (bg_elevated, text_secondary)
        };

        ctx.fill_rect(item_frame, bg, None);

        if active {
            let bar = Rect::new(frame.x, frame.y, indicator_w, self.fixed_height);
            ctx.fill_rect(bar, primary, Some(Radius::uniform(1.5)));
        }

        let mut cursor_x = frame.x + indicator_w;

        if !self.icon.is_empty() {
            let icon_x = cursor_x + 10.0;
            let icon_str = crate::widgets::icon::icon_char(&self.icon);
            let saved_font = *ctx.font();
            if let Some(fh) = crate::widgets::icon::lucide_handle() {
                ctx.set_font(fh);
            }
            ctx.text_center(icon_str, Rect::new(icon_x, frame.y, 20.0, self.fixed_height), text_color, 14.0);
            ctx.set_font(saved_font);
            cursor_x += 28.0;
        } else {
            cursor_x += if active { 12.0 } else { 15.0 };
        }

        let label_area = Rect::new(cursor_x, frame.y, self.fixed_width - (cursor_x - frame.x), self.fixed_height);
        ctx.draw_text_in_frame(&self.label, label_area, text_color, 14.0);
    }
}

impl NavItem {
    pub fn new(label: &str, index: usize, active_shared: SharedActive) -> Self {
        Self {
            label: label.to_string(),
            icon: String::new(),
            hovered: false,
            fixed_width: 200.0,
            fixed_height: 36.0,
            index,
            active_shared,
        }
    }

    pub fn icon(mut self, icon: &str) -> Self {
        self.icon = icon.to_string();
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.fixed_width = w;
        self
    }

    pub fn height(mut self, h: f32) -> Self {
        self.fixed_height = h;
        self
    }
}

/// NavGroup — 导航组，管理一组 NavItem 的共享选中状态。
pub struct NavGroup {
    active: SharedActive,
    items: Vec<NavItem>,
}

impl Default for NavGroup {
    fn default() -> Self {
        Self {
            active: Rc::new(Cell::new(0)),
            items: Vec::new(),
        }
    }
}

impl NavGroup {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn item(mut self, label: &str, icon: &str) -> Self {
        let index = self.items.len();
        let mut item = NavItem::new(label, index, self.active.clone());
        if !icon.is_empty() {
            item = item.icon(icon);
        }
        self.items.push(item);
        self
    }

    pub fn active_index(self, index: usize) -> Self {
        self.active.set(index);
        self
    }

    pub fn build(self) -> Vec<NavItem> {
        self.items
    }

    pub fn active(&self) -> &SharedActive {
        &self.active
    }
}

/// Navigation — 侧边栏导航容器（全功能版）
pub struct Navigation {
    title: String,
    active: SharedActive,
    items: Vec<NavItem>,
    width: f32,
    height: f32,
    show_version: bool,
}

impl Navigation {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            active: Rc::new(Cell::new(0)),
            items: Vec::new(),
            width: 200.0,
            height: 720.0,
            show_version: true,
        }
    }

    pub fn item(mut self, label: &str, icon: &str) -> Self {
        let index = self.items.len();
        let mut item = NavItem::new(label, index, self.active.clone());
        if !icon.is_empty() {
            item = item.icon(icon);
        }
        self.items.push(item);
        self
    }

    pub fn active_index(self, index: usize) -> Self {
        self.active.set(index);
        self
    }

    pub fn active(&self) -> &SharedActive {
        &self.active
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }

    pub fn height(mut self, h: f32) -> Self {
        self.height = h;
        self
    }

    pub fn show_version(mut self, show: bool) -> Self {
        self.show_version = show;
        self
    }

    pub fn build(self, tokens: &dyn crate::theme::TokenProvider) -> crate::widget::WidgetNode {
        let loc = crate::locale::use_locale();
        use crate::widget::IntoWidgetNode;
        use crate::widgets::{Container, Divider, Label};

        let item_h = 36.0;
        let mut children: Vec<crate::widget::WidgetNode> = Vec::new();

        children.push(
            Label::new(&self.title).color(tokens.color_primary())
                .font_size(20.0)
                .size(self.width, 52.0)
                .into_node(),
        );

        children.push(
            Divider::new().color(tokens.color_border_secondary()).into_node(),
        );

        for item in self.items {
            let mut nav_item = item;
            nav_item.fixed_width = self.width;
            nav_item.fixed_height = item_h;
            children.push(nav_item.into_node());
        }

        children.push(
            Container::new().size(self.width, 0.0).flex_grow(1.0).into_node(),
        );

        if self.show_version {
            children.push(
                Label::new(loc.nav_version).color(tokens.color_text_quaternary())
                    .font_size(11.0).size(self.width, 24.0).into_node(),
            );
        }

        crate::widget::WidgetNode::new(
            Box::new(Container::new()
                .bg(tokens.color_bg_container())
                .dir(crate::FlexDirection::Column)
                .size(self.width, self.height)),
            children,
        )
    }
}
