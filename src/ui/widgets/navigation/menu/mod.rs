//! Menu widget - horizontal or vertical navigation menu.
//!
//! Supports hover highlight, active selection, disabled items, icons, and
//! keyboard navigation.
// 紧凑侧栏呈现拆分到子模块，避免 Menu 主文件超过规模边界。
mod compact;
// typed 受控构造拆分到子模块，保持状态映射边界集中。
mod controlled;
mod presentation;

use crate::core::{Constraints, Rect, Size};
use crate::draw::Radius;
use crate::ui::SnapshotFields;
use crate::ui::reactive::state::State;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, View, ViewNode, WidgetId,
    WidgetTree,
};
use crate::widget;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::fmt::Display;
use std::rc::Rc;

use presentation::*;

/// Menu direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuMode {
    /// 在一行中从左到右排列顶层菜单项。
    Horizontal,
    /// 从上到下排列菜单项。
    Vertical,
    /// Inline 菜单使用垂直布局并保持子项展开。
    Inline,
}

/// `Menu` key collection accepted by `selected_keys` and `open_keys`.
///
/// A plain collection keeps the existing one-shot builder form, while
/// `State<Vec<String>>` establishes a bidirectional controlled binding.
#[doc(hidden)]
pub trait MenuKeyCollection {
    fn menu_keys(&self) -> Vec<String>;

    fn menu_state(&self) -> Option<State<Vec<String>>> {
        None
    }
}

impl MenuKeyCollection for Vec<String> {
    fn menu_keys(&self) -> Vec<String> {
        self.clone()
    }
}

impl MenuKeyCollection for [String] {
    fn menu_keys(&self) -> Vec<String> {
        self.to_vec()
    }
}

impl<const N: usize> MenuKeyCollection for [String; N] {
    fn menu_keys(&self) -> Vec<String> {
        self.to_vec()
    }
}

impl MenuKeyCollection for State<Vec<String>> {
    fn menu_keys(&self) -> Vec<String> {
        self.get()
    }

    fn menu_state(&self) -> Option<State<Vec<String>>> {
        Some(self.clone())
    }
}

/// 擦除类型参数后连接受控单选与展开状态。
trait MenuControlledBinding {
    /// 返回当前匹配菜单项的稳定字符串 key。
    fn selected_key(&self) -> Option<String>;
    /// 返回当前匹配菜单项的展开 key 集合。
    fn open_keys(&self) -> Vec<String>;
    /// 把用户选择写回调用方拥有的单选状态。
    fn select_key(&self, key: &str);
    /// 把用户展开集合写回调用方拥有的状态。
    fn set_open_keys(&self, keys: &[String]);
}

/// 保存具体 K 与字符串稳定 key 的双向映射。
struct StateMenuControlledBinding<K> {
    /// 调用方拥有的唯一选择事实。
    selected: State<Option<K>>,
    /// 调用方拥有的子菜单展开事实。
    open: State<Vec<K>>,
    /// 按首个有效菜单项顺序保存 key 映射。
    values: Vec<(String, K)>,
}

