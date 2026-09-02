//! MenuBar widget — 横向顶级菜单入口条。
//!
//! 入口行与每个入口的下拉弹层全部由本内核自绘；声明方通过统一样式
//! 覆盖入口前景与背景，弹层视觉与 Dropdown 弹层共享同一套浮层语言。

use std::cell::RefCell;

use crate::core::{Constraints, Rect, Size};
use crate::draw::Radius;
use crate::ui::ThemeTokens;
use crate::ui::animation::{TransitionPlayer, presets};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::theme::style::Style;
use crate::widget;
// 弹层动画颜色衰减复用 widgets 层共享辅助，不依赖反馈 capability。
use crate::ui::widgets::fade_token_color;
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent, View, ViewNode,
    WidgetId, WidgetTree,
};

mod presentation;
use presentation::*;

/// 菜单栏下拉弹层中的可选择条目或分隔线。
#[derive(Debug, Clone, PartialEq)]
pub struct MenuBarItem {
    /// 与展示 label 分离的稳定业务身份。
    pub key: String,
    /// 向用户展示的条目文本。
    pub label: String,
    /// 条目前显示的图标名称；空字符串表示不显示图标。
    pub icon: String,
    /// 指示此条目是否禁止选择和键盘高亮。
    pub disabled: bool,
    /// 指示此条目是否仅渲染为不可选择的分隔线。
    pub divider: bool,
}

impl MenuBarItem {
    /// 创建以展示文本兼作稳定 key 的叶子条目。
    pub fn new(label: impl Into<String>) -> Self {
        let label = label.into();
        Self {
            key: label.clone(),
            label,
            icon: String::new(),
            disabled: false,
            divider: false,
        }
    }

    /// UIX 内联构造入口：显式区分展示文本与稳定 key。
    pub fn from_text(label: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            icon: String::new(),
            disabled: false,
            divider: false,
        }
    }

    /// 设置条目前显示的图标名称。
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = icon.into();
        self
    }

    /// 禁止选择与键盘高亮。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 创建不可选择的分隔线条目。
    pub fn divider() -> Self {
        Self {
            key: String::new(),
            label: String::new(),
            icon: String::new(),
            disabled: true,
            divider: true,
        }
    }
}

/// 菜单栏的一个顶级菜单：入口文案与下拉条目集合。
#[derive(Debug, Clone, PartialEq)]
pub struct MenuBarMenu {
    /// 与展示 label 分离的稳定业务身份。
    pub key: String,
    /// 入口向用户展示的文本。
    pub label: String,
    /// 打开后展示的条目序列。
    pub items: Vec<MenuBarItem>,
}

impl MenuBarMenu {
    /// 创建以展示文本兼作稳定 key 的空菜单。
    pub fn new(label: impl Into<String>) -> Self {
        let label = label.into();
        Self {
            key: label.clone(),
            label,
            items: Vec::new(),
        }
    }

    /// UIX 内联构造入口：显式区分入口文本与稳定 key。
    pub fn from_text(label: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            items: Vec::new(),
        }
    }

    /// 追加一个条目。
    pub fn item(mut self, item: MenuBarItem) -> Self {
        self.items.push(item);
        self
    }

    /// 追加一组条目。
    pub fn items(mut self, items: Vec<MenuBarItem>) -> Self {
        self.items.extend(items);
        self
    }
}

