//! Navigation widgets — NavItem and Navigation sidebar.
//!
//! Ant Design style sidebar navigation with indicator bar, icons, labels,
//! hover/active states, and optional subtitle.
//!
//! NavItem 使用共享的 `Rc<Cell<usize>>` 管理选中索引，
//! 点击任一 NavItem 自动更新共享状态，其他项自动取消选中。

use std::cell::{Cell, RefCell};
use std::fmt::Display;
use std::rc::Rc;

use crate::core::{Constraints, Rect, Size, WidgetId};
use crate::draw::Radius;
use crate::ui::reactive::state::State;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent, View, ViewNode,
    WidgetTree,
};
use crate::widget;

mod presentation;
use presentation::*;

/// 共享的导航选中索引 —— 多个 NavItem 持有同一份 Rc 即可联动。
pub type SharedActive = Rc<Cell<usize>>;

/// 导航项绘制区域：上下各扩 0.5px，避免局部重绘与相邻项出现 1px 接缝。
fn nav_item_paint_rect(frame: Rect, min_w: f32, min_h: f32, layout: &NavLayoutVisual) -> Rect {
    Rect::new(
        frame.x,
        frame.y - layout.paint_vertical_expand,
        frame.w.max(min_w),
        frame.h.max(min_h) + layout.paint_height_expand,
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
widget! {
    /// 展示图标与标签并通过稳定键发布导航动作的侧边栏条目组件。
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
        // 同目录 UIX 生成的唯一静态视觉表。
        #[snapshot(skip)]
        visual: &'static NavVisual,
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

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|key| SemanticEvent::change(id, key))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let active = self.index == self.active_shared.get();
        // 标准态与紧凑态同帧共享一次主题解析。
        let visual = self.visual.resolve(ctx.tokens());
        let primary = visual.primary;
        let primary_bg = visual.primary_background;
        let text = visual.text;
        let text_secondary = visual.text_secondary;
        let fill_tertiary = visual.fill_tertiary;
        let bg_container = visual.container_background;
        let bg_elevated = visual.elevated_background;
        let layout = &self.visual.layout;
        let typography = &self.visual.typography;

        let item_frame = nav_item_paint_rect(
            frame,
            self.fixed_width,
            self.fixed_height,
            layout,
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
                    self.visual.chrome.focus_width,
                    Some(Radius::uniform(visual.radius)),
                );
            }

            if !self.icon.is_empty() {
                crate::ui::widgets::icon::Icon::paint_in_frame(
                    ctx,
                    &self.icon,
                    item_frame,
                    text_color,
                    typography.compact_icon,
                );
            } else if !self.label.is_empty() {
                let display = &self.label[..self
                    .label
                    .char_indices()
                    .nth(1)
                    .map(|(i, _)| i)
                    .unwrap_or(self.label.len())];
                let fs = typography.compact_fallback;
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
        let indicator_w = layout.indicator_width;

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
            ctx.fill_rect(bar, primary, Some(Radius::uniform(layout.indicator_radius)));
        }

        let mut cursor_x = frame.x + indicator_w;

        let row_h = frame.h.max(self.fixed_height);
        if !self.icon.is_empty() {
            let icon_slot = Rect::new(
                cursor_x + layout.icon_start,
                frame.y,
                layout.icon_slot_width,
                row_h,
            );
            // 先用 UI 字体光学中心画图标，再画标签（见 Icon::paint_in_frame）
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                &self.icon,
                icon_slot,
                icon_color,
                typography.icon,
            );
            cursor_x += layout.icon_advance;
        } else {
            cursor_x += if active {
                layout.active_label_start
            } else {
                layout.label_start
            };
        }

        let label_w = (frame.x + frame.w - cursor_x).max(0.0);
        let label_area = Rect::new(cursor_x, frame.y, label_w, row_h);
        ctx.draw_text_in_frame(
            &self.label,
            label_area,
            label_color,
            visual.label_font_size,
        );
        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(
                item_frame,
                primary,
                self.visual.chrome.focus_width,
                Some(Radius::uniform(visual.radius)),
            );
        }
    }
}

impl NavItem {
    fn intrinsic_size(&self) -> Size {
        Size::new(self.fixed_width, self.fixed_height)
    }

    /// 使用标签、索引和组拥有的共享选中状态创建导航项。
    pub fn new(label: &str, index: usize, active_shared: SharedActive) -> Self {
        let visual = NAV_VISUAL_REF;
        Self {
            label: label.to_string(),
            key: index.to_string(),
            icon: String::new(),
            hovered: false,
            fixed_width: visual.layout.default_width,
            fixed_height: visual.layout.default_item_height,
            index,
            active_shared,
            compact: false,
            focused: false,
            value_binding: None,
            pending_change: RefCell::new(None),
            visual,
        }
    }

