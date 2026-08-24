//! Dropdown widget.

use crate::core::{Constraints, Rect, Size};
use crate::draw::{Color, Radius};
use crate::ui::animation::{TransitionPlayer, presets};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::widget;
// 组合 Dropdown 使用组件树的公开子节点测量边界。
use crate::ui::widget_runtime::tree_measure::child_from_tree_with_constraints;
// 引入直接 trigger 子节点的布局快照类型。
use crate::ui::layout::LayoutChild;
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent, View, ViewNode,
    WidgetId, WidgetTree,
};
use std::cell::RefCell;
// 组合 owner 共享一次性 trigger View 建造句柄。
use std::rc::Rc;

// 下拉菜单使用基础层共享的触发方式，不依赖反馈组件族。
use crate::ui::widgets::TriggerMode;

mod presentation;
use presentation::*;

#[derive(Debug, Clone, PartialEq)]
/// 下拉菜单中的可选择条目、分组或分隔线。
pub struct DropdownItem {
    /// 与展示 label 分离的稳定业务身份。
    pub key: String,
    /// 向用户展示的条目文本。
    pub label: String,
    /// 指示此条目是否仅渲染为不可选择的分隔线。
    pub divider: bool,
    /// 指示此条目是否禁止选择和键盘高亮。
    pub disabled: bool,
    /// 条目前显示的图标名称；空字符串表示不显示图标。
    pub icon: String,
    /// 此条目展开时显示的嵌套子菜单条目。
    pub children: Vec<DropdownItem>,
}

impl DropdownItem {
    /// 创建叶子条目，并使用展示文本作为兼容业务 key。
    pub fn new(label: impl Into<String>) -> Self {
        // 兼容旧字符串构造时以 label 作为稳定 key。
        let label = label.into();
        Self {
            // 旧调用方保持 key=label 的既有事件载荷。
            key: label.clone(),
            // 保存展示文字。
            label,
            divider: false,
            disabled: false,
            icon: String::new(),
            children: Vec::new(),
        }
    }

    /// 创建不可选择且不携带业务身份的分隔线条目。
    pub fn divider() -> Self {
        Self {
            // 分隔线不承担可选择身份。
            key: String::new(),
            label: String::new(),
            divider: true,
            disabled: false,
            icon: String::new(),
            children: Vec::new(),
        }
    }

    /// 使用显式 label/key 构造 UIX keyed 选项。
    pub fn from_text(label: impl Into<String>, key: impl Into<String>) -> Self {
        // 复用兼容构造初始化展示字段。
        Self::new(label)
            // 用调用方稳定业务 key 覆盖兼容值。
            .key(key)
    }

    /// 覆盖选项稳定业务 key。
    pub fn key(mut self, key: impl Into<String>) -> Self {
        // 保存与展示 label 无关的身份。
        self.key = key.into();
        // 返回完成配置的拥有型选项。
        self
    }

    /// 设置此条目展开时显示的嵌套子菜单。
    pub fn children(mut self, children: Vec<Self>) -> Self {
        self.children = children;
        self
    }

    /// 设置此条目是否禁止选择和键盘高亮。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 设置条目前显示的图标名称。
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = icon.into();
        self
    }
}

impl From<DropdownItem> for String {
    fn from(item: DropdownItem) -> Self {
        item.label
    }
}

impl From<&str> for DropdownItem {
    fn from(label: &str) -> Self {
        Self::new(label)
    }
}

impl From<String> for DropdownItem {
    fn from(label: String) -> Self {
        Self::new(label)
    }
}

#[derive(Clone)]
struct VisibleDropdownItem {
    item: DropdownItem,
    depth: usize,
}