/// 为任意可比较 key 实现运行时类型擦除绑定。
impl<K> MenuControlledBinding for StateMenuControlledBinding<K>
where
    K: Clone + PartialEq + Send + Sync + 'static,
{
    /// 只把仍在菜单树内的外部选择投影为活动项。
    fn selected_key(&self) -> Option<String> {
        // 读取调用方状态但不改写失效值。
        let selected = self.selected.get()?;
        // 返回首个相等 typed key 对应的稳定字符串。
        self.values
            .iter()
            .find(|(_, value)| value == &selected)
            .map(|(key, _)| key.clone())
    }

    /// 保持外部顺序并过滤菜单树中不存在的展开 key。
    fn open_keys(&self) -> Vec<String> {
        // 读取调用方拥有的 typed key 集合。
        let open = self.open.get();
        // 把每个 typed key 投影到首个有效稳定字符串。
        open.iter()
            .filter_map(|value| {
                // 查找同值的首个菜单项。
                self.values
                    .iter()
                    .find(|(_, candidate)| candidate == value)
                    .map(|(key, _)| key.clone())
            })
            .collect()
    }

    /// 有效用户选择先写回调用方状态。
    fn select_key(&self, key: &str) {
        // 查找稳定字符串对应的 typed key。
        let Some((_, value)) = self.values.iter().find(|(candidate, _)| candidate == key) else {
            // 不存在的 key 不得污染外部状态。
            return;
        };
        // 重复选择不触发额外状态更新。
        if self.selected.get().as_ref() != Some(value) {
            // 写回拥有所有权的 typed key。
            self.selected.set(Some(value.clone()));
        }
    }

    /// 展开变化原子写回 typed key 集合。
    fn set_open_keys(&self, keys: &[String]) {
        // 按内部稳定字符串顺序恢复 typed key。
        let next = keys
            .iter()
            .filter_map(|key| {
                // 查找每个字符串对应的首个 typed key。
                self.values
                    .iter()
                    .find(|(candidate, _)| candidate == key)
                    .map(|(_, value)| value.clone())
            })
            .collect::<Vec<_>>();
        // 只在集合实际变化时写回。
        if self.open.get() != next {
            // 更新调用方拥有的展开事实。
            self.open.set(next);
        }
    }
}

/// Single menu item.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuItem<K = String> {
    /// 与展示标签分离的稳定业务身份。
    pub key: K,
    /// 向用户展示的菜单项文本。
    pub label: String,
    /// 菜单项前显示的图标名称；空字符串表示不显示图标。
    pub icon: String,
    /// 此菜单项包含的递归子菜单项。
    pub children: Vec<MenuItem<K>>,
    /// 指示此菜单项是否禁止选择和键盘交互。
    pub disabled: bool,
}

impl MenuItem<String> {
    /// 创建以展示文本兼作稳定 key 的字符串菜单项。
    pub fn new(label: impl Into<String>) -> Self {
        let label = label.into();
        Self {
            key: label.clone(),
            label,
            icon: String::new(),
            children: Vec::new(),
            disabled: false,
        }
    }

    /// 覆盖字符串菜单项的稳定 key。
    pub fn key(mut self, key: impl Into<String>) -> Self {
        // 保存调用方提供的稳定业务 key。
        self.key = key.into();
        // 返回完成配置的菜单项。
        self
    }

    /// 使用文本 label/key 构造 UIX 默认字符串菜单项。
    pub fn from_text(label: impl Into<String>, key: impl Into<String>) -> Self {
        // 把两个文本输入都提升为拥有所有权的 String。
        Self::with_key(label, key.into())
    }
}

impl<K> MenuItem<K> {
    /// 使用显式 typed key 构造菜单项。
    pub fn with_key(label: impl Into<String>, key: K) -> Self {
        // 保存展示标签、稳定 key 与空的可选元数据。
        Self {
            // typed key 由调用方和受控 State 共同拥有其值语义。
            key,
            // 标签只承担展示，不参与身份。
            label: label.into(),
            // 缺省不绘制图标。
            icon: String::new(),
            // 缺省没有递归子菜单。
            children: Vec::new(),
            // 缺省允许交互。
            disabled: false,
        }
    }

    /// 设置此菜单项包含的递归子菜单项。
    pub fn children(mut self, children: Vec<Self>) -> Self {
        self.children = children;
        self
    }

    /// 设置菜单项前显示的图标名称。
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = icon.into();
        self
    }

    /// 配置菜单项禁用状态。
    pub fn disabled(mut self, disabled: bool) -> Self {
        // 保存交互门控状态。
        self.disabled = disabled;
        // 返回完成配置的菜单项。
        self
    }
}