    /// 设置用于语义事件和快照的稳定导航键。
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = key.into();
        self
    }

    /// 设置显示在标签前方的 Lucide 图标名称。
    pub fn icon(mut self, icon: &str) -> Self {
        self.icon = icon.to_string();
        self
    }

    /// 返回导航项显示的文字标签。
    pub fn label_text(&self) -> &str {
        &self.label
    }

    /// 返回导航项用于语义事件和快照的稳定键。
    pub fn nav_key(&self) -> &str {
        &self.key
    }

    /// 返回导航项在所属组中的零基索引。
    pub fn nav_index(&self) -> usize {
        self.index
    }

    /// 返回共享选中索引当前是否指向该导航项。
    pub fn is_active(&self) -> bool {
        self.active_shared.get() == self.index
    }

    /// 设置导航项请求的固定宽度。
    pub fn width(mut self, w: f32) -> Self {
        self.fixed_width = w;
        self
    }

    /// 设置导航项请求的固定高度。
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
        // 同步 UIX 生成的视觉表引用，不保留 Rust 视觉副本。
        self.visual = next.visual;
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
    /// 创建选中首项且不含导航项的共享选中组。
    pub fn new() -> Self {
        Self::default()
    }

    /// 按声明顺序添加带可选图标的导航项。
    pub fn item(mut self, label: &str, icon: &str) -> Self {
        let index = self.items.len();
        let mut item = NavItem::new(label, index, self.active.clone());
        if !icon.is_empty() {
            item = item.icon(icon);
        }
        self.items.push(item);
        self
    }

    /// 设置组内共享的初始选中索引。
    pub fn active_index(self, index: usize) -> Self {
        self.active.set(index);
        self
    }

    /// 消费组并返回共享同一选中状态的导航项列表。
    pub fn build(self) -> Vec<NavItem> {
        self.items
    }

    /// 返回组拥有的共享选中索引句柄。
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
    collapsed_state: Option<State<bool>>,
    collapse_callback: Option<Rc<dyn Fn(bool)>>,
}

