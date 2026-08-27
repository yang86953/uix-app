//! Tabs widget — Ant Design style tab bar with content panels.

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Radius;
use crate::ui::reactive::state::State;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent, View, ViewNode,
    WidgetId, WidgetTree,
};
use crate::widget;
use std::cell::RefCell;
use std::fmt::Display;
use std::rc::Rc;

mod presentation;
use presentation::*;

/// A single tab definition.
#[derive(Debug, Clone, PartialEq)]
pub struct Tab {
    /// 标签栏显示的文字。
    pub label: String,
    /// 标识面板身份、语义事件和快照的稳定键。
    pub key: String,
    /// 显示在标签文字前方的可选 Lucide 图标名称。
    pub icon: String,
}

impl Tab {
    /// 使用文字创建标签，并默认以相同文字作为稳定键。
    pub fn new(label: impl Into<String>) -> Self {
        let label = label.into();
        Self {
            key: label.clone(),
            label,
            icon: String::new(),
        }
    }

    /// 设置显示在标签文字前方的 Lucide 图标名称。
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = icon.into();
        self
    }

    /// 覆盖稳定 key；展示 label 不参与面板身份。
    pub fn key(mut self, key: impl Into<String>) -> Self {
        // 保存调用方提供的稳定业务 key。
        self.key = key.into();
        // 返回完成配置的标签元数据。
        self
    }
}

/// Tab bar position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabPosition {
    /// 将标签栏放在内容面板上方。
    Top,
    /// 将标签栏放在内容面板下方。
    Bottom,
    /// 将标签栏垂直放在内容面板左侧。
    Left,
    /// 将标签栏垂直放在内容面板右侧。
    Right,
}