// Navigation menu widget.
widget! {
    /// 按稳定键管理选择、展开与键盘导航状态的菜单组件。
    pub struct Menu {
        items: Vec<MenuItem>,
        active_key: String,
        mode: MenuMode,
        selected_keys: Vec<String>,
        open_keys: Vec<String>,
        #[snapshot(skip)]
        selected_keys_binding: Option<State<Vec<String>>>,
        #[snapshot(skip)]
        open_keys_binding: Option<State<Vec<String>>>,
        #[snapshot(skip)]
        controlled_binding: Option<Rc<dyn MenuControlledBinding>>,
        #[snapshot(skip)]
        selected_keys_configured: bool,
        #[snapshot(skip)]
        open_keys_configured: bool,
        collapsible: bool,
        #[snapshot(skip)]
        compact_binding: Option<State<bool>>,
        diagnostics: Vec<String>,
        hovered_idx: Cell<usize>,
        focused: bool,
        item_h: f32,
        pending_change: RefCell<Option<String>>,
        // 同目录 UIX 生成的唯一静态视觉表。
        #[snapshot(skip)]
        visual: &'static MenuVisual,
    }

    tab_index => (&self) -> i32 { 1 }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        self.sync_bound_keys();
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let idx = self.item_at(pos.x, pos.y);
                if let Some(i) = idx {
                    let item = self.visible_items().get(i).map(|(item, _)| {
                        (item.disabled, item.key.clone(), !item.children.is_empty())
                    });
                    if let Some((disabled, key, has_children)) = item {
                        if disabled {
                            return EventResult::NotHandled;
                        }
                        self.select_key(key.clone());
                        // Inline 始终展开完整子树，不允许折叠交互改写调用方 openKeys。
                        if self.collapsible && self.mode != MenuMode::Inline && has_children {
                            self.toggle_open_key(&key);
                        }
                        self.hovered_idx.set(i);
                        return EventResult::Handled;
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                let idx = self.item_at(pos.x, pos.y);
                if let Some(i) = idx.filter(|&i| {
                    self.visible_items()
                        .get(i)
                        .is_some_and(|(item, _)| !item.disabled)
                }) {
                    self.hovered_idx.set(i);
                } else {
                    self.hovered_idx.set(usize::MAX);
                }
                EventResult::Handled
            }
            SystemEvent::PointerLeave => {
                self.hovered_idx.set(usize::MAX);
                EventResult::NotHandled
            }
            SystemEvent::FocusIn => { self.focused = true; EventResult::Handled }
            SystemEvent::FocusOut => { self.focused = false; EventResult::Handled }
            SystemEvent::KeyDown { key, .. } => {
                // 方向键只移动高亮选择；Enter 不在此处激活选中：菜单项的选中由
                // PointerDown 与 Click 语义统一驱动（选择即激活并展开子项），
                // 键盘保持与指针一致的单语义来源，避免两套激活路径。
                match (self.mode, key) {
                    (MenuMode::Horizontal, KeyCode::Right)
                    | (MenuMode::Vertical | MenuMode::Inline, KeyCode::Down) => {
                        self.select_adjacent(true);
                        EventResult::Handled
                    }
                    (MenuMode::Horizontal, KeyCode::Left)
                    | (MenuMode::Vertical | MenuMode::Inline, KeyCode::Up) => {
                        self.select_adjacent(false);
                        EventResult::Handled
                    }
                    _ => EventResult::NotHandled,
                }
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

    wants_continuous_pointer_move => (&self) -> bool { true }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        // 横向、纵向与紧凑呈现同帧共享一次主题解析。
        let visual = self.visual.resolve(ctx.tokens());
        let primary = visual.primary;
        let text = visual.text;
        let text_sec = visual.text_secondary;
        let fill = visual.fill_tertiary;
        let layout = &self.visual.layout;
        let typography = &self.visual.typography;
        let active_key = &self.active_key;
        let hovered = self.hovered_idx.get();
        let radius = Radius::uniform(visual.radius);

        match self.mode {
            MenuMode::Horizontal => {
                let mut cx = frame.x;
                for (i, (item, depth)) in self.visible_items().iter().enumerate() {
                    let iw = self.horizontal_item_width(item, *depth);
                    let item_rect = Rect::new(cx, frame.y, iw, self.item_h);
                    let is_active = item.key == *active_key || self.selected_keys.contains(&item.key);
                    let is_hover = i == hovered;
                    let item_c = if item.disabled { text_sec } else if is_active { primary } else { text };
                    if is_active || is_hover {
                        ctx.fill_rect(item_rect, fill, Some(radius));
                    }
                    if is_active {
                        ctx.fill_rect(
                            Rect::new(
                                cx + layout.active_inset,
                                frame.y + self.item_h - layout.active_thickness,
                                (iw - layout.active_inset * 2.0).max(0.0),
                                layout.active_thickness,
                            ),
                            primary,
                            None,
                        );
                    }
                    if item.icon.is_empty() {
                        ctx.text_center(&item.label, item_rect, item_c, visual.label_font_size);
                    } else {
                        let text_width = item.label.len() as f32 * layout.glyph_width;
                        let content_width =
                            layout.icon_slot_width + layout.icon_text_gap + text_width;
                        let content_x = cx + (iw - content_width) * 0.5;
                        crate::ui::widgets::icon::Icon::paint_in_frame(
                            ctx,
                            &item.icon,
                            Rect::new(content_x, frame.y, layout.icon_slot_width, self.item_h),
                            item_c,
                            typography.icon,
                        );
                        ctx.draw_text_in_frame(
                            &item.label,
                            Rect::new(
                                content_x + layout.icon_advance,
                                frame.y,
                                text_width,
                                self.item_h,
                            ),
                            item_c,
                            typography.icon_label,
                        );
                    }
                    cx += iw;
                }
            }
            MenuMode::Vertical | MenuMode::Inline => {
                for (i, (item, depth)) in self.visible_items().iter().enumerate() {
                    let item_y = frame.y + i as f32 * self.item_h;
                    let item_rect = Rect::new(frame.x, item_y, frame.w, self.item_h);
                    let is_active = item.key == *active_key || self.selected_keys.contains(&item.key);
                    let is_hover = i == hovered;
                    let item_c = if item.disabled { text_sec } else if is_active { primary } else { text };
                    if is_active || is_hover {
                        ctx.fill_rect(item_rect, fill, Some(radius));
                    }
                    // 紧凑侧栏绘制由专属呈现模块处理。
                    if self.paint_compact_item(ctx, item, item_rect, item_c, &visual) {
                        // 紧凑项已完成本行绘制。
                        continue;
                    }
                    let label_pad = if item.icon.is_empty() {
                        layout.plain_label_padding
                    } else {
                        layout.icon_label_padding
                    } + *depth as f32 * layout.depth_indent;
                    if !item.icon.is_empty() {
                        let icon_rect = Rect::new(
                            frame.x + layout.vertical_icon_start,
                            item_y,
                            layout.icon_slot_width,
                            self.item_h,
                        );
                        crate::ui::widgets::icon::Icon::paint_in_frame(
                            ctx, &item.icon, icon_rect, item_c, typography.icon,
                        );
                    }
                    let label_rect = Rect::new(
                        frame.x + label_pad,
                        item_y,
                        (frame.w - label_pad - layout.label_end_padding).max(0.0),
                        self.item_h,
                    );
                    ctx.draw_text_in_frame(
                        &item.label,
                        label_rect,
                        item_c,
                        visual.label_font_size,
                    );
                }
            }
        }

        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(
                frame,
                primary,
                self.visual.chrome.focus_width,
                Some(radius),
            );
        }
    }
}

