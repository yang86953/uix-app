//! Navigation components — NavItem and Navigation sidebar.
//!
//! Ant Design style sidebar navigation with indicator bar, icons, labels,
//! hover/active states, and optional subtitle.
//!
//! NavItem 使用共享的 `Rc<Cell<usize>>` 管理选中索引，
//! 点击任一 NavItem 自动更新共享状态，其他项自动取消选中。

use std::cell::{Cell, RefCell};
use std::fmt::Display;
use std::rc::Rc;

use crate::component;
use crate::core::{ComponentId, Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Radius;
use crate::ui::state::State;
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent, WidgetTree,
};

/// 共享的导航选中索引 —— 多个 NavItem 持有同一份 Rc 即可联动。
pub type SharedActive = Rc<Cell<usize>>;

/// 导航项绘制区域：上下各扩 0.5px，避免局部重绘与相邻项出现 1px 接缝。
fn nav_item_paint_rect(frame: Rect, min_w: f32, min_h: f32) -> Rect {
    Rect::new(
        frame.x,
        frame.y - 0.5,
        frame.w.max(min_w),
        frame.h.max(min_h) + 1.0,
    )
}

/// 先铺不透明底色，再叠 hover/active 色，避免局部清除后以透底产生白线。
fn paint_nav_item_bg(
    ctx: &mut PaintContext,
    rect: Rect,
    base: crate::draw::Color,
    overlay: Option<crate::draw::Color>,
) {
    ctx.fill_rect(rect, base, None);
    if let Some(color) = overlay {
        if color.a > 0 {
            ctx.fill_rect(rect, color, None);
        }
    }
}

// NavItem — 侧边栏导航项
component! {
    pub struct NavItem {
        label: String,
        key: String,
        icon: String,
        hovered: bool,
        fixed_width: f32,
        fixed_height: f32,
        index: usize,
        active_shared: SharedActive,
        compact: bool,
        focused: bool,
        value_binding: Option<Rc<dyn Fn()>>,
        pending_change: RefCell<Option<String>>,
    }

    tab_index => (&self) -> i32 { 1 }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerEnter => { self.hovered = true; EventResult::Handled }
            SystemEvent::PointerLeave => { self.hovered = false; EventResult::Handled }
            SystemEvent::PointerDown {
                button: MouseButton::Left,
                ..
            } => {
                self.activate();
                EventResult::Handled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                EventResult::Handled
            }
            SystemEvent::KeyDown {
                key: KeyCode::Enter | KeyCode::Space,
                ..
            } => {
                self.activate();
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|key| SemanticEvent::change(id, key))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let active = self.index == self.active_shared.get();
        let primary = ctx.tokens().color_primary();
        let primary_bg = ctx.tokens().color_primary_bg();
        let text = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let fill_tertiary = ctx.tokens().color_fill_tertiary();
        let bg_container = ctx.tokens().color_bg_container();
        let bg_elevated = ctx.tokens().color_bg_elevated();

        let item_frame = nav_item_paint_rect(
            frame,
            self.fixed_width,
            self.fixed_height,
        );

        // —— Compact 模式：纯图标按钮，无文字标签，无指示条 ——
        if self.compact {
            let (base, overlay, text_color) = if active {
                (bg_elevated, Some(primary_bg), primary)
            } else if self.hovered {
                (bg_elevated, Some(fill_tertiary), text)
            } else {
                (bg_elevated, None, text_secondary)
            };
            paint_nav_item_bg(ctx, item_frame, base, overlay);

            if self.focused && tree.keyboard_focus_visible() {
                ctx.stroke_rect(
                    item_frame,
                    primary,
                    1.5,
                    Some(Radius::uniform(ctx.tokens().border_radius_sm())),
                );
            }

            if !self.icon.is_empty() {
                crate::ui::widgets::icon::paint_icon_in_frame(
                    ctx,
                    &self.icon,
                    item_frame,
                    text_color,
                    18.0,
                );
            } else if !self.label.is_empty() {
                let display = &self.label[..self
                    .label
                    .char_indices()
                    .nth(1)
                    .map(|(i, _)| i)
                    .unwrap_or(self.label.len())];
                let fs = 18.0;
                let tw = ctx.measure_text(display, fs).w;
                let th = ctx.line_box_height(fs);
                ctx.draw_text(
                    display,
                    crate::core::Point::new(
                        item_frame.x + (item_frame.w - tw) * 0.5,
                        item_frame.y + (item_frame.h - th) * 0.5,
                    ),
                    text_color,
                    fs,
                );
            }
            return;
        }

        // —— 标准模式 ——
        let indicator_w = 3.0;

        let (overlay, icon_color, label_color) = if active {
            (Some(primary_bg), primary, text)
        } else if self.hovered {
            (Some(fill_tertiary), text, text)
        } else {
            (None, text_secondary, text_secondary)
        };

        paint_nav_item_bg(ctx, item_frame, bg_container, overlay);

        if active {
            let bar = Rect::new(frame.x, frame.y, indicator_w, frame.h.max(self.fixed_height));
            ctx.fill_rect(bar, primary, Some(Radius::uniform(1.5)));
        }

        let mut cursor_x = frame.x + indicator_w;

        let row_h = frame.h.max(self.fixed_height);
        if !self.icon.is_empty() {
            let icon_slot = Rect::new(cursor_x + 10.0, frame.y, 20.0, row_h);
            // 先用 UI 字体光学中心画图标，再画标签（见 paint_icon_in_frame）
            crate::ui::widgets::icon::paint_icon_in_frame(
                ctx,
                &self.icon,
                icon_slot,
                icon_color,
                14.0,
            );
            cursor_x += 28.0;
        } else {
            cursor_x += if active { 12.0 } else { 15.0 };
        }

        let label_w = (frame.x + frame.w - cursor_x).max(0.0);
        let label_area = Rect::new(cursor_x, frame.y, label_w, row_h);
        ctx.draw_text_in_frame(&self.label, label_area, label_color, 14.0);
        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(
                item_frame,
                primary,
                1.5,
                Some(Radius::uniform(ctx.tokens().border_radius_sm())),
            );
        }
    }
}