impl TabPosition {
    fn is_vertical(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

trait TabsValueBinding {
    fn selected_index(&self) -> Option<usize>;
    fn select_index(&self, index: usize);
}

struct StateTabsValueBinding<T> {
    state: State<T>,
    values: Vec<T>,
}

impl<T> TabsValueBinding for StateTabsValueBinding<T>
where
    T: Clone + PartialEq + Send + Sync + 'static,
{
    fn selected_index(&self) -> Option<usize> {
        let selected = self.state.get();
        self.values.iter().position(|value| value == &selected)
    }

    fn select_index(&self, index: usize) {
        let Some(value) = self.values.get(index) else {
            return;
        };
        if self.state.get() != *value {
            self.state.set(value.clone());
        }
    }
}

widget! {
    /// Tabs widget with a tab bar and content switching.
    pub struct Tabs {
        tabs: Vec<Tab>,
        active_index: usize,
        value_binding: Option<Rc<dyn TabsValueBinding>>,
        position: TabPosition,
        tab_height: f32,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        focused: bool,
        /// 活动面板切换后请求组件树重新同步子可见性与布局。
        layout_requested: std::cell::Cell<bool>,
        pending_change: RefCell<Option<String>>,
        editable: bool,
        scrollable: bool,
        add_callback: Option<Rc<dyn Fn(&str)>>,
        close_callback: Option<Rc<dyn Fn(&str)>>,
        last_frame: std::cell::Cell<Option<Rect>>,
        /// 每帧 render 时计算的各 tab 主轴区间，坐标相对 tab bar 且不含滚动偏移。
        tab_main_ranges: RefCell<Vec<(f32, f32)>>,
        tab_content_extent: std::cell::Cell<f32>,
        tab_scroll_offset: std::cell::Cell<f32>,
        // 同目录 UIX 生成的唯一静态视觉表。
        #[snapshot(skip)]
        visual: &'static TabsVisual,
    }

    tab_index => (&self) -> i32 { 1 }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        self.sync_bound_value();
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let clicked = self.tab_index_at_point(*pos);
                if let Some(index) = clicked {
                    if self.editable {
                        let close = self.close_rect_for(index);
                        if close.contains(*pos) {
                            let key = self.tabs[index].key.clone();
                            if let Some(callback) = self.close_callback.as_ref() {
                                callback(&key);
                            }
                            self.tabs.remove(index);
                            self.active_index = self.active_index.min(self.tabs.len().saturating_sub(1));
                            self.tab_main_ranges.borrow_mut().clear();
                            return EventResult::Handled;
                        }
                    }
                    self.select_index(index);
                    return EventResult::Handled;
                }
                if self.editable && self.add_rect().contains(*pos) {
                    if let Some(callback) = self.add_callback.as_ref() {
                        callback("");
                    }
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }
            SystemEvent::Wheel { pos, delta }
                if self.scrollable && self.local_tab_bar_rect().contains(*pos) =>
            {
                let wheel_delta = if self.position.is_vertical() {
                    delta.y
                } else if delta.x.abs() > 0.01 {
                    delta.x
                } else {
                    delta.y
                };
                if self.scroll_by(wheel_delta * self.visual.layout.wheel_step) {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } if !self.tabs.is_empty() => match key {
                KeyCode::Right | KeyCode::Down => {
                    let next = if self.active_index < self.tabs.len() {
                        (self.active_index + 1) % self.tabs.len()
                    } else {
                        0
                    };
                    self.select_index(next);
                    EventResult::Handled
                }
                KeyCode::Left | KeyCode::Up => {
                    let previous = if self.active_index < self.tabs.len() {
                        (self.active_index + self.tabs.len() - 1) % self.tabs.len()
                    } else {
                        self.tabs.len() - 1
                    };
                    self.select_index(previous);
                    EventResult::Handled
                }
                KeyCode::Home => {
                    self.select_index(0);
                    EventResult::Handled
                }
                KeyCode::End => {
                    self.select_index(self.tabs.len() - 1);
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
            .map(|key| SemanticEvent::change(id, key))
    }

    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        self.capture_bound_value_dependency();
        let frame = Self::normalized_frame(frame);
        self.last_frame.set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        // 标签栏、编辑入口、内容区与焦点同帧共享一次主题解析。
        let visual = self.visual.resolve(ctx.tokens());
        let layout = &self.visual.layout;
        let typography = &self.visual.typography;
        let bg_container = visual.container_background;
        let border_secondary = visual.border_secondary;
        let primary = visual.primary;
        let text_secondary = visual.text_secondary;

        let vertical = self.position.is_vertical();
        let tab_bar = self.tab_bar_rect(frame);
        let content_extent = self.rebuild_tab_main_ranges(|label| {
            ctx.measure_text(label, visual.label_font_size).w
        });
        self.tab_content_extent.set(content_extent);
        let viewport_extent = if vertical { tab_bar.h } else { tab_bar.w };
        let offset = if self.scrollable {
            self.tab_scroll_offset
                .get()
                .clamp(0.0, (content_extent - viewport_extent).max(0.0))
        } else {
            0.0
        };
        self.tab_scroll_offset.set(offset);

        ctx.fill_rect(tab_bar, bg_container, None);
        let divider = match self.position {
            TabPosition::Top => Rect::new(
                tab_bar.x,
                tab_bar.y + tab_bar.h - layout.divider_thickness,
                tab_bar.w,
                layout.divider_thickness,
            ),
            TabPosition::Bottom => Rect::new(
                tab_bar.x,
                tab_bar.y,
                tab_bar.w,
                layout.divider_thickness,
            ),
            TabPosition::Left => Rect::new(
                tab_bar.x + tab_bar.w - layout.divider_thickness,
                tab_bar.y,
                layout.divider_thickness,
                tab_bar.h,
            ),
            TabPosition::Right => Rect::new(
                tab_bar.x,
                tab_bar.y,
                layout.divider_thickness,
                tab_bar.h,
            ),
        };
        ctx.fill_rect(divider, border_secondary, None);

        ctx.push_clip(tab_bar);
        let ranges = self.tab_main_ranges.borrow();
        for (i, tab) in self.tabs.iter().enumerate() {
            let (start, end) = ranges[i];
            let tab_rect = if vertical {
                Rect::new(tab_bar.x, tab_bar.y + start - offset, tab_bar.w, end - start)
            } else {
                Rect::new(tab_bar.x + start - offset, tab_bar.y, end - start, tab_bar.h)
            };
            let is_active = i == self.active_index;
            let text_color = if is_active { primary } else { text_secondary };
            // 标签字号：统一使用主题 font_size token。
            let tab_text_y = ctx.visual_center_y(tab_rect, visual.label_font_size);
            let mut text_x = tab_rect.x + layout.horizontal_padding;
            if !tab.icon.is_empty() {
                crate::ui::widgets::icon::Icon::paint_in_frame(
                    ctx,
                    &tab.icon,
                    Rect::new(
                        tab_rect.x + layout.horizontal_padding,
                        tab_rect.y,
                        layout.icon_slot_width,
                        tab_rect.h,
                    ),
                    text_color,
                    typography.tab_icon,
                );
                text_x += layout.icon_advance;
            }
            ctx.draw_text(
                &tab.label,
                Point::new(text_x, tab_text_y),
                text_color,
                // 标签字号：统一使用主题 font_size token。
                visual.label_font_size,
            );

            if self.editable {
                let close = Rect::new(
                    (tab_rect.x + tab_rect.w - layout.close_size).max(tab_rect.x),
                    tab_rect.y,
                    layout.close_size.min(tab_rect.w),
                    tab_rect.h,
                );
                crate::ui::widgets::icon::Icon::paint_in_frame(
                    ctx,
                    self.visual.icons.close,
                    close,
                    text_secondary,
                    typography.close_icon,
                );
            }

            if is_active {
                let indicator = match self.position {
                    TabPosition::Top => {
                        let w = tab_rect.w * layout.indicator_extent_factor;
                        Rect::new(
                            tab_rect.x + (tab_rect.w - w) * 0.5,
                            tab_rect.y + tab_rect.h - layout.indicator_thickness,
                            w,
                            layout.indicator_thickness,
                        )
                    }
                    TabPosition::Bottom => {
                        let w = tab_rect.w * layout.indicator_extent_factor;
                        Rect::new(
                            tab_rect.x + (tab_rect.w - w) * 0.5,
                            tab_rect.y,
                            w,
                            layout.indicator_thickness,
                        )
                    }
                    TabPosition::Left => {
                        let h = tab_rect.h * layout.indicator_extent_factor;
                        Rect::new(
                            tab_rect.x + tab_rect.w - layout.indicator_thickness,
                            tab_rect.y + (tab_rect.h - h) * 0.5,
                            layout.indicator_thickness,
                            h,
                        )
                    }
                    TabPosition::Right => {
                        let h = tab_rect.h * layout.indicator_extent_factor;
                        Rect::new(
                            tab_rect.x,
                            tab_rect.y + (tab_rect.h - h) * 0.5,
                            layout.indicator_thickness,
                            h,
                        )
                    }
                };
                ctx.fill_rect(
                    indicator,
                    primary,
                    Some(Radius::uniform(self.visual.chrome.indicator_radius)),
                );
            }
        }
        drop(ranges);
        if self.editable {
            let add = Self::absolute_rect(frame, self.add_rect());
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                self.visual.icons.add,
                add,
                primary,
                // 添加按钮图标字号：统一使用主题 font_size token。
                visual.label_font_size,
            );
        }
        ctx.pop_clip();

        ctx.fill_rect(self.content_rect(frame), bg_container, None);
        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(
                tab_bar,
                primary,
                self.visual.chrome.focus_width,
                Some(Radius::uniform(visual.radius)),
            );
        }
    }

    measure_children_into => (
        &self,
        _frame: Rect,
        children: &[WidgetId],
        _tree: &WidgetTree,
        output: &mut Vec<crate::ui::LayoutChild>
    ) {
        output.clear();
        output.reserve(children.len());
        output.extend(
            children
                .iter()
                .copied()
                .map(|id| crate::ui::LayoutChild::new(id, Size::zero())),
        );
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::WidgetId, Rect)>
    {
        let mut output = Vec::new();
        self.layout_active_child_into(frame, children, &mut output);
        output
    }

    layout_children_into => (
        &self,
        frame: Rect,
        children: &[crate::ui::LayoutChild],
        _tree: &WidgetTree,
        _scratch: &mut crate::ui::LayoutEngineScratch,
        output: &mut Vec<(crate::ui::WidgetId, Rect)>
    ) {
        self.layout_active_child_into(frame, children, output);
    }

    child_visible => (&self, index: usize) -> bool {
        index == self.active_index && index < self.tabs.len()
    }
}

impl Tabs {
    // 把活动面板位置写入布局树拥有的跨帧数组。
    fn layout_active_child_into(
        &self,
        frame: Rect,
        children: &[crate::ui::LayoutChild],
        output: &mut Vec<(crate::ui::WidgetId, Rect)>,
    ) {
        output.clear();
        if self.active_index >= self.tabs.len() {
            return;
        }
        let Some(child) = children.first() else {
            return;
        };
        let content = self.content_rect(Self::normalized_frame(frame));
        let horizontal_inset = self
            .visual
            .layout
            .content_horizontal_inset
            .min(content.w * 0.5);
        let vertical_inset = self
            .visual
            .layout
            .content_vertical_inset
            .min(content.h * 0.5);
        output.reserve(1);
        output.push((
            child.id,
            Rect::new(
                content.x + horizontal_inset,
                content.y + vertical_inset,
                (content.w - horizontal_inset * 2.0).max(0.0),
                (content.h - vertical_inset * 2.0).max(0.0),
            ),
        ));
    }
}

impl Default for Tabs {
    fn default() -> Self {
        Self::new()
    }
}

impl Tabs {
    // 重建标签主轴区间并复用组件已有缓存，避免稳态渲染重新申请数组。
    fn rebuild_tab_main_ranges(&self, mut measure_label_width: impl FnMut(&str) -> f32) -> f32 {
        let vertical = self.position.is_vertical();
        let layout = &self.visual.layout;
        let mut ranges = self.tab_main_ranges.borrow_mut();
        ranges.clear();
        ranges.reserve(self.tabs.len());
        let mut cursor = if vertical {
            0.0
        } else {
            layout.horizontal_padding
        };
        for tab in &self.tabs {
            let icon_width = if tab.icon.is_empty() {
                0.0
            } else {
                layout.icon_reserve
            };
            let close_width = if self.editable {
                layout.close_reserve
            } else {
                0.0
            };
            let extent = if vertical {
                self.tab_height
            } else {
                measure_label_width(&tab.label)
                    + layout.horizontal_padding * 2.0
                    + icon_width
                    + close_width
            };
            ranges.push((cursor, cursor + extent));
            cursor += extent + if vertical { 0.0 } else { layout.gap };
        }
        let mut content_extent = ranges.last().map_or(0.0, |(_, end)| *end);
        if !vertical && !ranges.is_empty() {
            content_extent += layout.horizontal_padding;
        }
        if self.editable {
            content_extent += if vertical {
                self.tab_height
            } else {
                layout.gap + layout.add_size
            };
        }
        content_extent
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(
            self.fixed_width.unwrap_or(self.visual.layout.default_width),
            self.fixed_height
                .unwrap_or(self.visual.layout.default_height),
        )
    }