impl Menu {
    fn intrinsic_size(&self) -> Size {
        let items = self.visible_items();
        match self.mode {
            MenuMode::Horizontal => {
                let w = items
                    .iter()
                    .map(|(item, depth)| self.horizontal_item_width(item, *depth))
                    .sum::<f32>();
                Size::new(w.max(self.visual.layout.horizontal_min_width), self.item_h)
            }
            MenuMode::Vertical | MenuMode::Inline => {
                // 折叠侧栏使用稳定紧凑宽度，展开态保持兼容宽度。
                Size::new(
                    if self.is_compact() {
                        self.visual.layout.compact_width
                    } else {
                        self.visual.layout.expanded_width
                    },
                    items.len() as f32 * self.item_h,
                )
            }
        }
    }

    fn item_at(&self, px: f32, py: f32) -> Option<usize> {
        let items = self.visible_items();
        match self.mode {
            MenuMode::Horizontal => {
                if py < 0.0 || py > self.item_h {
                    return None;
                }
                let mut cx = 0.0f32;
                for (i, (item, depth)) in items.iter().enumerate() {
                    let iw = self.horizontal_item_width(item, *depth);
                    if px >= cx && px < cx + iw {
                        return Some(i);
                    }
                    cx += iw;
                }
                None
            }
            MenuMode::Vertical | MenuMode::Inline => {
                let idx = (py / self.item_h) as usize;
                if idx < items.len() && py >= 0.0 {
                    Some(idx)
                } else {
                    None
                }
            }
        }
    }