impl<K> Navigation<K>
where
    K: Clone + PartialEq + Display + Send + Sync + 'static,
{
    /// 使用标题创建空的 typed-key 侧边栏导航容器。
    pub fn new(title: &str) -> Self {
        let visual = NAV_VISUAL_REF;
        Self {
            title: title.to_string(),
            active: Rc::new(Cell::new(0)),
            items: Vec::new(),
            keys: Vec::new(),
            page_binding: None,
            width: visual.layout.default_width,
            height: visual.layout.default_shell_height,
            show_version: visual.defaults.show_version,
            show_title: visual.defaults.show_title,
            compact_items: visual.defaults.compact_items,
            collapsed_state: None,
            collapse_callback: None,
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

    /// 添加带 typed key 与 Lucide 图标的导航项。
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

    /// 设置非受控模式的选中索引，并清除页面状态绑定。
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

    /// 返回当前选中索引对应的 typed key；索引无效时返回空值。
    pub fn active_key(&self) -> Option<&K> {
        self.keys.get(self.active.get())
    }

    /// 返回容器拥有的共享选中索引句柄。
    pub fn active(&self) -> &SharedActive {
        &self.active
    }

    /// 设置展开状态下导航容器请求的宽度。
    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }

    /// 设置导航容器请求的高度。
    pub fn height(mut self, h: f32) -> Self {
        self.height = h;
        self
    }

    /// 设置是否在导航底部显示应用版本信息。
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

    /// 将折叠状态双向绑定到调用方拥有的响应式状态。
    pub fn collapsed(mut self, state: &State<bool>) -> Self {
        self.collapsed_state = Some(state.clone());
        self
    }

    /// 注册折叠状态变化回调；未绑定状态时创建内部折叠状态。
    pub fn on_collapse<F>(mut self, callback: F) -> Self
    where
        F: Fn(bool) + 'static,
    {
        // `on_collapse` is useful on its own as well as with a caller-owned
        // state.  Keep an internal state for the former so the generated
        // toggle remains an actual interaction instead of a callback-only
        // decoration.
        if self.collapsed_state.is_none() {
            self.collapsed_state = Some(State::new(false));
        }
        self.collapse_callback = Some(Rc::new(callback));
        self
    }

    /// 使用令牌 Provider 构建包含标题、导航项和可选版本信息的节点树。
    pub fn build(
        mut self,
        tokens: &dyn crate::ui::theme::traits::TokenProvider,
    ) -> crate::ui::widget_runtime::widget::WidgetNode {
        self.sync_page_binding();
        // 兼容构建器的标题、导航项与版本区共享一次主题解析。
        let visual = NAV_VISUAL_REF.resolve(tokens);
        let layout = &NAV_VISUAL_REF.layout;
        let typography = &NAV_VISUAL_REF.typography;
        let collapsed = self.collapsed_state.as_ref().is_some_and(State::get);
        let compact_items = self.compact_items || collapsed;
        let width = if collapsed {
            self.height.min(self.width).max(layout.compact_min_width)
        } else {
            self.width
        };
        let loc = crate::ui::widget_runtime::locale::use_locale();
        use crate::ui::IntoWidgetNode;
        use crate::ui::widgets::{Container, Divider, Label};

        let item_h = if compact_items {
            width
        } else {
            layout.default_item_height
        };
        let mut children: Vec<crate::ui::widget_runtime::widget::WidgetNode> = Vec::new();

        if let Some(collapsed_state) = self.collapsed_state.clone() {
            let callback = self.collapse_callback.clone();
            let label = if collapsed { "展开" } else { "收起" };
            let toggle_view: crate::ui::view::ViewNode = crate::ui::widgets::button(label)
                .on_click_fn(move || {
                    let next = !collapsed_state.get();
                    collapsed_state.set(next);
                    if let Some(callback) = callback.as_ref() {
                        callback(next);
                    }
                })
                .into();
            let toggle = crate::ui::adapter::ViewAdapter::expand(
                toggle_view.width(width).height(layout.toggle_height),
            );
            children.push(toggle);
        }

        if self.show_title && !collapsed {
            children.push(
                Label::new(&self.title)
                    .color(visual.primary)
                    .font_size(typography.title)
                    .size(width, layout.title_height)
                    .into_node(),
            );

            children.push(Divider::new().color(visual.border_secondary).into_node());
        }

        for item in self.items {
            let mut nav_item = item;
            nav_item.fixed_width = width;
            nav_item.fixed_height = item_h;
            if compact_items {
                nav_item.compact = true;
            }
            children.push(nav_item.into_node());
        }

        children.push(Container::new().size(width, 0.0).flex_grow(1.0).into_node());

        if self.show_version && !collapsed {
            children.push(
                Label::new(loc.nav_version)
                    .color(visual.text_quaternary)
                    .font_size(typography.version)
                    .size(width, layout.version_height)
                    .into_node(),
            );
        }

        crate::ui::widget_runtime::widget::WidgetNode::new(
            Box::new(
                Container::new()
                    .bg(visual.container_background)
                    .dir(crate::ui::layout::FlexDirection::Column)
                    .w(width)
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

// 新 UIX 组合契约固定从默认 Navigation 类型入口构造 typed Menu 子树。
impl Navigation<String> {
    /// 构造只拥有侧栏外壳并复用受控 Menu 唯一事实的 Navigation View。
    pub fn controlled<K, I>(
        title: impl Into<String>,
        items: I,
        active: &State<Option<K>>,
        open: &State<Vec<K>>,
        collapsed: &State<bool>,
        version: Option<String>,
    ) -> crate::ui::view::ViewNode
    where
        // typed key 必须满足 Menu 的值映射与跨线程 State 契约。
        K: Clone + PartialEq + Display + Send + Sync + 'static,
        // 调用方提供拥有型共享 MenuItem 树。
        I: IntoIterator<Item = crate::ui::widgets::MenuItem<K>>,
    {
        // Menu 是选择、展开、重复 key 诊断和交互的唯一 owner。
        let menu = crate::ui::widgets::Menu::controlled(items, active, open)
            // Navigation 固定使用垂直菜单布局。
            .mode(crate::ui::widgets::MenuMode::Vertical)
            // Navigation 内递归菜单组允许展开与收起。
            .collapsible(true)
            // 整栏折叠只改变 Menu 呈现，不改写 active/open 状态。
            .compact_when(collapsed);
        // 外壳只拥有标题、版本与整栏折叠状态。
        let shell = crate::ui::widgets::NavigationShell::new(title, version, collapsed);
        // 公开 ViewNode 明确外壳对唯一 Menu 子组件的实例所有权。
        crate::ui::view::ViewNode::new(
            // 侧栏外壳成为组合根。
            shell,
            // Menu 是唯一直接子树。
            vec![crate::ui::view::ViewNode::leaf(menu)],
        )
    }
}

// UIX 只注入静态视觉表，Rust 内核继续拥有稳定 key、共享状态与事件。
fn build_nav_item_view(mut kernel: NavItem, visual: &'static NavVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for NavItem {
    fn build(self) -> ViewNode {
        build_nav_item_view(self, NAV_VISUAL_REF)
    }
}