widget! {
    /// Click-triggered dropdown menu.
    pub struct Dropdown {
        label: String,
        items: Vec<DropdownItem>,
        /// UIX 组合模式由唯一直接子 View 绘制触发器。
        custom_trigger: bool,
        // Dropdown 在声明期拥有完整 trigger ViewNode 子树。
        #[snapshot(skip)]
        custom_trigger_view: Option<Rc<RefCell<Option<crate::ui::view::ViewNode>>>>,
        /// 保存组件树实际登记的直接 trigger 数量。
        trigger_child_count: usize,
        /// 保存空 key 与重复 key 等可观察数据诊断。
        diagnostics: Vec<String>,
        expanded_keys: Vec<String>,
        open: bool,
        transition: TransitionPlayer,
        closing: bool,
        transition_dirty: bool,
        focused: bool,
        selected_index: Option<usize>,
        selected_value: Option<String>,
        highlighted_index: Option<usize>,
        trigger_mode: TriggerMode,
        pending_change: RefCell<Option<String>>,
        // 同目录 UIX 生成的唯一静态视觉表。
        #[snapshot(skip)]
        visual: &'static DropdownVisual,
    }

    // 组合模式把 Tab 焦点交给 trigger 子树；缺失子树时保留可恢复焦点入口。
    tab_index => (&self) -> i32 { i32::from(!self.custom_trigger || self.trigger_child_count == 0) }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    // 指针命中由 Dropdown owner 统一解释，trigger 子树只负责视觉呈现。
    hit_test_children => (&self) -> bool { false }

    // 记录唯一 trigger 子树的挂载与卸载变化。
    on_children_changed => (&mut self, child_count: usize) {
        // 组件树通知成为 trigger 生命周期的当前事实。
        self.trigger_child_count = child_count;
    }

    // 将声明期拥有的 trigger ViewNode 一次性交给组件树物化。
    build_view_children => (&self) -> Vec<crate::ui::view::ViewNode> {
        // 取走待物化子树，避免同一声明重复挂载。
        self.custom_trigger_view
            // 组合模式才持有一次性建造句柄。
            .as_ref()
            // 从共享单元取走完整 ViewNode。
            .and_then(|view| view.borrow_mut().take())
            // 零或一个 trigger 统一转成迭代器。
            .into_iter()
            // 返回组件树可消费的直接子节点集合。
            .collect()
    }

    // 使用 trigger 子节点自然尺寸完成一次受限测量。
    measure_children => (&self, frame: Rect, children: &[WidgetId], tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        // trigger 只占据组件顶部交互区域。
        let trigger_constraints = Constraints::new(
            // 允许子 View 使用自己的最小宽度。
            Size::zero(),
            // 上界采用 Dropdown 实际宽度与固定触发高度。
            Size::new(
                frame.w.max(0.0),
                self.visual.layout.trigger_height.min(frame.h.max(0.0)),
            ),
            // trigger 区域没有额外的确定尺寸覆盖。
            None,
        );
        // 组合契约只布局首个直接 trigger，额外子树不得被静默绘制。
        children
            // 借用直接子节点身份。
            .first()
            // 测量唯一 trigger 子树。
            .map(|id| child_from_tree_with_constraints(*id, tree, trigger_constraints))
            // 将可选测量结果物化为布局集合。
            .into_iter()
            // 返回零或一个 trigger 布局快照。
            .collect()
    }

    // 让唯一 trigger View 填充 Dropdown 的触发区域。
    layout_children => (&self, frame: Rect, children: &[LayoutChild], _tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        // 没有 trigger 时不产生伪布局。
        let Some(child) = children.first() else { return Vec::new(); };
        // trigger 与旧按钮共享 UIX 声明的稳定交互高度。
        vec![(child.id, Rect::new(
            frame.x,
            frame.y,
            frame.w.max(0.0),
            self.visual.layout.trigger_height.min(frame.h.max(0.0)),
        ))]
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown {
                pos,
                button,
                ..
            } => {
                if pos.y >= 0.0 && pos.y <= self.visual.layout.trigger_height {
                    let accepted_trigger = match self.trigger_mode {
                        TriggerMode::ContextMenu => *button == MouseButton::Right,
                        _ => *button == MouseButton::Left,
                    };
                    if !accepted_trigger {
                        return EventResult::NotHandled;
                    }
                    if self.open {
                        self.close();
                    } else {
                        self.open();
                    }
                    return EventResult::Handled;
                }
                if self.open
                    && pos.y > self.visual.layout.trigger_height
                    && *button == MouseButton::Left
                {
                    if let Some(index) = self.item_at_y(pos.y) {
                        if self.toggle_or_select(index) {
                            return EventResult::Handled;
                        }
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerUp { .. } if self.trigger_mode == TriggerMode::ContextMenu => {
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if self.trigger_mode == TriggerMode::Hover && !self.open {
                    self.open();
                }
                self.highlighted_index = self.item_at_y(pos.y);
                EventResult::Handled
            }
            SystemEvent::PointerEnter if self.trigger_mode == TriggerMode::Hover => {
                self.open();
                EventResult::Handled
            }
            SystemEvent::PointerLeave => {
                self.highlighted_index = self.selected_index;
                if self.trigger_mode == TriggerMode::Hover {
                    self.close();
                }
                EventResult::Handled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                if self.trigger_mode == TriggerMode::Focus {
                    self.open();
                }
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.close();
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => match key {
                KeyCode::Enter | KeyCode::Space => {
                    if self.open {
                        if let Some(index) = self.highlighted_index {
                            self.toggle_or_select(index);
                        }
                    } else {
                        self.open();
                    }
                    EventResult::Handled
                }
                KeyCode::Down => {
                    self.move_highlight(true);
                    EventResult::Handled
                }
                KeyCode::Up => {
                    self.move_highlight(false);
                    EventResult::Handled
                }
                KeyCode::Home if self.open => {
                    self.highlighted_index = self.next_selectable(None, true);
                    EventResult::Handled
                }
                KeyCode::End if self.open => {
                    self.highlighted_index = self.next_selectable(None, false);
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

    // focus 模式观察完整 trigger 子树而不是某个内部控件。
    on_focus_within => (&mut self, focused: bool) -> EventResult {
        // 其他触发模式不消费焦点范围通知。
        if self.trigger_mode != TriggerMode::Focus {
            // 保持默认未处理语义。
            return EventResult::NotHandled;
        }
        // 进入 trigger 子树时打开下拉层。
        if focused {
            // 复用统一进入动画与高亮初始化。
            self.open();
        } else {
            // 离开整个 Dropdown 子树后关闭下拉层。
            self.close();
        }
        // focus trigger 已消费本次范围变化。
        EventResult::Handled
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|value| SemanticEvent::change(id, value))
    }

    wants_continuous_pointer_move => (&self) -> bool { self.open }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        // 触发器与弹层同帧共享一次主题解析。
        let visual = self.visual.resolve(ctx.tokens());
        let radius = Some(Radius::uniform(visual.radius));
        let layout = &self.visual.layout;
        let typography = &self.visual.typography;

        // 兼容构造继续由 Dropdown 绘制旧字符串按钮。
        if !self.custom_trigger {
            // 旧按钮占据 UIX 声明的顶部触发区域。
            let btn_rect = Rect::new(frame.x, frame.y, frame.w, layout.trigger_height);
            // 使用主题主色绘制兼容按钮背景。
            ctx.fill_rect(btn_rect, visual.primary, radius);
            // 旧 label 只服务兼容触发器展示。
            ctx.text_center(&self.label, btn_rect, visual.white, typography.label);
            // 键盘焦点可见时绘制兼容触发器焦点环。
            if self.focused && tree.keyboard_focus_visible() {
                // 焦点环使用主题活动主色。
                ctx.stroke_rect(
                    btn_rect,
                    visual.primary_active,
                    self.visual.chrome.focus_width,
                    radius,
                );
            }
        }

        if !self.is_present() {
            return;
        }
        let visible = self.visible_items();
        let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
        let bg = fade_color(visual.elevated_background, opacity);
        let border = fade_color(visual.border, opacity);
        let text_color = fade_color(visual.text, opacity);
        let disabled_color = fade_color(visual.text_quaternary, opacity);
        let highlight = fade_color(visual.fill_tertiary, opacity);

        let menu_y = frame.y + layout.trigger_height;
        let menu_h = visible.iter().map(|row| self.row_height(row)).sum::<f32>();
        let menu_rect = Rect::new(frame.x, menu_y, frame.w, menu_h);
        let shadow = visual.shadow;
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

        let mut y = menu_y;
        for (index, row) in visible.iter().enumerate() {
            let h = self.row_height(row);
            let item_rect = Rect::new(frame.x, y, frame.w, h);
            if row.item.divider {
                let inset = layout.divider_inset.min(frame.w * 0.5);
                ctx.fill_rect(
                    Rect::new(
                        frame.x + inset,
                        y + h * 0.5,
                        (frame.w - inset * 2.0).max(0.0),
                        layout.divider_thickness,
                    ),
                    border,
                    None,
                );
            } else {
                if self.highlighted_index == Some(index) || self.selected_index == Some(index) {
                    ctx.fill_rect(item_rect, highlight, radius);
                }
                let color = if row.item.disabled { disabled_color } else { text_color };
                let mut content_x =
                    frame.x + layout.content_start + row.depth as f32 * layout.depth_indent;
                if !row.item.icon.is_empty() {
                    let icon_rect = Rect::new(content_x, y, layout.icon_slot_width, h);
                    crate::ui::widgets::icon::Icon::paint_in_frame(
                        ctx,
                        &row.item.icon,
                        icon_rect,
                        color,
                        typography.icon,
                    );
                    content_x += layout.icon_advance;
                }
                let arrow_space = if row.item.children.is_empty() {
                    0.0
                } else {
                    layout.arrow_reserve
                };
                let label_rect = Rect::new(
                    content_x,
                    y,
                    (frame.x + frame.w - content_x - arrow_space - layout.label_end_padding)
                        .max(0.0),
                    h,
                );
                ctx.draw_text_in_frame(&row.item.label, label_rect, color, typography.label);
                if !row.item.children.is_empty() {
                    crate::ui::widgets::icon::Icon::paint_in_frame(
                        ctx,
                        if self.expanded_keys.iter().any(|key| key == &row.item.key) {
                            self.visual.icons.expanded
                        } else {
                            self.visual.icons.collapsed
                        },
                        Rect::new(
                            frame.x + frame.w - layout.arrow_end_inset,
                            y,
                            layout.arrow_slot_width,
                            h,
                        ),
                        color,
                        typography.arrow,
                    );
                }
            }
            y += h;
        }
    }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        if self.is_present() {
            let menu_h = self
                .visible_items()
                .iter()
                .map(|row| self.row_height(row))
                .sum::<f32>();
            Rect::new(
                frame.x,
                frame.y,
                frame.w,
                self.visual.layout.trigger_height + menu_h,
            )
        } else {
            frame
        }
    }

    // 打开时登记浮层，保证外部点击关闭与 Esc 路由在 OverlayStack 生效。
    overlay_entry => (&self, id: crate::ui::WidgetId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        if !self.is_present() {
            return None;
        }
        // 菜单从触发区下方展开，浮层命中范围与 hit_test_frame 一致。
        let menu_h = self
            .visible_items()
            .iter()
            .map(|row| self.row_height(row))
            .sum::<f32>();
        let bounds = Rect::new(
            frame.x,
            frame.y,
            frame.w,
            self.visual.layout.trigger_height + menu_h,
        );
        Some(
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Popover)
                .bounds(bounds)
                .z_index(self.visual.chrome.overlay_z)
                // 外部点击由树的 System 私有取消端口回调 owner 关闭。
                .dismiss_on_outside(true),
        )
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        dropdown_dirty_rect(
            frame,
            self.visible_items()
                .iter()
                .map(|row| self.row_height(row))
                .sum(),
            self.visual,
        )
    }

    update_animation => (&mut self, dt: f64) -> bool {
        if !self.is_present() || self.transition.finished {
            self.transition_dirty = false;
            return false;
        }

        self.transition.update(dt);
        self.transition_dirty = true;

        if self.closing && self.transition.finished {
            self.open = false;
            self.closing = false;
        }

        self.is_present() && !self.transition.finished
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.transition_dirty {
            dropdown_dirty_rect(
                frame,
                self.visible_items()
                    .iter()
                    .map(|row| self.row_height(row))
                    .sum(),
                self.visual,
            )
        } else {
            Rect::zero()
        }
    }
}

impl Default for Dropdown {
    fn default() -> Self {
        Self::new("Menu")
    }
}

impl Dropdown {
    fn intrinsic_size(&self) -> Size {
        Size::new(self.visual.layout.width, self.visual.layout.trigger_height)
    }

    /// 创建使用文本按钮和点击触发方式的空下拉菜单。
    pub fn new(label: impl Into<String>) -> Self {
        let visual = DROPDOWN_VISUAL_REF;
        Self {
            label: label.into(),
            items: Vec::new(),
            // 兼容构造默认继续绘制字符串触发按钮。
            custom_trigger: false,
            // 兼容构造不持有自定义 trigger 子树。
            custom_trigger_view: None,
            // 叶构造尚未登记直接 trigger 子树。
            trigger_child_count: 0,
            // 兼容空数据构造没有 keyed 诊断。
            diagnostics: Vec::new(),
            expanded_keys: Vec::new(),
            open: false,
            transition: TransitionPlayer::new(presets::tooltip_enter()),
            closing: false,
            transition_dirty: false,
            focused: false,
            selected_index: None,
            selected_value: None,
            highlighted_index: None,
            trigger_mode: TriggerMode::Click,
            pending_change: RefCell::new(None),
            visual,
        }
    }

    /// 设置可转换为下拉条目的顶层菜单数据。
    ///
    /// 字符串条目会同时使用其文本作为展示标签和稳定 key。
    pub fn items<T>(mut self, items: Vec<T>) -> Self
    where
        T: Into<DropdownItem>,
    {
        self.items = items.into_iter().map(|s| s.into()).collect();
        self
    }

    /// 配置要求非空且全树唯一 key 的 UIX 拥有型选项。
    pub fn keyed_items<I>(mut self, items: I) -> Self
    where
        // UIX 表达式可提供数组、Vec 或其他拥有型迭代器。
        I: IntoIterator<Item = DropdownItem>,
    {
        // 对动态数据执行确定的首项优先身份门禁。
        let (items, diagnostics) = Self::normalize_keyed_items(items);
        // 保存通过门禁的完整选项树。
        self.items = items;
        // 暴露所有被拒绝身份的稳定诊断。
        self.diagnostics = diagnostics;
        // 返回完整 keyed Dropdown。
        self
    }

    /// 返回 keyed 数据物化期间产生的只读诊断。
    pub fn diagnostics(&self) -> &[String] {
        // 调用方可把诊断接入日志、测试或开发工具。
        &self.diagnostics
    }

    /// 让 Dropdown 成为一个完整 trigger View 子树的生命周期 owner。
    pub fn trigger_view<V: crate::ui::view::View>(mut self, trigger: V) -> Self {
        // 关闭旧字符串按钮绘制并启用组合生命周期。
        self.custom_trigger = true;
        // 构建并保存包含 handlers、样式与身份的完整 ViewNode。
        self.custom_trigger_view = Some(Rc::new(RefCell::new(Some(
            // 通过公开 View 契约构建调用方 trigger。
            crate::ui::view::View::build(trigger),
        ))));
        // 返回拥有待物化 trigger 子树的组件。
        self
    }

    /// 设置打开或关闭下拉菜单的触发方式。
    pub fn trigger(mut self, trigger: TriggerMode) -> Self {
        self.trigger_mode = trigger;
        self
    }

    /// 返回菜单是否处于接受交互的打开状态。
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// 返回菜单是否仍需呈现，包括正在执行退出动画的状态。
    pub fn is_present(&self) -> bool {
        self.open || self.closing
    }

    /// 返回当前选中条目在可见扁平条目序列中的索引。
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// 返回当前选中条目的稳定业务 key。
    pub fn current_value(&self) -> Option<&str> {
        // 兼容方法现在返回稳定 key；旧构造仍保持 key=label。
        self.selected_value.as_deref()
    }

    /// 打开菜单、启动进入动画并高亮当前或首个可选择条目。
    pub fn open(&mut self) {
        self.open = true;
        self.closing = false;
        self.highlighted_index = self
            .selected_index
            .filter(|index| self.is_selectable(*index))
            .or_else(|| self.next_selectable(None, true));
        self.transition = TransitionPlayer::new(presets::tooltip_enter());
        self.transition_dirty = true;
    }

    /// 关闭菜单并在需要时启动退出动画。
    pub fn close(&mut self) {
        if !self.is_present() {
            self.open = false;
            self.closing = false;
            self.transition_dirty = false;
            return;
        }
        self.open = false;
        self.closing = true;
        self.transition = TransitionPlayer::new(presets::tooltip_exit());
        self.transition_dirty = true;
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        // reconcile 按稳定 key 保持选择，不再依赖可能重复的 label。
        let selected_value = self.current_value().map(str::to_owned);
        // 保存运行时已经展开的 keyed 子菜单组。
        let expanded_keys = std::mem::take(&mut self.expanded_keys);
        self.label = next.label;
        self.items = next.items;
        // 同步声明期组合触发器模式。
        self.custom_trigger = next.custom_trigger;
        // 下一声明提供新的完整 trigger ViewNode 供组件树 reconcile。
        self.custom_trigger_view = next.custom_trigger_view;
        // keyed 数据诊断随最新声明替换。
        self.diagnostics = next.diagnostics;
        // 只保留刷新后仍存在且仍拥有 children 的展开组。
        self.expanded_keys = expanded_keys
            // 按既有用户展开顺序检查每个稳定 key。
            .into_iter()
            // 删除已卸载或不再是分组的 key。
            .filter(|key| Self::contains_group_key(&self.items, key))
            // 物化刷新后的展开集合。
            .collect();
        self.trigger_mode = next.trigger_mode;
        // 同步 UIX 生成的视觉表引用，不保留 Rust 视觉副本。
        self.visual = next.visual;
        self.selected_index = selected_value.as_ref().and_then(|value| {
            self.visible_items()
                .iter()
                // 使用稳定 key 查找刷新后的同一业务选项。
                .position(|row| row.item.key == *value)
        });
        // 已移除的业务 key 不再保留为幽灵选择。
        self.selected_value = self.selected_index.and(selected_value);
        self.highlighted_index = if self.open {
            self.selected_index
                .filter(|index| self.is_selectable(*index))
                .or_else(|| self.next_selectable(None, true))
        } else {
            None
        };
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Dropdown {
            label: self.label.clone(),
            // 快照保留完整 keyed 选项，便于 reconcile 与诊断观察身份。
            items: self
                // 展开树顺序必须与 selected_index 和 highlighted_index 对齐。
                .visible_items()
                // 快照不需要保留布局深度，只复制 keyed 选项。
                .into_iter()
                // 提取每一行的拥有型选项。
                .map(|row| row.item)
                // 物化可观察的当前列表顺序。
                .collect(),
            open: self.open,
            selected_index: self.selected_index,
            highlighted_index: self.highlighted_index,
        }
    }

    fn item_at_y(&self, y: f32) -> Option<usize> {
        if !self.open || y <= self.visual.layout.trigger_height {
            return None;
        }
        let mut cursor = self.visual.layout.trigger_height;
        for (index, row) in self.visible_items().iter().enumerate() {
            let height = self.row_height(row);
            if y >= cursor && y < cursor + height {
                return Some(index);
            }
            cursor += height;
        }
        None
    }

    fn move_highlight(&mut self, forward: bool) {
        if !self.open {
            self.open();
            return;
        }
        self.highlighted_index = self.next_selectable(self.highlighted_index, forward);
    }

    fn select_index(&mut self, index: usize) -> bool {
        let rows = self.visible_items();
        let Some(row) = rows.get(index) else {
            return false;
        };
        if row.item.divider || row.item.disabled || !row.item.children.is_empty() {
            return false;
        }
        // 用户选择发布稳定 key，不再把展示 label 当身份。
        let value = row.item.key.clone();
        self.selected_index = Some(index);
        self.selected_value = Some(value.clone());
        self.highlighted_index = Some(index);
        self.pending_change.replace(Some(value));
        self.close();
        true
    }

    fn toggle_or_select(&mut self, index: usize) -> bool {
        let Some(row) = self.visible_items().get(index).cloned() else {
            return false;
        };
        if row.item.divider || row.item.disabled {
            return false;
        }
        if !row.item.children.is_empty() {
            // 递归组展开状态同样以稳定 key 为身份。
            if self.expanded_keys.iter().any(|key| key == &row.item.key) {
                // 收起当前 keyed 子菜单组。
                self.expanded_keys.retain(|key| key != &row.item.key);
            } else {
                // 展开当前 keyed 子菜单组。
                self.expanded_keys.push(row.item.key);
            }
            self.highlighted_index = Some(index);
            return true;
        }
        self.select_index(index)
    }

    fn is_selectable(&self, index: usize) -> bool {
        self.visible_items()
            .get(index)
            .is_some_and(|row| !row.item.divider && !row.item.disabled)
    }

    fn next_selectable(&self, current: Option<usize>, forward: bool) -> Option<usize> {
        let rows = self.visible_items();
        if rows.is_empty() {
            return None;
        }
        let start = match (current, forward) {
            (Some(index), true) => (index + 1) % rows.len(),
            (Some(index), false) => (index + rows.len() - 1) % rows.len(),
            (None, true) => 0,
            (None, false) => rows.len() - 1,
        };
        for offset in 0..rows.len() {
            let index = if forward {
                (start + offset) % rows.len()
            } else {
                (start + rows.len() - offset) % rows.len()
            };
            if !rows[index].item.divider && !rows[index].item.disabled {
                return Some(index);
            }
        }
        None
    }

    fn visible_items(&self) -> Vec<VisibleDropdownItem> {
        fn visit(
            items: &[DropdownItem],
            expanded_keys: &[String],
            depth: usize,
            rows: &mut Vec<VisibleDropdownItem>,
        ) {
            for item in items {
                // 递归可见性由稳定 key 驱动，同名 label 不再冲突。
                let expanded = expanded_keys.iter().any(|key| key == &item.key);
                let children = item.children.clone();
                rows.push(VisibleDropdownItem {
                    item: item.clone(),
                    depth,
                });
                if expanded && !children.is_empty() {
                    visit(&children, expanded_keys, depth + 1, rows);
                }
            }
        }

        let mut rows = Vec::new();
        visit(&self.items, &self.expanded_keys, 0, &mut rows);
        rows
    }

    fn row_height(&self, row: &VisibleDropdownItem) -> f32 {
        if row.item.divider {
            self.visual.layout.divider_row_height
        } else {
            self.visual.layout.row_height
        }
    }
}

fn dropdown_dirty_rect(frame: Rect, menu_h: f32, visual: &DropdownVisual) -> Rect {
    let menu = Rect::new(
        frame.x,
        frame.y + visual.layout.trigger_height,
        frame.w,
        menu_h,
    );
    let expanded = frame.union(&menu);
    let expand = visual.chrome.shadow_expand;
    Rect::new(
        expanded.x - expand,
        expanded.y - expand,
        expanded.w + expand * 2.0,
        expanded.h + expand * 2.0,
    )
}

fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}

// 将 keyed 选择、触发方式与组合子树生命周期测试拆到独立文件。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/navigation/dropdown/tests.rs"]
mod tests;
// 把 keyed 数据身份门禁从组件事件和绘制主体中拆分。
mod keyed;

// UIX 只注入静态视觉表，Rust 内核继续拥有数据、状态、生命周期与事件。
fn build_dropdown_view(mut kernel: Dropdown, visual: &'static DropdownVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Dropdown {
    fn build(self) -> ViewNode {
        build_dropdown_view(self, DROPDOWN_VISUAL_REF)
    }
}