    fn item_index_of_key(&self, key: &str) -> Option<usize> {
        self.visible_items()
            .iter()
            .position(|(item, _)| item.key == key)
    }

    fn horizontal_item_width(&self, item: &MenuItem, depth: usize) -> f32 {
        item.label.len() as f32 * self.visual.layout.glyph_width
            + self.visual.layout.horizontal_padding
            + depth as f32 * self.visual.layout.depth_indent
            + if item.icon.is_empty() {
                0.0
            } else {
                self.visual.layout.icon_extra_width
            }
    }

    fn select_adjacent(&mut self, forward: bool) {
        let visible = self.visible_items();
        let enabled = visible
            .iter()
            .enumerate()
            .filter_map(|(index, (item, _))| (!item.disabled).then_some(index))
            .collect::<Vec<_>>();
        if enabled.is_empty() {
            return;
        }

        let current = self.item_index_of_key(&self.active_key);
        let next_position = current
            .and_then(|index| enabled.iter().position(|&candidate| candidate == index))
            .map(|position| {
                if forward {
                    (position + 1) % enabled.len()
                } else {
                    (position + enabled.len() - 1) % enabled.len()
                }
            })
            .unwrap_or_else(|| if forward { 0 } else { enabled.len() - 1 });
        let next_key = visible[enabled[next_position]].0.key.clone();
        self.select_key(next_key);
    }

    fn select_key(&mut self, key: String) {
        let changed = self.active_key != key || self.selected_keys != [key.clone()];
        self.active_key.clone_from(&key);
        self.selected_keys = vec![key.clone()];
        self.write_selected_keys();
        if changed {
            self.pending_change.replace(Some(key));
        }
    }

    fn toggle_open_key(&mut self, key: &str) {
        if let Some(index) = self.open_keys.iter().position(|open| open == key) {
            self.open_keys.remove(index);
        } else {
            self.open_keys.push(key.to_string());
        }
        self.write_open_keys();
    }