impl NavItem {
    fn intrinsic_size(&self) -> Size {
        Size::new(self.fixed_width, self.fixed_height)
    }

    pub fn new(label: &str, index: usize, active_shared: SharedActive) -> Self {
        Self {
            label: label.to_string(),
            key: index.to_string(),
            icon: String::new(),
            hovered: false,
            fixed_width: 200.0,
            fixed_height: 36.0,
            index,
            active_shared,
            compact: false,
            focused: false,
            value_binding: None,
            pending_change: RefCell::new(None),
        }
    }

    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = key.into();
        self
    }

    pub fn icon(mut self, icon: &str) -> Self {
        self.icon = icon.to_string();
        self
    }

    pub fn label_text(&self) -> &str {
        &self.label
    }

    pub fn nav_key(&self) -> &str {
        &self.key
    }

    pub fn nav_index(&self) -> usize {
        self.index
    }

    pub fn is_active(&self) -> bool {
        self.active_shared.get() == self.index
    }

    pub fn width(mut self, w: f32) -> Self {
        self.fixed_width = w;
        self
    }

    pub fn height(mut self, h: f32) -> Self {
        self.fixed_height = h;
        self
    }

    /// 紧凑模式：纯图标按钮，不显示文字标签和指示条，高度自动设为宽度（正方形）。
    pub fn compact(mut self, val: bool) -> Self {
        self.compact = val;
        self
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.label = next.label;
        self.key = next.key;
        self.icon = next.icon;
        self.fixed_width = next.fixed_width;
        self.fixed_height = next.fixed_height;
        self.index = next.index;
        self.compact = next.compact;
        self.value_binding = next.value_binding;
        // 同步选中值（不替换 Rc），使 State 驱动的重建能刷新高亮。
        self.active_shared.set(next.active_shared.get());
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::NavItem {
            label: self.label.clone(),
            key: self.key.clone(),
            icon: self.icon.clone(),
            fixed_width: self.fixed_width,
            fixed_height: self.fixed_height,
            index: self.index,
            compact: self.compact,
            active: self.is_active(),
        }
    }

    fn activate(&mut self) {
        if !self.is_active() {
            self.active_shared.set(self.index);
            if let Some(binding) = self.value_binding.as_ref() {
                binding();
            }
            self.pending_change.replace(Some(self.key.clone()));
        }
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
pub struct Navigation<K = String> {
    title: String,
    active: SharedActive,
    items: Vec<NavItem>,
    keys: Vec<K>,
    page_binding: Option<State<K>>,
    width: f32,
    height: f32,
    show_version: bool,
    show_title: bool,
    compact_items: bool,
}

impl<K> Navigation<K>
where
    K: Clone + PartialEq + Display + Send + Sync + 'static,
{
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            active: Rc::new(Cell::new(0)),
            items: Vec::new(),
            keys: Vec::new(),
            page_binding: None,
            width: 200.0,
            height: 720.0,
            show_version: true,
            show_title: true,
            compact_items: false,
        }
    }

    /// 添加带 typed key 的导航项；key 的 Display 文本用于语义事件和快照。
    pub fn item(mut self, label: &str, key: K) -> Self {
        let index = self.items.len();
        let item = NavItem::new(label, index, self.active.clone()).key(key.to_string());
        self.items.push(item);
        self.keys.push(key);
        self.sync_page_binding();
        self
    }

    pub fn item_with_icon(mut self, label: &str, key: K, icon: &str) -> Self {
        let index = self.items.len();
        let mut item = NavItem::new(label, index, self.active.clone()).key(key.to_string());
        if !icon.is_empty() {
            item = item.icon(icon);
        }
        self.items.push(item);
        self.keys.push(key);
        self.sync_page_binding();
        self
    }

    pub fn active_index(mut self, index: usize) -> Self {
        self.page_binding = None;
        self.active.set(index);
        self
    }

    /// 将选中项双向绑定到外部页面 State；State 不匹配任何 key 时不选中项。
    pub fn active_page(mut self, state: &State<K>) -> Self {
        self.page_binding = Some(state.clone());
        self.sync_page_binding();
        self
    }

    pub fn active_key(&self) -> Option<&K> {
        self.keys.get(self.active.get())
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

    /// 是否显示标题栏（默认 true）。紧凑导航栏可设为 false。
    pub fn show_title(mut self, show: bool) -> Self {
        self.show_title = show;
        self
    }

    /// 紧凑模式：所有项变为纯图标按钮，无文字标签，高度自动设为宽度。
    pub fn compact(mut self, val: bool) -> Self {
        self.compact_items = val;
        self
    }

    pub fn build(
        mut self,
        tokens: &dyn crate::ui::traits::TokenProvider,
    ) -> crate::ui::core::widget::WidgetNode {
        self.sync_page_binding();
        let loc = crate::ui::locale::use_locale();
        use crate::ui::widgets::{Container, Divider, Label};
        use crate::ui::IntoWidgetNode;

        let item_h = if self.compact_items { self.width } else { 36.0 };
        let mut children: Vec<crate::ui::core::widget::WidgetNode> = Vec::new();

        if self.show_title {
            children.push(
                Label::new(&self.title)
                    .color(tokens.color_primary())
                    .font_size(20.0)
                    .size(self.width, 52.0)
                    .into_node(),
            );

            children.push(
                Divider::new()
                    .color(tokens.color_border_secondary())
                    .into_node(),
            );
        }

        for item in self.items {
            let mut nav_item = item;
            nav_item.fixed_width = self.width;
            nav_item.fixed_height = item_h;
            if self.compact_items {
                nav_item.compact = true;
            }
            children.push(nav_item.into_node());
        }

        children.push(
            Container::new()
                .size(self.width, 0.0)
                .flex_grow(1.0)
                .into_node(),
        );

        if self.show_version {
            children.push(
                Label::new(loc.nav_version)
                    .color(tokens.color_text_quaternary())
                    .font_size(11.0)
                    .size(self.width, 24.0)
                    .into_node(),
            );
        }

        crate::ui::core::widget::WidgetNode::new(
            Box::new(
                Container::new()
                    .bg(tokens.color_bg_container())
                    .dir(crate::ui::layout::FlexDirection::Column)
                    .w(self.width)
                    // 侧栏在 Row 父容器中仅固定宽度，禁止 flex-grow 抢占主轴（水平）空间
                    .flex_shrink(0.0),
            ),
            children,
        )
    }

    fn sync_page_binding(&mut self) {
        let Some(state) = self.page_binding.as_ref() else {
            return;
        };
        let selected = state.get();
        self.active.set(
            self.keys
                .iter()
                .position(|key| key == &selected)
                .unwrap_or(usize::MAX),
        );
        for (item, key) in self.items.iter_mut().zip(&self.keys) {
            let state = state.clone();
            let key = key.clone();
            item.value_binding = Some(Rc::new(move || {
                if state.get() != key {
                    state.set(key.clone());
                }
            }));
        }
    }
}
