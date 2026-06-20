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
use crate::graphics::{Color, Radius};
use crate::base::{Rect, Size};
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, WidgetEvent, WidgetTree};

/// 共享的导航选中索引 —— 多个 NavItem 持有同一份 Rc 即可联动。
pub type SharedActive = Rc<Cell<usize>>;

// NavItem — 侧边栏导航项
//
// 显示为一行，左侧有选中指示条（active 时显示），
// 可选图标（emoji/文字图标），标签文本。
//
// 多个 NavItem 持有同一个 `SharedActive` 时，点击自动切换选中项。
define_widget! {
    pub struct NavItem {
        label: String,
        icon: String,
        hovered: bool,
        fixed_width: f32,
        fixed_height: f32,
        /// This item's index in the navigation group.
        index: usize,
        /// Shared active index — all items in the same group share this Cell.
        active_shared: SharedActive,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        Size::new(self.fixed_width, self.fixed_height)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::HoverEnter => { self.hovered = true; EventResult::Handled }
            WidgetEvent::HoverLeave => { self.hovered = false; EventResult::Handled }
            WidgetEvent::MouseDown { .. } => {
                eprintln!("[TRACE:L1] NavItem[{}] MouseDown -> active_shared.set({})", self.index, self.index);
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

        let item_frame = Rect::new(frame.x, frame.y, self.fixed_width, self.fixed_height);
        let indicator_w = 3.0;

        let (bg, text_color) = if active {
            (primary_bg, primary)
        } else if self.hovered {
            (fill_tertiary, text)
        } else {
            (Color::transparent(), text_secondary)
        };

        // Background
        if bg.a > 0 {
            ctx.fill_rect(item_frame, bg, None);
        }

        // Active indicator bar (left)
        if active {
            let bar = Rect::new(frame.x, frame.y, indicator_w, self.fixed_height);
            ctx.fill_rect(bar, primary, Some(Radius::uniform(1.5)));
        }

        let mut cursor_x = frame.x + indicator_w;

        // Icon（支持 Lucide 图标名称和 emoji 回退）
        if !self.icon.is_empty() {
            let icon_x = cursor_x + 10.0;
            let icon_str = crate::ui::widgets::icon::icon_char(&self.icon);
            let saved_font = *ctx.font();
            if let Some(fh) = crate::ui::widgets::icon::lucide_handle() {
                ctx.set_font(fh);
            }
            ctx.text_center(icon_str, Rect::new(icon_x, frame.y, 20.0, self.fixed_height), text_color, 14.0);
            ctx.set_font(saved_font);
            cursor_x += 28.0;
        } else {
            cursor_x += if active { 12.0 } else { 15.0 };
        }

        // Label
        let label_x = cursor_x;
        ctx.text_center(&self.label, Rect::new(label_x, frame.y, 0.0, self.fixed_height), text_color, 14.0);
    }
}

impl NavItem {
    /// Create a new nav item.
    ///
    /// - `label`: 显示文本
    /// - `index`: 该选项在导航组中的序号
    /// - `active_shared`: 与同组其他 NavItem 共享的选中索引
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

    /// Set an icon (emoji or text) displayed before the label.
    pub fn icon(mut self, icon: &str) -> Self {
        self.icon = icon.to_string();
        self
    }

    /// Set the item width.
    pub fn width(mut self, w: f32) -> Self {
        self.fixed_width = w;
        self
    }

    /// Set the item height.
    pub fn height(mut self, h: f32) -> Self {
        self.fixed_height = h;
        self
    }
}

/// NavGroup — 导航组，管理一组 NavItem 的共享选中状态。
///
/// 提供便捷的 builder 接口，自动分配 index 和共享 active_shared。
///
/// # Example
/// ```ignore
/// let nav = NavGroup::default()
///     .item(" Dashboard", "📊")
///     .item(" Widgets",  "🧩")
///     .item(" Settings", "⚙️");
/// // nav.items() 返回 Vec<NavItem>，所有 item 共享选中状态
/// ```
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
    /// Create a new empty navigation group.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a nav item with the given label and optional icon.
    pub fn item(mut self, label: &str, icon: &str) -> Self {
        let index = self.items.len();
        let mut item = NavItem::new(label, index, self.active.clone());
        if !icon.is_empty() {
            item = item.icon(icon);
        }
        self.items.push(item);
        self
    }

    /// Set the initially active item index.
    pub fn active_index(self, index: usize) -> Self {
        self.active.set(index);
        self
    }

    /// Consume the group and return the list of NavItems.
    pub fn build(self) -> Vec<NavItem> {
        self.items
    }

    /// Get the shared active index Cell for external read.
    pub fn active(&self) -> &SharedActive {
        &self.active
    }
}