    fn visible_items(&self) -> Vec<(&MenuItem, usize)> {
        // 整栏折叠时只显示顶层身份，但不改写调用方 openKeys。
        if self.is_compact() {
            // 返回顶层菜单项的稳定源码顺序。
            return self.items.iter().map(|item| (item, 0)).collect();
        }
        fn visit<'a>(
            items: &'a [MenuItem],
            open_keys: &[String],
            expand_all: bool,
            depth: usize,
            output: &mut Vec<(&'a MenuItem, usize)>,
        ) {
            for item in items {
                output.push((item, depth));
                if !item.children.is_empty() && (expand_all || open_keys.contains(&item.key)) {
                    visit(&item.children, open_keys, expand_all, depth + 1, output);
                }
            }
        }

        let mut output = Vec::new();
        visit(
            &self.items,
            &self.open_keys,
            // Inline 的固定展开语义优先于可折叠交互配置。
            self.mode == MenuMode::Inline || !self.collapsible,
            0,
            &mut output,
        );
        output
    }

    fn sync_bound_keys(&mut self) {
        if let Some(binding) = self.controlled_binding.as_ref() {
            // 外部失效选择保持原值，但界面显示无选择。
            self.active_key = binding.selected_key().unwrap_or_default();
            // 单选快照只包含当前有效活动 key。
            self.selected_keys = (!self.active_key.is_empty())
                .then(|| self.active_key.clone())
                .into_iter()
                .collect();
            // 展开集合过滤失效值但不反向归一化外部状态。
            self.open_keys = Self::deduplicate_keys(binding.open_keys());
            // typed 受控绑定拥有最高优先级。
            return;
        }
        if let Some(state) = self.selected_keys_binding.as_ref() {
            self.apply_selected_keys(state.get());
        }
        if let Some(state) = self.open_keys_binding.as_ref() {
            self.open_keys = Self::deduplicate_keys(state.get());
        }
    }

    fn apply_selected_keys(&mut self, keys: Vec<String>) {
        self.selected_keys = Self::deduplicate_keys(keys);
        self.active_key = self
            .selected_keys
            .iter()
            .find(|key| Self::contains_key(&self.items, key))
            .cloned()
            .unwrap_or_default();
    }

    fn deduplicate_keys(keys: Vec<String>) -> Vec<String> {
        let mut seen = HashSet::new();
        keys.into_iter()
            .filter(|key| seen.insert(key.clone()))
            .collect()
    }

    fn contains_key(items: &[MenuItem], key: &str) -> bool {
        items.iter().any(|item| {
            item.key == key
                || (!item.children.is_empty() && Self::contains_key(&item.children, key))
        })
    }

    fn write_selected_keys(&self) {
        if let Some(binding) = self.controlled_binding.as_ref() {
            // 先把选择写回 typed State<Option<K>>。
            binding.select_key(&self.active_key);
        }
        if let Some(state) = self.selected_keys_binding.as_ref() {
            if state.get() != self.selected_keys {
                state.set(self.selected_keys.clone());
            }
        }
    }

    fn write_open_keys(&self) {
        if let Some(binding) = self.controlled_binding.as_ref() {
            // 把展开集合恢复成 typed key 并写回外部状态。
            binding.set_open_keys(&self.open_keys);
        }
        if let Some(state) = self.open_keys_binding.as_ref() {
            if state.get() != self.open_keys {
                state.set(self.open_keys.clone());
            }
        }
    }
}

impl Default for Menu {
    fn default() -> Self {
        Self::new()
    }
}

impl Menu {
    /// 创建空的水平菜单。
    pub fn new() -> Self {
        let visual = MENU_VISUAL_REF;
        Self {
            items: Vec::new(),
            active_key: String::new(),
            mode: MenuMode::Horizontal,
            selected_keys: Vec::new(),
            open_keys: Vec::new(),
            selected_keys_binding: None,
            open_keys_binding: None,
            controlled_binding: None,
            selected_keys_configured: false,
            open_keys_configured: false,
            collapsible: false,
            compact_binding: None,
            diagnostics: Vec::new(),
            hovered_idx: Cell::new(usize::MAX),
            focused: false,
            item_h: visual.layout.default_item_height,
            pending_change: RefCell::new(None),
            visual,
        }
    }
    /// 替换顶层菜单项，并重新应用已配置的选择状态。
    pub fn items(mut self, items: Vec<MenuItem>) -> Self {
        self.items = items;
        if self.selected_keys_configured {
            self.apply_selected_keys(self.selected_keys.clone());
        }
        self
    }
    /// 追加一个顶层菜单项，并重新应用已配置的选择状态。
    pub fn add_item(mut self, item: MenuItem) -> Self {
        self.items.push(item);
        if self.selected_keys_configured {
            self.apply_selected_keys(self.selected_keys.clone());
        }
        self
    }