widget! {
    /// 横向顶级菜单入口条，入口点击展开下拉弹层。
    pub struct MenuBar {
        menus: Vec<MenuBarMenu>,
        /// 当前展开（含退出动画中）的顶级菜单索引。
        open_index: Option<usize>,
        closing: bool,
        /// 弹层内键盘/悬停高亮条目索引。
        highlighted_index: Option<usize>,
        /// 悬停中的入口索引，驱动已开状态的直接切换。
        hover_entry: Option<usize>,
        transition: TransitionPlayer,
        transition_dirty: bool,
        focused: bool,
        pending_change: RefCell<Option<String>>,
        /// 保存空 key 与重复 key 等可观察数据诊断。
        diagnostics: Vec<String>,
        /// 声明样式桥接的入口前景与背景覆盖。
        view_style: Style,
        // 同目录 UIX 生成的唯一静态视觉表。
        #[snapshot(skip)]
        visual: &'static MenuBarVisual,
    }

    // 菜单栏整体保持一个键盘焦点位，弹层内导航由方向键承担。
    tab_index => (&self) -> i32 { 1 }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    flex_grow => (&self) -> f32 { self.view_style.flex_grow }

    flex_shrink => (&self) -> f32 { self.view_style.flex_shrink }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown { pos, button, .. } if *button == MouseButton::Left => {
                if let Some(index) = self.entry_at(pos.x) {
                    if pos.y >= 0.0 && pos.y <= self.visual.layout.entry_height {
                        if self.open_index == Some(index) && !self.closing {
                            self.close();
                        } else {
                            self.open_entry(index);
                        }
                        return EventResult::Handled;
                    }
                }
                if self.is_open() && !self.closing {
                    if let Some((menu_index, item_index)) = self.item_at(pos.x, pos.y) {
                        if self.open_index == Some(menu_index)
                            && self.select_item(item_index)
                        {
                            return EventResult::Handled;
                        }
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                // 已开状态下悬停其他入口直接切换，保持菜单栏经典交互。
                if let Some(index) = self.entry_at(pos.x) {
                    if pos.y >= 0.0 && pos.y <= self.visual.layout.entry_height {
                        if self.is_open()
                            && !self.closing
                            && self.open_index != Some(index)
                        {
                            self.open_entry(index);
                        }
                        if self.hover_entry != Some(index) {
                            self.hover_entry = Some(index);
                        }
                        return EventResult::Handled;
                    }
                }
                if self.is_open() && !self.closing {
                    let highlighted = self
                        .item_at(pos.x, pos.y)
                        .filter(|(menu_index, _)| self.open_index == Some(*menu_index))
                        .map(|(_, item_index)| item_index)
                        .filter(|&index| self.is_selectable(index));
                    if highlighted != self.highlighted_index {
                        self.highlighted_index = highlighted;
                    }
                }
                EventResult::Handled
            }
            SystemEvent::PointerLeave => {
                self.hover_entry = None;
                if self.is_open() {
                    self.highlighted_index = None;
                }
                EventResult::Handled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.close();
                EventResult::Handled
            }
            // Agent click_at 与真实指针都成对派发抬起；入口行与已开弹层内的抬起
            // 属于菜单栏交互面，消费后保持整次点击语义完整（选择只在按下时发生）。
            SystemEvent::PointerUp { pos, button, .. } if *button == MouseButton::Left => {
                if self.hits_interactive_area(pos.x, pos.y) {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::KeyDown { key, .. } => match key {
                KeyCode::Enter | KeyCode::Space => {
                    if self.is_open() {
                        if let Some(index) = self.highlighted_index {
                            self.select_item(index);
                        }
                    } else if let Some(first) = self.first_entry() {
                        self.open_entry(first);
                    }
                    EventResult::Handled
                }
                KeyCode::Down => {
                    if self.is_open() {
                        self.move_highlight(true);
                    } else if let Some(first) = self.first_entry() {
                        self.open_entry(first);
                    }
                    EventResult::Handled
                }
                KeyCode::Up if self.is_open() => {
                    self.move_highlight(false);
                    EventResult::Handled
                }
                KeyCode::Left if self.is_open() => {
                    self.switch_open_entry(false);
                    EventResult::Handled
                }
                KeyCode::Right if self.is_open() => {
                    self.switch_open_entry(true);
                    EventResult::Handled
                }
                KeyCode::Escape if self.is_present() => {
                    self.close();
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|value| SemanticEvent::change(id, value))
    }

    // 打开状态需要持续指针移动以驱动入口悬停切换。
    wants_continuous_pointer_move => (&self) -> bool { self.is_present() }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let visual = self.visual.resolve(ctx.tokens());
        let layout = &self.visual.layout;
        let typography = &self.visual.typography;
        let entry_radius = Some(Radius::uniform(6.0));

        for index in 0..self.menus.len() {
            let entry = self.entry_rect(frame, index);
            let is_open = self.open_index == Some(index);
            let is_hover = self.hover_entry == Some(index);
            // 打开与悬停各自落到可区分的填充，深色声明可整体覆盖。
            let entry_bg = if is_open {
                Some(self.entry_open_color(&visual, ctx.tokens()))
            } else if is_hover {
                Some(self.entry_hover_color(&visual, ctx.tokens()))
            } else {
                self.view_style
                    .background
                    .map(|value| value.resolve(ctx.tokens()))
            };
            if let Some(bg) = entry_bg {
                ctx.fill_rect(entry, bg, entry_radius);
            }
            let text_color = self.entry_text_color(&visual, ctx.tokens());
            ctx.text_center(
                &self.menus[index].label,
                entry,
                text_color,
                typography.entry_label,
            );
            // 键盘焦点可见时绘制入口整体焦点环。
            if self.focused && tree.keyboard_focus_visible() && !self.is_present() {
                ctx.stroke_rect(
                    entry,
                    visual.menu.border,
                    self.visual.chrome.focus_width,
                    entry_radius,
                );
            }
        }

        if !self.is_present() {
            return;
        }
        let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
        let bg = fade_token_color(visual.menu.background, opacity);
        let border = fade_token_color(visual.menu.border, opacity);
        let text_color = fade_token_color(visual.menu.text, opacity);
        let disabled_color = fade_token_color(visual.menu.text_quaternary, opacity);
        let highlight = fade_token_color(visual.menu.highlight, opacity);

        let menu_rect = self.menu_rect(frame).expect("已开状态必有弹层矩形");
        let radius = Some(Radius::uniform(visual.chrome.radius));
        let shadow = visual.chrome.shadow;
        ctx.draw_box_shadow(
            menu_rect,
            shadow.layer_1.2,
            shadow.layer_1.0,
            shadow.layer_1.1,
            shadow.layer_1.3,
            radius,
        );
        ctx.fill_rect(menu_rect, bg, radius);
        ctx.stroke_rect(menu_rect, border, self.visual.chrome.border_width, radius);

        let mut y = menu_rect.y;
        for (index, item) in self.open_items().enumerate() {
            let h = self.row_height(item);
            let item_rect = Rect::new(menu_rect.x, y, menu_rect.w, h);
            if item.divider {
                let inset = layout.divider_inset.min(menu_rect.w * 0.5);
                ctx.fill_rect(
                    Rect::new(
                        menu_rect.x + inset,
                        y + h * 0.5,
                        (menu_rect.w - inset * 2.0).max(0.0),
                        layout.divider_thickness,
                    ),
                    border,
                    None,
                );
            } else {
                if self.highlighted_index == Some(index) {
                    ctx.fill_rect(item_rect, highlight, None);
                }
                let color = if item.disabled {
                    disabled_color
                } else {
                    text_color
                };
                let mut content_x = menu_rect.x + layout.content_start;
                if !item.icon.is_empty() {
                    let icon_rect = Rect::new(content_x, y, layout.icon_slot_width, h);
                    crate::ui::widgets::icon::Icon::paint_in_frame(
                        ctx,
                        &item.icon,
                        icon_rect,
                        color,
                        typography.icon,
                    );
                    content_x += layout.icon_advance;
                }
                let label_rect = Rect::new(
                    content_x,
                    y,
                    (menu_rect.x + menu_rect.w - content_x - layout.label_end_padding).max(0.0),
                    h,
                );
                ctx.draw_text_in_frame(&item.label, label_rect, color, typography.label);
            }
            y += h;
        }
    }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        if let Some(menu) = self.menu_rect(frame) {
            frame.union(&menu)
        } else {
            frame
        }
    }

    // 打开时登记浮层，保证外部点击关闭与 Esc 路由在 OverlayStack 生效。
    overlay_entry => (&self, id: crate::ui::WidgetId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        let menu = self.menu_rect(frame)?;
        let bounds = frame.union(&menu);
        Some(
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Popover)
                .bounds(bounds)
                .z_index(self.visual.chrome.overlay_z)
                // 外部点击由树的 System 私有取消端口回调 owner 关闭。
                .dismiss_on_outside(true),
        )
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        menu_bar_dirty_rect(frame, self.menu_rect(frame), self.visual)
    }

    update_animation => (&mut self, dt: f64) -> bool {
        if !self.is_present() || self.transition.finished {
            self.transition_dirty = false;
            return false;
        }

        self.transition.update(dt);
        self.transition_dirty = true;

        if self.closing && self.transition.finished {
            self.open_index = None;
            self.closing = false;
            self.highlighted_index = None;
        }

        self.is_present() && !self.transition.finished
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.transition_dirty {
            menu_bar_dirty_rect(frame, self.menu_rect(frame), self.visual)
        } else {
            Rect::zero()
        }
    }
}

impl Default for MenuBar {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuBar {
    /// 创建空菜单栏。
    pub fn new() -> Self {
        let visual = MENUBAR_VISUAL_REF;
        Self {
            menus: Vec::new(),
            open_index: None,
            closing: false,
            highlighted_index: None,
            hover_entry: None,
            transition: TransitionPlayer::new(presets::tooltip_enter()),
            transition_dirty: false,
            focused: false,
            pending_change: RefCell::new(None),
            diagnostics: Vec::new(),
            view_style: Style::default(),
            visual,
        }
    }

    /// 配置要求非空且菜单内唯一 key 的顶级菜单集合。
    pub fn keyed_menus<I>(mut self, menus: I) -> Self
    where
        // UIX 表达式可提供数组、Vec 或其他拥有型迭代器。
        I: IntoIterator<Item = MenuBarMenu>,
    {
        // 对动态数据执行确定的首项优先身份门禁。
        let (menus, diagnostics) = Self::normalize_keyed_menus(menus);
        // 保存通过门禁的完整菜单集合。
        self.menus = menus;
        // 暴露所有被拒绝身份的稳定诊断。
        self.diagnostics = diagnostics;
        self
    }

    /// 返回 keyed 数据物化期间产生的只读诊断。
    pub fn diagnostics(&self) -> &[String] {
        // 调用方可把诊断接入日志、测试或开发工具。
        &self.diagnostics
    }

    /// 返回当前展开的顶级菜单索引。
    pub fn open_index(&self) -> Option<usize> {
        self.open_index
    }

    /// 返回菜单栏是否处于接受交互的打开状态。
    pub fn is_open(&self) -> bool {
        self.open_index.is_some() && !self.closing
    }

    /// 返回菜单栏是否仍需呈现，包括正在执行退出动画的状态。
    pub fn is_present(&self) -> bool {
        self.open_index.is_some()
    }

    /// 打开指定入口的下拉菜单并初始化高亮。
    pub fn open_entry(&mut self, index: usize) {
        if index >= self.menus.len() {
            return;
        }
        self.open_index = Some(index);
        self.closing = false;
        self.highlighted_index = self.next_selectable(None, true);
        self.transition = TransitionPlayer::new(presets::tooltip_enter());
        self.transition_dirty = true;
    }

    /// 关闭菜单并在需要时启动退出动画。
    pub fn close(&mut self) {
        if !self.is_present() {
            self.open_index = None;
            self.closing = false;
            self.highlighted_index = None;
            self.transition_dirty = false;
            return;
        }
        self.closing = true;
        self.transition = TransitionPlayer::new(presets::tooltip_exit());
        self.transition_dirty = true;
    }

    /// 声明样式桥：消费入口前景、背景与浮层布局字段。
    pub(crate) fn apply_view_layout_style(&mut self, style: &Style) {
        // 入口视觉覆盖整体来自统一样式，kernel 不重复解析布局字段。
        self.view_style = style.clone();
    }

    // View DSL 显式 Flex 覆盖（含 0.0），Style::apply 无法表达「设为默认值」。
    pub(crate) fn override_flex_grow(&mut self, grow: f32) {
        self.view_style.flex_grow = grow;
    }

    pub(crate) fn override_flex_shrink(&mut self, shrink: f32) {
        self.view_style.flex_shrink = shrink;
    }

    // keyed 数据身份门禁：入口 key 非空且全栏唯一；条目 key 非空且菜单内唯一。
    fn normalize_keyed_menus<I>(menus: I) -> (Vec<MenuBarMenu>, Vec<String>)
    where
        I: IntoIterator<Item = MenuBarMenu>,
    {
        let mut normalized = Vec::new();
        let mut diagnostics = Vec::new();
        let mut seen_menu_keys = std::collections::BTreeSet::new();
        for menu in menus {
            if menu.key.is_empty() {
                diagnostics.push(format!("菜单入口 {} 缺少稳定 key", menu.label));
                continue;
            }
            if !seen_menu_keys.insert(menu.key.clone()) {
                diagnostics.push(format!("菜单入口 key 重复：{}", menu.key));
                continue;
            }
            // 门禁拒绝时用于整份回落的原始副本。
            let original_items = menu.items.clone();
            let source_items = menu.items;
            let mut items = Vec::with_capacity(source_items.len());
            let mut seen_item_keys = std::collections::BTreeSet::new();
            let mut rejected = false;
            for item in source_items.into_iter() {
                if item.divider {
                    // 分隔线不携带身份，不参与唯一性门禁。
                    items.push(item);
                    continue;
                }
                if item.key.is_empty() {
                    diagnostics.push(format!("菜单 {} 的条目 {} 缺少稳定 key", menu.key, item.label));
                    rejected = true;
                    continue;
                }
                if !seen_item_keys.insert(item.key.clone()) {
                    diagnostics.push(format!("菜单 {} 的条目 key 重复：{}", menu.key, item.key));
                    rejected = true;
                    continue;
                }
                items.push(item);
            }
            if rejected {
                // 存在被拒绝条目时整份菜单保持原顺序回落，避免半份数据。
                items = original_items;
            }
            normalized.push(MenuBarMenu {
                key: menu.key,
                label: menu.label,
                items,
            });
        }
        (normalized, diagnostics)
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        // 保留运行中的打开状态，数据刷新后按索引语义继续有效。
        self.menus = next.menus;
        // keyed 数据诊断随最新声明替换。
        self.diagnostics = next.diagnostics;
        // 同步 UIX 生成的视觉表引用，不保留 Rust 视觉副本。
        self.visual = next.visual;
        // 声明样式覆盖随最新声明替换。
        self.view_style = next.view_style;
        // 打开索引超出新数据范围时立即收起。
        if self.open_index.is_some_and(|index| index >= self.menus.len()) {
            self.open_index = None;
            self.closing = false;
            self.highlighted_index = None;
        } else if self.is_open() {
            // 高亮按新数据重新收敛到首个可选择条目。
            self.highlighted_index = self.next_selectable(None, true);
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::MenuBar {
            menus: self.menus.clone(),
            open_index: self.open_index,
            highlighted_index: self.highlighted_index,
        }
    }

    // ── 布局几何 ────────────────────────────────────────────────────────

    fn intrinsic_size(&self) -> Size {
        let layout = &self.visual.layout;
        let mut width = 0.0;
        for (index, menu) in self.menus.iter().enumerate() {
            if index > 0 {
                width += layout.entry_gap;
            }
            width += self.entry_width(menu);
        }
        Size::new(width.max(layout.entry_min_width), layout.entry_height)
    }

    // 入口自然宽：文字估算加左右内边距，不小于最小可点击宽度。
    fn entry_width(&self, menu: &MenuBarMenu) -> f32 {
        let layout = &self.visual.layout;
        let text_w =
            crate::draw::resources::font::text_backend::estimate_text_metrics(
                &menu.label,
                f32::INFINITY,
                self.visual.typography.entry_label,
            )
            .max_line_width;
        (text_w + layout.entry_padding_x * 2.0).max(layout.entry_min_width)
    }

    // 返回入口在组件局部/绝对共用坐标系中的矩形。
    fn entry_rect(&self, frame: Rect, index: usize) -> Rect {
        let layout = &self.visual.layout;
        let mut x = frame.x;
        for (current, menu) in self.menus.iter().enumerate() {
            let width = self.entry_width(menu);
            if current == index {
                return Rect::new(x, frame.y, width, layout.entry_height);
            }
            x += width + layout.entry_gap;
        }
        Rect::new(frame.x, frame.y, 0.0, layout.entry_height)
    }

    // 命中入口的 x 坐标到入口索引。
    fn entry_at(&self, x: f32) -> Option<usize> {
        let layout = &self.visual.layout;
        let mut cursor = 0.0_f32;
        for (index, menu) in self.menus.iter().enumerate() {
            let width = self.entry_width(menu);
            if x >= cursor && x < cursor + width {
                return Some(index);
            }
            cursor += width + layout.entry_gap;
        }
        None
    }

    // 已开菜单的弹层矩形；入口下方左对齐展开。
    fn menu_rect(&self, frame: Rect) -> Option<Rect> {
        let index = self.open_index?;
        let layout = &self.visual.layout;
        let entry = self.entry_rect(frame, index);
        let mut height = 0.0_f32;
        for item in self.open_items() {
            height += self.row_height(item);
        }
        Some(Rect::new(
            entry.x,
            frame.y + layout.entry_height,
            layout.menu_width,
            height,
        ))
    }

    // 已开弹层的可见总高度。
    fn visible_menu_height(&self) -> f32 {
        let mut height = 0.0_f32;
        for item in self.open_items() {
            height += self.row_height(item);
        }
        height
    }

    fn open_items(&self) -> impl Iterator<Item = &MenuBarItem> {
        self.open_index
            .and_then(|index| self.menus.get(index))
            .into_iter()
            .flat_map(|menu| menu.items.iter())
    }

    fn row_height(&self, item: &MenuBarItem) -> f32 {
        if item.divider {
            self.visual.layout.divider_row_height
        } else {
            self.visual.layout.row_height
        }
    }

    // 命中入口行与已开弹层的完整交互面（含分隔线与禁用行）。
    fn hits_interactive_area(&self, x: f32, y: f32) -> bool {
        let layout = &self.visual.layout;
        if y >= 0.0 && y <= layout.entry_height && self.entry_at(x).is_some() {
            return true;
        }
        if !self.is_present() {
            return false;
        }
        let Some(menu_index) = self.open_index else {
            return false;
        };
        let entry = self.entry_rect(Rect::zero(), menu_index);
        let inside_width = x >= entry.x && x < entry.x + layout.menu_width;
        let inside_height = y > layout.entry_height
            && y <= layout.entry_height + self.visible_menu_height();
        inside_width && inside_height
    }

    // 命中弹层条目的坐标到（菜单索引, 条目索引）。
    fn item_at(&self, x: f32, y: f32) -> Option<(usize, usize)> {
        let menu_index = self.open_index?;
        let layout = &self.visual.layout;
        if y <= layout.entry_height {
            return None;
        }
        let entry = self.entry_rect(Rect::zero(), menu_index);
        if x < entry.x || x >= entry.x + layout.menu_width {
            return None;
        }
        let mut cursor = layout.entry_height;
        for (item_index, item) in self.open_items().enumerate() {
            let height = self.row_height(item);
            if y >= cursor && y < cursor + height {
                return Some((menu_index, item_index));
            }
            cursor += height;
        }
        None
    }

    // ── 交互语义 ────────────────────────────────────────────────────────

    fn first_entry(&self) -> Option<usize> {
        (!self.menus.is_empty()).then_some(0)
    }

    // 已开状态下切换到相邻入口。
    fn switch_open_entry(&mut self, forward: bool) {
        let count = self.menus.len();
        if count == 0 {
            return;
        }
        let current = self.open_index.unwrap_or(0);
        let next = if forward {
            (current + 1) % count
        } else {
            (current + count - 1) % count
        };
        self.open_entry(next);
    }

    fn move_highlight(&mut self, forward: bool) {
        self.highlighted_index = self.next_selectable(self.highlighted_index, forward);
    }

    // 提交条目选择：写稳定 key、关闭弹层并发布 Change 语义。
    fn select_item(&mut self, item_index: usize) -> bool {
        let Some(item) = self
            .open_items()
            .enumerate()
            .find(|(index, _)| *index == item_index)
            .map(|(_, item)| item)
        else {
            return false;
        };
        if item.divider || item.disabled {
            return false;
        }
        let value = item.key.clone();
        self.highlighted_index = Some(item_index);
        self.pending_change.replace(Some(value));
        self.close();
        true
    }

    fn is_selectable(&self, item_index: usize) -> bool {
        self.open_items()
            .enumerate()
            .any(|(index, item)| index == item_index && !item.divider && !item.disabled)
    }

    // 从当前高亮出发寻找下一个可选择条目，环形回绕。
    fn next_selectable(&self, current: Option<usize>, forward: bool) -> Option<usize> {
        let count = self.open_items().count();
        if count == 0 {
            return None;
        }
        let start = match (current, forward) {
            (Some(index), true) => (index + 1) % count,
            (Some(index), false) => (index + count - 1) % count,
            (None, true) => 0,
            (None, false) => count - 1,
        };
        let mut nearest = None;
        for (index, item) in self.open_items().enumerate() {
            if item.divider || item.disabled {
                continue;
            }
            let distance = if forward {
                (index + count - start) % count
            } else {
                (start + count - index) % count
            };
            if nearest.is_none_or(|(_, best_distance)| distance < best_distance) {
                nearest = Some((index, distance));
            }
        }
        nearest.map(|(index, _distance)| index)
    }

    // ── 声明样式覆盖的入口调色 ──────────────────────────────────────────

    // 声明显式前景直接作为入口文字色；默认值与主题中性文本同源，行为等价。
    fn entry_text_color(&self, _visual: &MenuBarVisualResolved, tokens: &dyn ThemeTokens) -> crate::draw::Color {
        self.view_style.color.resolve(tokens)
    }

    fn entry_hover_color(&self, visual: &MenuBarVisualResolved, tokens: &dyn ThemeTokens) -> crate::draw::Color {
        self.view_style
            .background_hover
            .map(|value| value.resolve(tokens))
            .unwrap_or(visual.entry.hover)
    }

    fn entry_open_color(&self, visual: &MenuBarVisualResolved, tokens: &dyn ThemeTokens) -> crate::draw::Color {
        self.view_style
            .background_active
            .or(self.view_style.background_hover)
            .map(|value| value.resolve(tokens))
            .unwrap_or(visual.entry.open)
    }
}

// 弹层脏区：入口行与弹层并集再外扩阴影带。
fn menu_bar_dirty_rect(
    frame: Rect,
    menu: Option<Rect>,
    visual: &MenuBarVisual,
) -> Rect {
    let expanded = match menu {
        Some(menu) => frame.union(&menu),
        None => frame,
    };
    let expand = visual.chrome.shadow_expand;
    Rect::new(
        expanded.x - expand,
        expanded.y - expand,
        expanded.w + expand * 2.0,
        expanded.h + expand * 2.0,
    )
}

// UIX 只注入静态视觉表，Rust 内核继续拥有数据、状态、生命周期与事件。
fn build_menu_bar_view(mut kernel: MenuBar, visual: &'static MenuBarVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for MenuBar {
    fn build(self) -> ViewNode {
        build_menu_bar_view(self, MENUBAR_VISUAL_REF)
    }
}