/// Navigation — 侧边栏导航容器（全功能版）
///
/// 自动组装标题、分割线、NavItem、空白填充、版本号。
///
/// # Example
/// ```ignore
/// Navigation::new("UIX Framework", &tokens)
///     .item(" Dashboard", "📊")
///     .item(" Widgets",  "🧩")
///     .item(" Settings", "⚙️")
///     .active_index(0)
///     .build()
/// ```
pub struct Navigation {
    title: String,
    active: SharedActive,
    items: Vec<NavItem>,
    width: f32,
    height: f32,
    show_version: bool,
}

impl Navigation {
    /// Create a new Navigation with a title.
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

    /// Add a nav item with label and optional icon.
    pub fn item(mut self, label: &str, icon: &str) -> Self {
        let index = self.items.len();
        let mut item = NavItem::new(label, index, self.active.clone());
        if !icon.is_empty() {
            item = item.icon(icon);
        }
        self.items.push(item);
        self
    }

    /// Set the active item index.
    pub fn active_index(self, index: usize) -> Self {
        self.active.set(index);
        self
    }

    /// Get the shared active index Cell for external read.
    pub fn active(&self) -> &SharedActive {
        &self.active
    }

    /// Set navigation width.
    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }

    /// Set navigation height.
    pub fn height(mut self, h: f32) -> Self {
        self.height = h;
        self
    }

    /// Show or hide the version label at the bottom.
    pub fn show_version(mut self, show: bool) -> Self {
        self.show_version = show;
        self
    }

    /// Build into a WidgetNode ready for `tree!` or `WidgetTree::build`.
    pub fn build(self, tokens: &dyn crate::ui::theme::TokenProvider) -> crate::ui::widget::WidgetNode {
        use crate::ui::widget::IntoWidgetNode;
        use crate::ui::widgets::{Container, Divider, Label};

        let item_h = 36.0;
        let mut children: Vec<crate::ui::widget::WidgetNode> = Vec::new();

        // Title
        children.push(
            Label::new(&self.title).color(tokens.color_primary())
                .font_size(20.0)
                .size(self.width, 52.0)
                .into_node(),
        );

        // Divider
        children.push(
            Divider::new().color(tokens.color_border_secondary()).into_node(),
        );

        // Nav items
        for item in self.items {
            let mut nav_item = item;
            nav_item.fixed_width = self.width;
            nav_item.fixed_height = item_h;
            children.push(nav_item.into_node());
        }

        // Spacer — flex_grow(1.0) 自动填充剩余空间，适配窗口高度
        children.push(
            Container::new().size(self.width, 0.0).flex_grow(1.0).into_node(),
        );

        // Version label
        if self.show_version {
            children.push(
                Label::new("UIX v0.1.0 — Rust Native").color(tokens.color_text_quaternary())
                    .font_size(11.0).size(self.width, 24.0).into_node(),
            );
        }

        crate::ui::widget::WidgetNode::new(
            Box::new(Container::new()
                .bg(tokens.color_bg_elevated())
                .dir(crate::ui::FlexDirection::Column)
                .size(self.width, self.height)),
            children,
        )
    }
}