    /// 配置含 children 的菜单组是否允许展开和收起。
    pub fn collapsible(mut self, collapsible: bool) -> Self {
        // 保存子菜单组交互策略。
        self.collapsible = collapsible;
        // 不可折叠时全部子菜单都参与布局。
        if !collapsible {
            // 清除内部展开过滤，不改写外部状态。
            self.open_keys.clear();
        }
        // 返回完成配置的菜单。
        self
    }

    /// 设置菜单项的排列和子菜单呈现模式。
    pub fn mode(mut self, m: MenuMode) -> Self {
        self.mode = m;
        self
    }
    /// 设置非受控活动 key，并解除字符串选择状态绑定。
    pub fn active_key(mut self, key: &str) -> Self {
        self.active_key = key.to_string();
        self.selected_keys_binding = None;
        self.selected_keys_configured = false;
        self
    }
    /// 返回当前活动菜单项的稳定 key。
    pub fn get_active_key(&self) -> &str {
        &self.active_key
    }
    /// 更新活动 key、单选集合及已绑定的外部选择状态。
    pub fn set_active_key(&mut self, key: &str) {
        self.active_key = key.to_string();
        self.selected_keys = if key.is_empty() {
            Vec::new()
        } else {
            vec![key.to_string()]
        };
        self.write_selected_keys();
    }
    /// 设置每个可见菜单行的高度。
    pub fn item_height(mut self, h: f32) -> Self {
        self.item_h = h;
        self
    }

    /// 设置选中 key；传入 `State<Vec<String>>` 时建立双向受控绑定。
    pub fn selected_keys<T>(mut self, keys: &T) -> Self
    where
        T: MenuKeyCollection + ?Sized,
    {
        self.selected_keys_binding = keys.menu_state();
        self.selected_keys_configured = true;
        self.apply_selected_keys(keys.menu_keys());
        self
    }

    /// 设置展开 key；传入 `State<Vec<String>>` 时建立双向受控绑定。
    pub fn open_keys<T>(mut self, keys: &T) -> Self
    where
        T: MenuKeyCollection + ?Sized,
    {
        self.open_keys_binding = keys.menu_state();
        self.open_keys_configured = true;
        self.open_keys = Self::deduplicate_keys(keys.menu_keys());
        self
    }

    /// 返回当前选中菜单项的稳定 key 集合。
    pub fn get_selected_keys(&self) -> &[String] {
        &self.selected_keys
    }