    /// 创建顶部标签栏、无标签且未启用编辑或滚动的组件。
    pub fn new() -> Self {
        let visual = TABS_VISUAL_REF;
        Self {
            tabs: Vec::new(),
            active_index: 0,
            value_binding: None,
            position: visual.defaults.position,
            tab_height: visual.layout.tab_height,
            fixed_width: None,
            fixed_height: None,
            focused: false,
            // 新组件尚无待处理的布局请求。
            layout_requested: std::cell::Cell::new(false),
            pending_change: RefCell::new(None),
            last_frame: std::cell::Cell::new(None),
            tab_main_ranges: RefCell::new(Vec::new()),
            tab_content_extent: std::cell::Cell::new(0.0),
            tab_scroll_offset: std::cell::Cell::new(0.0),
            editable: visual.defaults.editable,
            scrollable: visual.defaults.scrollable,
            add_callback: None,
            close_callback: None,
            visual,
        }
    }

    /// 添加标签并切换为非受控模式。
    pub fn tab(mut self, label: &str, key: &str) -> Self {
        self.value_binding = None;
        self.tabs.push(Tab {
            label: label.to_string(),
            key: key.to_string(),
            icon: String::new(),
        });
        self
    }
    /// 替换全部标签并切换为非受控模式。
    pub fn tabs(mut self, tabs: Vec<Tab>) -> Self {
        self.value_binding = None;
        self.tabs = tabs;
        self
    }
    /// 设置非受控活动索引，并夹取到当前标签范围。
    pub fn active(mut self, index: usize) -> Self {
        self.value_binding = None;
        self.active_index = index.min(self.tabs.len().saturating_sub(1));
        self
    }
    /// 返回当前活动标签的零基索引。
    pub fn active_index(&self) -> usize {
        self.active_index
    }
    /// 返回当前活动标签的稳定键；没有有效标签时返回空值。
    pub fn current_key(&self) -> Option<&str> {
        self.tabs.get(self.active_index).map(|tab| tab.key.as_str())
    }