    /// 返回当前展开菜单组的稳定 key 集合。
    pub fn get_open_keys(&self) -> &[String] {
        &self.open_keys
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let previous_selected_keys = std::mem::take(&mut self.selected_keys);
        let previous_open_keys = std::mem::take(&mut self.open_keys);
        self.items = next.items;
        self.mode = next.mode;
        self.item_h = next.item_h;
        self.selected_keys_binding = next.selected_keys_binding;
        self.open_keys_binding = next.open_keys_binding;
        self.controlled_binding = next.controlled_binding;
        self.selected_keys_configured = next.selected_keys_configured;
        self.open_keys_configured = next.open_keys_configured;
        self.collapsible = next.collapsible;
        self.compact_binding = next.compact_binding;
        self.diagnostics = next.diagnostics;
        // 同步 UIX 生成的视觉表引用，不保留 Rust 视觉副本。
        self.visual = next.visual;
        if self.controlled_binding.is_some() {
            // typed 受控状态始终覆盖组件内部兼容状态。
            self.sync_bound_keys();
        } else if self.selected_keys_configured {
            self.apply_selected_keys(next.selected_keys);
        } else {
            self.selected_keys = previous_selected_keys;
        }
        // typed 受控绑定已经从外部唯一事实源同步展开集合，不再使用兼容状态覆盖。
        if self.controlled_binding.is_none() {
            // 兼容 String 状态绑定继续采用既有重建语义。
            self.open_keys = if self.open_keys_configured {
                // 显式配置时采用新视图携带的展开集合。
                next.open_keys
            } else {
                // 非受控模式保留旧视图的内部展开集合。
                previous_open_keys
            };
        }
        self.hovered_idx.set(usize::MAX);
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Menu {
            items: self.items.clone(),
            active_key: self.active_key.clone(),
            mode: self.mode,
            item_h: self.item_h,
        }
    }

    /// 擦除 typed 菜单项并按全树首项优先去除重复 key。
    fn erase_controlled_items<K, I>(items: I) -> (Vec<MenuItem>, Vec<(String, K)>, Vec<String>)
    where
        K: Display,
        I: IntoIterator<Item = MenuItem<K>>,
    {
        /// 递归转换同一棵菜单树。
        fn visit<K>(
            items: impl IntoIterator<Item = MenuItem<K>>,
            seen: &mut HashSet<String>,
            values: &mut Vec<(String, K)>,
            diagnostics: &mut Vec<String>,
        ) -> Vec<MenuItem>
        where
            K: Display,
        {
            // 保存当前层通过全局唯一门禁的菜单项。
            let mut output = Vec::new();
            // 按源码或调用方集合顺序遍历菜单项。
            for item in items {
                // 拆出 typed key 与展示字段，避免不必要克隆。
                let MenuItem {
                    key,
                    label,
                    icon,
                    children,
                    disabled,
                } = item;
                // Display 文本成为跨类型擦除边界的稳定 key。
                let key_text = key.to_string();
                // 重复 key 保留首项并记录确定诊断。
                if !seen.insert(key_text.clone()) {
                    // 记录可由测试、诊断面板或日志消费的消息。
                    diagnostics.push(format!("Menu 忽略重复 key {key_text:?}，保留首项"));
                    // 被忽略项及其子树不再拥有运行时身份。
                    continue;
                }
                // 保存稳定字符串到 typed key 的首项映射。
                values.push((key_text.clone(), key));
                // 子树共享同一 seen 集合以保证全局唯一。
                let children = visit(children, seen, values, diagnostics);
                // 生成非泛型绘制层拥有的菜单项。
                output.push(MenuItem {
                    // 擦除后的 key 继续承担渲染与语义事件身份。
                    key: key_text,
                    // 保留展示标签。
                    label,
                    // 保留可选图标。
                    icon,
                    // 保留去重后的递归子树。
                    children,
                    // 保留交互禁用状态。
                    disabled,
                });
            }
            // 返回当前层的稳定菜单项集合。
            output
        }

        // 全树共享已见 key 集合。
        let mut seen = HashSet::new();
        // 保存字符串到 typed key 的回写映射。
        let mut values = Vec::new();
        // 保存重复 key 等可观察诊断。
        let mut diagnostics = Vec::new();
        // 递归擦除并去重完整输入树。
        let items = visit(items, &mut seen, &mut values, &mut diagnostics);
        // 返回绘制树、typed 映射与诊断。
        (items, values, diagnostics)
    }
}

// UIX 只注入静态视觉表，Rust 内核继续拥有递归数据、受控状态与事件。
fn build_menu_view(mut kernel: Menu, visual: &'static MenuVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Menu {
    fn build(self) -> ViewNode {
        build_menu_view(self, MENU_VISUAL_REF)
    }
}