    /// 将字符串 tab key 双向绑定到外部 State；应在 `tab` / `tabs` 之后调用。
    pub fn active_key(mut self, state: &State<String>) -> Self {
        let values = self.tabs.iter().map(|tab| tab.key.clone()).collect();
        self.bind_values(state, values);
        self
    }

    /// 以同一种 typed key 构造受控 Tabs；`Display` 文本作为语义事件和快照 key。
    pub fn controlled<K, I, L>(tabs: I, state: &State<K>) -> Self
    where
        K: Clone + PartialEq + Display + Send + Sync + 'static,
        I: IntoIterator<Item = (L, K)>,
        L: Into<String>,
    {
        let mut widget = Self::new();
        let mut values = Vec::new();
        for (label, key) in tabs {
            widget.tabs.push(Tab {
                label: label.into(),
                key: key.to_string(),
                icon: String::new(),
            });
            values.push(key);
        }
        widget.bind_values(state, values);
        widget
    }
    /// 设置标签栏相对内容面板的位置。
    pub fn position(mut self, pos: TabPosition) -> Self {
        self.position = pos;
        self
    }

    /// [`Self::position`] 的兼容别名。
    pub fn tab_position(self, pos: TabPosition) -> Self {
        self.position(pos)
    }

    /// 创建已启用新增和关闭入口的空标签组件。
    pub fn editable() -> Self {
        Self::new().editable_mode()
    }

    /// 为当前标签组件启用新增和关闭入口。
    pub fn editable_mode(mut self) -> Self {
        self.editable = true;
        self
    }

    /// 注册用户点击新增入口时调用的候选键回调。
    pub fn on_add<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) + 'static,
    {
        self.add_callback = Some(Rc::new(callback));
        self
    }

    /// 注册标签关闭前接收其稳定键的回调。
    pub fn on_close<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) + 'static,
    {
        self.close_callback = Some(Rc::new(callback));
        self
    }

    /// 设置标签栏溢出时是否响应滚轮滚动。
    pub fn scrollable(mut self, scrollable: bool) -> Self {
        self.scrollable = scrollable;
        self
    }
    /// 设置标签组件请求的固定宽度与高度。
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.fixed_width = Some(w);
        self.fixed_height = Some(h);
        self
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        // 记录同步前的活动索引，用于判断是否需要重新布局。
        let previous_active_index = self.active_index;
        let previous_key = self.current_key().map(str::to_owned);
        let previous_position = self.position;
        let controlled_index = next
            .value_binding
            .as_ref()
            .map(|binding| binding.selected_index().unwrap_or(usize::MAX));
        self.tabs = next.tabs;
        self.value_binding = next.value_binding;
        self.active_index = controlled_index.unwrap_or_else(|| {
            previous_key
                .as_deref()
                .and_then(|key| self.tabs.iter().position(|tab| tab.key == key))
                .unwrap_or_else(|| self.active_index.min(self.tabs.len().saturating_sub(1)))
        });
        self.position = next.position;
        self.tab_height = next.tab_height;
        self.fixed_width = next.fixed_width;
        self.fixed_height = next.fixed_height;
        self.editable = next.editable;
        self.scrollable = next.scrollable;
        self.add_callback = next.add_callback;
        self.close_callback = next.close_callback;
        // 同步 UIX 生成的视觉表引用，不保留 Rust 视觉副本。
        self.visual = next.visual;
        if !self.scrollable || previous_position.is_vertical() != self.position.is_vertical() {
            self.tab_scroll_offset.set(0.0);
        }
        self.tab_main_ranges.borrow_mut().clear();
        self.tab_content_extent.set(0.0);
        // 活动面板变化时请求组件树更新可见性门控与布局。
        if self.active_index != previous_active_index {
            // 合并尚未消费的布局请求，避免覆盖事件阶段的请求。
            self.layout_requested.set(true);
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Tabs {
            tabs: self.tabs.clone(),
            active_index: self.active_index,
            active_key: self.current_key().map(str::to_owned),
            position: self.position,
            tab_height: self.tab_height,
            fixed_width: self.fixed_width,
            fixed_height: self.fixed_height,
        }
    }

    fn select_index(&mut self, index: usize) {
        let Some(key) = self.tabs.get(index).map(|tab| tab.key.clone()) else {
            return;
        };
        if index != self.active_index {
            self.active_index = index;
            // 活动面板切换后请求重新同步子可见性与布局。
            self.layout_requested.set(true);
            if let Some(binding) = self.value_binding.as_ref() {
                binding.select_index(index);
            }
            self.pending_change.replace(Some(key));
        }
        self.ensure_tab_visible(index);
    }

    fn bind_values<T>(&mut self, state: &State<T>, values: Vec<T>)
    where
        T: Clone + PartialEq + Send + Sync + 'static,
    {
        let binding: Rc<dyn TabsValueBinding> = Rc::new(StateTabsValueBinding {
            state: state.clone(),
            values,
        });
        self.active_index = binding.selected_index().unwrap_or(usize::MAX);
        self.value_binding = Some(binding);
    }

    fn sync_bound_value(&mut self) {
        // 记录受控值同步前的活动索引。
        let previous_active_index = self.active_index;
        let Some(index) = self
            .value_binding
            .as_ref()
            .and_then(|binding| binding.selected_index())
        else {
            if self.value_binding.is_some() {
                self.active_index = usize::MAX;
                // 外部 key 不匹配时清空面板并请求重新布局。
                if self.active_index != previous_active_index {
                    // 标记组件树需要重新同步可见性门控。
                    self.layout_requested.set(true);
                }
            }
            return;
        };
        self.active_index = index;
        // 受控 key 切换面板时请求重新布局。
        if self.active_index != previous_active_index {
            // 标记组件树需要重新同步可见性门控。
            self.layout_requested.set(true);
        }
        self.ensure_tab_visible(index);
    }

    fn capture_bound_value_dependency(&self) {
        if let Some(binding) = self.value_binding.as_ref() {
            let _ = binding.selected_index();
        }
    }

    fn close_rect_for(&self, index: usize) -> Rect {
        let Some(tab) = self.local_tab_rect(index) else {
            return Rect::zero();
        };
        Rect::new(
            (tab.x + tab.w - self.visual.layout.close_size).max(tab.x),
            tab.y,
            self.visual.layout.close_size.min(tab.w),
            tab.h,
        )
    }

    fn add_rect(&self) -> Rect {
        let bar = self.local_tab_bar_rect();
        let end = self
            .tab_main_ranges
            .borrow()
            .last()
            .map(|(_, end)| *end)
            .unwrap_or(0.0);
        if self.position.is_vertical() {
            Rect::new(
                bar.x,
                bar.y + end - self.tab_scroll_offset.get(),
                bar.w,
                self.tab_height,
            )
        } else {
            Rect::new(
                bar.x + end + self.visual.layout.gap - self.tab_scroll_offset.get(),
                bar.y,
                self.visual.layout.add_size,
                bar.h,
            )
        }
    }

    fn tab_index_at_point(&self, point: Point) -> Option<usize> {
        let bar = self.local_tab_bar_rect();
        if !bar.contains(point) {
            return None;
        }
        let main = if self.position.is_vertical() {
            point.y - bar.y
        } else {
            point.x - bar.x
        } + self.tab_scroll_offset.get();
        self.tab_main_ranges
            .borrow()
            .iter()
            .position(|&(start, end)| main >= start && main < end)
    }

    fn local_tab_rect(&self, index: usize) -> Option<Rect> {
        let (start, end) = self.tab_main_ranges.borrow().get(index).copied()?;
        let bar = self.local_tab_bar_rect();
        let offset = self.tab_scroll_offset.get();
        Some(if self.position.is_vertical() {
            Rect::new(bar.x, bar.y + start - offset, bar.w, end - start)
        } else {
            Rect::new(bar.x + start - offset, bar.y, end - start, bar.h)
        })
    }

    fn ensure_tab_visible(&self, index: usize) {
        if !self.scrollable {
            return;
        }
        let Some((start, end)) = self.tab_main_ranges.borrow().get(index).copied() else {
            return;
        };
        let viewport = self.tab_bar_main_extent();
        let old = self.tab_scroll_offset.get();
        let next = if start < old {
            start
        } else if end > old + viewport {
            end - viewport
        } else {
            old
        };
        self.tab_scroll_offset
            .set(next.clamp(0.0, self.max_scroll_offset()));
    }

    fn scroll_by(&self, delta: f32) -> bool {
        if !delta.is_finite() || delta.abs() <= 0.01 {
            return false;
        }
        let old = self.tab_scroll_offset.get();
        let next = (old + delta).clamp(0.0, self.max_scroll_offset());
        if (next - old).abs() <= 0.01 {
            return false;
        }
        self.tab_scroll_offset.set(next);
        true
    }

    fn max_scroll_offset(&self) -> f32 {
        (self.tab_content_extent.get() - self.tab_bar_main_extent()).max(0.0)
    }

    fn tab_bar_main_extent(&self) -> f32 {
        let bar = self.local_tab_bar_rect();
        if self.position.is_vertical() {
            bar.h
        } else {
            bar.w
        }
    }

    fn local_tab_bar_rect(&self) -> Rect {
        self.tab_bar_rect(self.last_frame.get().unwrap_or_else(|| {
            let size = self.intrinsic_size();
            Rect::new(0.0, 0.0, size.w, size.h)
        }))
    }

    fn tab_bar_rect(&self, frame: Rect) -> Rect {
        let tab_height = self.tab_height.min(frame.h).max(0.0);
        let side_width = self.visual.layout.side_bar_width.min(frame.w).max(0.0);
        match self.position {
            TabPosition::Top => Rect::new(frame.x, frame.y, frame.w, tab_height),
            TabPosition::Bottom => {
                Rect::new(frame.x, frame.y + frame.h - tab_height, frame.w, tab_height)
            }
            TabPosition::Left => Rect::new(frame.x, frame.y, side_width, frame.h),
            TabPosition::Right => {
                Rect::new(frame.x + frame.w - side_width, frame.y, side_width, frame.h)
            }
        }
    }

    fn content_rect(&self, frame: Rect) -> Rect {
        let bar = self.tab_bar_rect(frame);
        match self.position {
            TabPosition::Top => {
                Rect::new(frame.x, bar.y + bar.h, frame.w, (frame.h - bar.h).max(0.0))
            }
            TabPosition::Bottom => Rect::new(frame.x, frame.y, frame.w, (frame.h - bar.h).max(0.0)),
            TabPosition::Left => {
                Rect::new(bar.x + bar.w, frame.y, (frame.w - bar.w).max(0.0), frame.h)
            }
            TabPosition::Right => Rect::new(frame.x, frame.y, (frame.w - bar.w).max(0.0), frame.h),
        }
    }

    fn normalized_frame(frame: Rect) -> Rect {
        Rect::new(
            frame.x,
            frame.y,
            if frame.w.is_finite() {
                frame.w.max(0.0)
            } else {
                0.0
            },
            if frame.h.is_finite() {
                frame.h.max(0.0)
            } else {
                0.0
            },
        )
    }

    fn absolute_rect(frame: Rect, local: Rect) -> Rect {
        Rect::new(frame.x + local.x, frame.y + local.y, local.w, local.h)
    }

    // 测试目标保留标签滚动偏移观测入口，供导航交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn tab_scroll_offset(&self) -> f32 {
        self.tab_scroll_offset.get()
    }

    // 测试目标保留标签区域观测入口，供导航布局测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn tab_rect_for_test(&self, index: usize) -> Option<Rect> {
        self.local_tab_rect(index)
    }

    // 测试目标保留新增标签按钮区域观测入口，供导航交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn add_rect_for_test(&self) -> Rect {
        self.add_rect()
    }
}

// UIX 只注入静态视觉表，Rust 内核继续拥有稳定 key、值绑定、滚动与子可见性。
fn build_tabs_view(mut kernel: Tabs, visual: &'static TabsVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Tabs {
    fn build(self) -> ViewNode {
        build_tabs_view(self, TABS_VISUAL_REF)
    }
}
