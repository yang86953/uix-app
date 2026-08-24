//! Selectable list widget.
use std::cell::Cell;

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::{Color, Radius};
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::widget;
// 引入稳定条目 id 的受控状态句柄。
use crate::ui::reactive::state::State;
use crate::ui::virtualization::virtual_scroll::VirtualListScroll;
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent, ThemeTokens,
    WidgetId, WidgetTree,
};

// 可选择条目的无状态构造方法由同名子模块维护。
mod item;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectableListAction {
    Header,
    Row(usize),
}

// 保存由 UIX 声明的固有尺寸、头尾区域与行布局。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SelectableListGeometryVisual {
    default_width: f32,
    default_height: f32,
    min_height: f32,
    default_item_height: f32,
    min_item_height: f32,
    header_height: f32,
    header_inset: f32,
    header_button_height: f32,
    footer_height: f32,
    row_gap: f32,
    row_horizontal_inset: f32,
}

// 保存由 UIX 声明的按钮、行、图标、活动条和焦点框视觉。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SelectableListChromeVisual {
    header_pressed_alpha: u8,
    row_pressed_alpha: u8,
    row_active_alpha: u8,
    row_icon_slot: f32,
    row_icon_size: f32,
    row_text_indent: f32,
    row_icon_text_gap: f32,
    row_text_right_pad: f32,
    row_radius: f32,
    active_bar_width: f32,
    active_bar_radius: f32,
    active_bar_vertical_inset: f32,
    row_font_size: f32,
    header_icon_inset: f32,
    header_icon_frame_ratio: f32,
    center_ratio: f32,
    header_text_gap: f32,
    header_text_right_pad: f32,
    separator_y_offset: f32,
    separator_height: f32,
    frame_border_width: f32,
    footer_inset: f32,
    footer_inset_ratio: f32,
    footer_font_size: f32,
    focus_inset: f32,
    focus_stroke_width: f32,
    plus_icon: &'static str,
}

// SelectableList 使用的主题字号角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectableListFontRole {
    Body,
}

impl SelectableListFontRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Body => tokens.font_size(),
        }
    }
}

// SelectableList 焦点框使用的主题圆角角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectableListRadiusRole {
    Small,
}

impl SelectableListRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

// 保存由 UIX 声明的主题语义角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SelectableListPaletteVisual {
    background: ColorValue,
    border: ColorValue,
    fill_secondary: ColorValue,
    fill_hover: ColorValue,
    text_secondary: ColorValue,
    text_tertiary: ColorValue,
    primary: ColorValue,
    icon_font: SelectableListFontRole,
    focus_radius: SelectableListRadiusRole,
}

// 完整视觉配置由全部 SelectableList 实例共享。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SelectableListVisual {
    geometry: SelectableListGeometryVisual,
    chrome: SelectableListChromeVisual,
    palette: SelectableListPaletteVisual,
}

// 同目录 UIX 生成几何、装饰、色板与根视觉记录及稳定借用。
crate::uix_items!("src/ui/widgets/display/selectable_list/selectable_list.uix");

// 保存每帧一次性解析的主题颜色、字号与圆角。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ResolvedSelectableListVisual {
    background: Color,
    border: Color,
    fill_secondary: Color,
    fill_hover: Color,
    text_secondary: Color,
    text_tertiary: Color,
    primary: Color,
    icon_font_size: f32,
    focus_radius: f32,
}

impl SelectableListVisual {
    fn resolve(self, tokens: &dyn ThemeTokens) -> ResolvedSelectableListVisual {
        ResolvedSelectableListVisual {
            background: self.palette.background.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            fill_secondary: self.palette.fill_secondary.resolve(tokens),
            fill_hover: self.palette.fill_hover.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            text_tertiary: self.palette.text_tertiary.resolve(tokens),
            primary: self.palette.primary.resolve(tokens),
            icon_font_size: self.palette.icon_font.resolve(tokens),
            focus_radius: self.palette.focus_radius.resolve(tokens),
        }
    }
}

// 向 UIX 提供受限表达式不能直接书写的主题角色。
const fn selectable_list_body_font() -> SelectableListFontRole {
    SelectableListFontRole::Body
}
const fn selectable_list_small_radius() -> SelectableListRadiusRole {
    SelectableListRadiusRole::Small
}
const fn selectable_list_background_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
const fn selectable_list_border_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BorderSecondary)
}
const fn selectable_list_fill_secondary_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillSecondary)
}
const fn selectable_list_fill_hover_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Fill)
}
const fn selectable_list_secondary_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
const fn selectable_list_tertiary_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextTertiary)
}
const fn selectable_list_primary_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}

#[derive(Debug, Clone, Copy)]
struct SelectableListGeometry {
    frame: Rect,
    header_button: Option<Rect>,
    body: Rect,
    footer: Option<Rect>,
}

#[derive(Debug, Clone, PartialEq)]
/// 可选择列表中的稳定业务条目。
pub struct SelectableItem {
    /// 条目用于选择状态与变更事件的稳定业务标识。
    pub id: String,
    /// 条目向用户显示的文字。
    pub text: String,
    /// 条目可选的图标名称。
    pub icon: Option<String>,
}

impl Default for SelectableList {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectableList {
    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::SelectableList {
            items: self.items.clone(),
            active_index: self.active_index,
            header_button_text: self.header_button_text.clone(),
            footer_text: self.footer_text.clone(),
            item_height: self.item_height,
        }
    }

    pub(crate) fn item_stride(&self) -> f32 {
        self.item_height + self.visual.geometry.row_gap
    }

    pub(crate) fn list_body_viewport_height(&self) -> f32 {
        let frame = self.last_frame.get().unwrap_or_else(|| {
            Rect::new(
                0.0,
                0.0,
                self.visual.geometry.default_width,
                self.visual.geometry.default_height,
            )
        });
        self.geometry(frame).body.h
    }

    // 测试目标保留行命中索引观测入口，供列表交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn row_index_at_y(&self, pos_y: f32) -> Option<usize> {
        let frame = self.last_frame.get().unwrap_or_else(|| {
            Rect::new(
                0.0,
                0.0,
                self.visual.geometry.default_width,
                self.visual.geometry.default_height,
            )
        });
        let geometry = self.local_geometry(frame);
        self.row_index_in_geometry(Point::new(geometry.body.x, pos_y), geometry)
    }

    fn row_index_in_geometry(
        &self,
        point: Point,
        geometry: SelectableListGeometry,
    ) -> Option<usize> {
        if !geometry.body.contains(point) {
            return None;
        }
        let local_y = point.y - geometry.body.y + self.body_scroll.scroll_offset();
        if local_y < 0.0 {
            return None;
        }
        let stride = self.item_stride();
        let row = (local_y / stride) as usize;
        let y_in_row = local_y - row as f32 * stride;
        if row < self.items.len() && y_in_row < self.item_height {
            Some(row)
        } else {
            None
        }
    }

    fn push_scroll_delta(&self, dx: f32, dy: f32) {
        if dx.abs() <= 0.01 && dy.abs() <= 0.01 {
            return;
        }
        let current = self.scroll_delta_strip.get();
        self.scroll_delta_strip
            .set((current.0 + dx, current.1 + dy));
    }

    fn action_at_point(&self, point: Point) -> Option<SelectableListAction> {
        let frame = self.last_frame.get().unwrap_or_else(|| {
            Rect::new(
                0.0,
                0.0,
                self.visual.geometry.default_width,
                self.visual.geometry.default_height,
            )
        });
        let geometry = self.local_geometry(frame);
        if geometry
            .header_button
            .is_some_and(|button| button.contains(point))
        {
            Some(SelectableListAction::Header)
        } else {
            self.row_index_in_geometry(point, geometry)
                .map(SelectableListAction::Row)
        }
    }

    fn select(&mut self, index: usize, activate_unchanged: bool) {
        if self.items.is_empty() {
            return;
        }
        let index = index.min(self.items.len() - 1);
        let changed = index != self.active_index;
        self.active_index = index;
        // 用户选择先写回外部唯一事实源，再登记语义变化事件。
        if let Some(state) = self.active_binding.as_ref() {
            // 只发布当前集合中确实存在的稳定条目 id。
            let selected = Some(self.items[index].id.clone());
            // 避免向响应式状态重复写入相同值。
            if state.get() != selected {
                // 受控状态必须在 Change 事件被消费前完成更新。
                state.set(selected);
            }
        }
        if changed || activate_unchanged {
            self.pending_action
                .set(Some(SelectableListAction::Row(index)));
        }
        if changed {
            self.ensure_active_visible();
        }
    }

    fn move_active(&mut self, forward: bool) {
        if self.items.is_empty() {
            return;
        }
        // 外部空值或失效 id 从首尾边界开始恢复键盘选择。
        let next = if self.active_index >= self.items.len() {
            // 向下从首项开始，向上从末项开始。
            if forward { 0 } else { self.items.len() - 1 }
        } else if forward {
            (self.active_index + 1).min(self.items.len() - 1)
        } else {
            self.active_index.saturating_sub(1)
        };
        self.select(next, false);
    }

    fn ensure_active_visible(&mut self) {
        let stride = self.item_stride();
        let viewport_height = self.list_body_viewport_height();
        let old_offset = self.body_scroll.scroll_offset();
        let row_top = self.active_index as f32 * stride;
        let row_bottom = row_top + stride;
        let new_offset = if row_top < old_offset {
            row_top
        } else if row_bottom > old_offset + viewport_height {
            row_bottom - viewport_height
        } else {
            old_offset
        };
        self.body_scroll.set_scroll_offset(new_offset);
        self.body_scroll
            .clamp_to_content(self.items.len(), stride, viewport_height);
        self.push_scroll_delta(0.0, self.body_scroll.scroll_offset() - old_offset);
    }

    /// 替换列表条目，并按当前受控身份或非受控索引调和选择。
    pub fn items(mut self, items: Vec<SelectableItem>) -> Self {
        self.items = items;
        // 受控模式按稳定 id 重新定位，非受控模式保留索引兼容行为。
        if self.active_binding.is_some() {
            // 数据晚于状态绑定设置时也必须采用外部事实。
            self.sync_bound_active();
        } else {
            // 旧索引在集合缩短后收敛到最后一个有效位置。
            self.active_index = self.active_index.min(self.items.len().saturating_sub(1));
        }
        self
    }

    /// 设置非受控模式下的初始活动条目索引。
    pub fn active(mut self, index: usize) -> Self {
        // 显式索引构建器保持原有非受控语义。
        self.active_binding = None;
        self.active_index = index.min(self.items.len().saturating_sub(1));
        self
    }

    /// 将当前活动条目的稳定标识双向绑定到外部可空状态。
    pub fn active_state(mut self, state: &State<Option<String>>) -> Self {
        // 克隆轻量状态句柄供交互写回与响应式依赖捕获使用。
        self.active_binding = Some(state.clone());
        // 构造时立即同步，确保首帧快照和绘制使用外部事实。
        self.sync_bound_active();
        self
    }

    /// 设置列表头部动作按钮的文字。
    pub fn header_button(mut self, text: impl Into<String>) -> Self {
        self.header_button_text = text.into();
        self
    }

    /// 设置列表底部的辅助文字。
    pub fn footer(mut self, text: impl Into<String>) -> Self {
        self.footer_text = text.into();
        self
    }

    /// 设置行高；仅有限值生效，并至少归一化为二十像素。
    pub fn row_height(mut self, height: f32) -> Self {
        if height.is_finite() {
            self.item_height = height.max(self.visual.geometry.min_item_height);
            self.item_height_authored = true;
        }
        self
    }

    /// 返回当前活动条目的稳定标识；没有有效活动项时返回 `None`。
    pub fn selected_id(&self) -> Option<&str> {
        self.items
            .get(self.active_index)
            .map(|item| item.id.as_str())
    }

    /// 返回当前活动条目的显示文字；没有有效活动项时返回 `None`。
    pub fn selected_text(&self) -> Option<&str> {
        self.items
            .get(self.active_index)
            .map(|item| item.text.as_str())
    }

    // 从外部稳定 id 同步当前内部命中索引。
    fn sync_bound_active(&mut self) {
        // 未绑定时完整保留组件内部索引状态。
        let Some(active) = self.active_binding.as_ref().map(State::get) else {
            return;
        };
        // 空值或失效 id 均显示为无活动项，且不反向归一化外部状态。
        self.active_index = active
            .as_deref()
            .and_then(|id| self.items.iter().position(|item| item.id == id))
            .unwrap_or(usize::MAX);
    }

    // 在绘制期登记外部活动状态的响应式读取依赖。
    fn capture_bound_active_dependency(&self) {
        // 仅受控模式需要触发声明视图重建。
        if let Some(state) = self.active_binding.as_ref() {
            // 读取值即可由状态系统捕获当前组件依赖。
            let _ = state.get();
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let selected_id = self.selected_id().map(str::to_owned);
        self.items = next.items;
        self.header_button_text = next.header_button_text;
        self.footer_text = next.footer_text;
        self.item_height = next.item_height;
        self.item_height_authored = next.item_height_authored;
        self.visual = next.visual;
        // 下一帧声明决定是否进入受控模式。
        self.active_binding = next.active_binding;
        // 受控状态覆盖旧内部选择，非受控模式继续按稳定 id 调和。
        if self.active_binding.is_some() {
            // 外部状态是当前活动项的唯一事实源。
            self.sync_bound_active();
        } else {
            // 非受控重建优先保留旧活动条目的稳定身份。
            self.active_index = selected_id
                .as_deref()
                .and_then(|id| self.items.iter().position(|item| item.id == id))
                .unwrap_or_else(|| next.active_index.min(self.items.len().saturating_sub(1)));
        }
        self.body_scroll.clamp_to_content(
            self.items.len(),
            self.item_stride(),
            self.list_body_viewport_height(),
        );
        self.hovered_index.set(
            self.hovered_index
                .get()
                .filter(|idx| *idx < self.items.len()),
        );
        self.pressed_action.set(None);
    }

    fn geometry(&self, frame: Rect) -> SelectableListGeometry {
        let frame = Self::normalized_frame(frame);
        let header_height = if self.header_button_text.is_empty() {
            0.0
        } else {
            self.visual.geometry.header_height.min(frame.h)
        };
        let remaining = (frame.h - header_height).max(0.0);
        let footer_height = if self.footer_text.is_empty() {
            0.0
        } else {
            self.visual.geometry.footer_height.min(remaining)
        };
        let body_height = (remaining - footer_height).max(0.0);
        let header_button = (!self.header_button_text.is_empty()).then(|| {
            let horizontal_inset = self.visual.geometry.header_inset.min(frame.w * 0.5);
            let vertical_inset = self.visual.geometry.header_inset.min(header_height * 0.5);
            Rect::new(
                frame.x + horizontal_inset,
                frame.y + vertical_inset,
                (frame.w - horizontal_inset * 2.0).max(0.0),
                self.visual
                    .geometry
                    .header_button_height
                    .min((header_height - vertical_inset * 2.0).max(0.0)),
            )
        });
        let body = Rect::new(frame.x, frame.y + header_height, frame.w, body_height);
        let footer = (!self.footer_text.is_empty() && footer_height > 0.0)
            .then(|| Rect::new(frame.x, body.y + body.h, frame.w, footer_height));
        SelectableListGeometry {
            frame,
            header_button,
            body,
            footer,
        }
    }

    fn local_geometry(&self, frame: Rect) -> SelectableListGeometry {
        self.geometry(Rect::new(0.0, 0.0, frame.w, frame.h))
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

    fn paint_single_line(
        ctx: &mut PaintContext,
        value: &str,
        frame: Rect,
        color: Color,
        font_size: f32,
    ) {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        // 复用 UI 绘制上下文拥有的保守单行省略算法。
        let Some(value) = ctx.elide_single_line_cow(value, font_size, frame.w) else {
            return;
        };
        ctx.push_clip(frame);
        ctx.draw_text_in_frame(&value, frame, color, font_size);
        ctx.pop_clip();
    }
}

widget! {
    /// A vertical list with selectable rows.
    pub struct SelectableList {
        /// 按展示顺序持有的可选择条目。
        pub items: Vec<SelectableItem>,
        /// 非受控模式下当前活动条目的原始索引。
        pub active_index: usize,
        // 外部状态只拥有稳定 id，不接管列表数据与滚动状态。
        #[snapshot(skip)]
        active_binding: Option<State<Option<String>>>,
        /// 列表顶部操作按钮的文本；空字符串隐藏该入口。
        pub header_button_text: String,
        /// 列表底部显示的文本；空字符串隐藏页脚。
        pub footer_text: String,
        /// 每个列表条目的逻辑行高。
        pub item_height: f32,
        // 标记行高是否由 Rust 调用方显式覆盖。
        #[snapshot(skip)]
        item_height_authored: bool,
        hovered_index: Cell<Option<usize>>,
        hovered_header: Cell<bool>,
        pressed_action: Cell<Option<SelectableListAction>>,
        focused: bool,
        pub(crate) body_scroll: VirtualListScroll,
        scroll_delta_strip: Cell<(f32, f32)>,
        pub(crate) last_frame: Cell<Option<Rect>>,
        pending_action: Cell<Option<SelectableListAction>>,
        // 全部实例共享 UIX 声明固化后的只读视觉配置。
        #[snapshot(skip)]
        visual: &'static SelectableListVisual,
    }
    @new -> Self {
        Self {
            items: Vec::new(),
            active_index: 0,
            // 缺省保持既有非受控索引模式。
            active_binding: None,
            header_button_text: String::new(),
            footer_text: String::new(),
            item_height: SELECTABLE_LIST_VISUAL.geometry.default_item_height,
            item_height_authored: false,
            hovered_index: Cell::new(None),
            hovered_header: Cell::new(false),
            pressed_action: Cell::new(None),
            focused: false,
            body_scroll: VirtualListScroll::new(),
            scroll_delta_strip: Cell::new((0.0, 0.0)),
            last_frame: Cell::new(None),
            pending_action: Cell::new(None),
            visual: SELECTABLE_LIST_VISUAL_REF,
        }
    }
    tab_index => (&self) -> i32 {
        i32::from(!self.items.is_empty() || !self.header_button_text.is_empty())
    }
    measure => (&self, constraints: Constraints) -> Size {
        let mut h = 0.0;
        if !self.header_button_text.is_empty() {
            h += self.visual.geometry.header_height;
        }
        h += self.items.len() as f32 * self.item_stride();
        if !self.footer_text.is_empty() {
            h += self.visual.geometry.footer_height;
        }
        constraints.clamp(Size::new(
            self.visual.geometry.default_width,
            h.max(self.visual.geometry.min_height),
        ))
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        // 每次交互前重新读取外部事实，避免基于过期活动项处理输入。
        self.sync_bound_active();
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(action) = self.action_at_point(*pos) {
                    self.hovered_header
                        .set(action == SelectableListAction::Header);
                    self.hovered_index.set(match action {
                        SelectableListAction::Row(index) => Some(index),
                        SelectableListAction::Header => None,
                    });
                    self.pressed_action.set(Some(action));
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }

            SystemEvent::PointerUp {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let pressed = self.pressed_action.replace(None);
                if let Some(action) = pressed {
                    let released = self.action_at_point(*pos);
                    self.hovered_header
                        .set(released == Some(SelectableListAction::Header));
                    self.hovered_index.set(match released {
                        Some(SelectableListAction::Row(index)) => Some(index),
                        _ => None,
                    });
                    if released == Some(action) {
                        match action {
                            SelectableListAction::Header => {
                                self.pending_action.set(Some(SelectableListAction::Header));
                            }
                            SelectableListAction::Row(index) => self.select(index, true),
                        }
                    }
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }

            SystemEvent::PointerMove { pos, .. } => {
                let old_hover = self.hovered_index.get();
                let old_btn = self.hovered_header.get();

                let action = self.action_at_point(*pos);
                let new_btn = action == Some(SelectableListAction::Header);
                let new_hover = match action {
                    Some(SelectableListAction::Row(index)) => Some(index),
                    _ => None,
                };

                self.hovered_index.set(new_hover);
                self.hovered_header.set(new_btn);
                if old_hover != new_hover || old_btn != new_btn {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }

            SystemEvent::Wheel { pos, delta } => {
                let frame = self
                    .last_frame
                    .get()
                    .unwrap_or_else(|| {
                        Rect::new(
                            0.0,
                            0.0,
                            self.visual.geometry.default_width,
                            self.visual.geometry.default_height,
                        )
                    });
                if !self.local_geometry(frame).body.contains(*pos) {
                    return EventResult::NotHandled;
                }
                let viewport_h = self.list_body_viewport_height();
                let dy = self.body_scroll.scroll_by_wheel(
                    delta.y,
                    self.items.len(),
                    self.item_stride(),
                    viewport_h,
                );
                if dy.abs() > 0.01 {
                    self.push_scroll_delta(0.0, dy);
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }

            SystemEvent::PointerLeave => {
                let changed = self.hovered_index.replace(None).is_some()
                    | self.hovered_header.replace(false)
                    | self.pressed_action.replace(None).is_some();
                if changed {
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
                self.pressed_action.set(None);
                EventResult::Handled
            }

            SystemEvent::KeyDown { key, .. } => match key {
                KeyCode::Up if !self.items.is_empty() => {
                    self.move_active(false);
                    EventResult::Handled
                }
                KeyCode::Down if !self.items.is_empty() => {
                    self.move_active(true);
                    EventResult::Handled
                }
                KeyCode::Home if !self.items.is_empty() => {
                    self.select(0, false);
                    EventResult::Handled
                }
                KeyCode::End if !self.items.is_empty() => {
                    self.select(self.items.len() - 1, false);
                    EventResult::Handled
                }
                KeyCode::Enter | KeyCode::Space if self.selected_id().is_some() => {
                    self.select(self.active_index, true);
                    EventResult::Handled
                }
                KeyCode::Enter | KeyCode::Space if !self.header_button_text.is_empty() => {
                    self.pending_action.set(Some(SelectableListAction::Header));
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },

            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        match self.pending_action.take()? {
            SelectableListAction::Header => {
                Some(SemanticEvent::submit(id, self.header_button_text.clone()))
            }
            SelectableListAction::Row(index) => self
                .items
                .get(index)
                .map(|item| SemanticEvent::change(id, item.id.clone())),
        }
    }

    scroll_delta_for_dirty => (&self) -> Option<(f32, f32)> {
        let delta = self.scroll_delta_strip.get();
        if delta.0.abs() > 0.01 || delta.1.abs() > 0.01 {
            self.scroll_delta_strip.set((0.0, 0.0));
            Some(delta)
        } else {
            None
        }
    }

    scroll_composite_viewport => (&self, frame: Rect) -> Option<Rect> {
        let body = self.geometry(frame).body;
        (body.w > 0.0 && body.h > 0.0).then_some(body)
    }

    viewport_scroll_offset => (&self) -> Option<(f32, f32)> {
        Some((0.0, self.body_scroll.scroll_offset()))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        // 登记受控活动状态依赖，外部更新会重建并同步当前组件。
        self.capture_bound_active_dependency();
        let geometry = self.geometry(frame);
        let frame = geometry.frame;
        self.last_frame.set(Some(frame));
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let resolved = self.visual.resolve(ctx.tokens());
        let chrome = self.visual.chrome;

        ctx.push_clip(frame);
        ctx.fill_rect(frame, resolved.background, None);
        if frame.w >= chrome.frame_border_width {
            ctx.fill_rect(
                Rect::new(
                    frame.x + frame.w - chrome.frame_border_width,
                    frame.y,
                    chrome.frame_border_width,
                    frame.h,
                ),
                resolved.border,
                None,
            );
        }

        if let Some(btn_frame) = geometry.header_button {
            let pressed = self.pressed_action.get() == Some(SelectableListAction::Header)
                && self.hovered_header.get();
            let btn_bg = if pressed {
                // 按压态：token 主色 + 固定 alpha（替换原硬编码 55,110,255 以支持换肤）。
                resolved.primary.with_alpha(chrome.header_pressed_alpha)
            } else if self.hovered_header.get() {
                resolved.fill_hover
            } else {
                resolved.fill_secondary
            };
            ctx.fill_rect(
                btn_frame,
                btn_bg,
                Some(Radius::uniform(chrome.row_radius)),
            );
            let icon_size = chrome.row_icon_size.min(btn_frame.h);
            let icon_frame = Rect::new(
                btn_frame.x
                    + chrome
                        .header_icon_inset
                        .min(btn_frame.w * chrome.header_icon_frame_ratio),
                btn_frame.y + (btn_frame.h - icon_size) * chrome.center_ratio,
                icon_size,
                icon_size,
            );
            crate::ui::widgets::general::icon::Icon::paint_in_frame(
                ctx,
                chrome.plus_icon,
                icon_frame,
                resolved.text_secondary,
                resolved.icon_font_size,
            );
            let text_frame = Rect::new(
                icon_frame.x + icon_frame.w + chrome.header_text_gap,
                btn_frame.y,
                (btn_frame.x + btn_frame.w
                    - icon_frame.x
                    - icon_frame.w
                    - chrome.header_text_right_pad)
                    .max(0.0),
                btn_frame.h,
            );
            Self::paint_single_line(
                ctx,
                &self.header_button_text,
                text_frame,
                resolved.text_secondary,
                chrome.row_font_size,
            );
            let separator_width =
                (frame.w - self.visual.geometry.header_inset * 2.0).max(0.0);
            if separator_width > 0.0 && geometry.body.y > frame.y {
                ctx.fill_rect(
                    Rect::new(
                        frame.x + self.visual.geometry.header_inset.min(frame.w * 0.5),
                        (geometry.body.y - chrome.separator_y_offset).max(frame.y),
                        separator_width,
                        chrome.separator_height,
                    ),
                    resolved.border,
                    None,
                );
            }
        }

        let list_clip = geometry.body;
        ctx.push_clip(list_clip);

        let stride = self.item_stride();
        let scroll_offset = self.body_scroll.scroll_offset();
        let (start, end) = self
            .body_scroll
            .scroll_range(self.items.len(), stride, list_clip.h);

        for i in start..end {
            let iy = list_clip.y + i as f32 * stride - scroll_offset;
            if iy + self.item_height < list_clip.y || iy > list_clip.y + list_clip.h {
                continue;
            }

            let is_active = i == self.active_index;
            let is_hover = self.hovered_index.get() == Some(i);
            let horizontal_inset = self
                .visual
                .geometry
                .row_horizontal_inset
                .min(frame.w * 0.5);
            let item_frame = Rect::new(
                frame.x + horizontal_inset,
                iy,
                (frame.w - horizontal_inset * 2.0).max(0.0),
                self.item_height.min(list_clip.h.max(0.0)),
            );
            let is_pressed = self.pressed_action.get() == Some(SelectableListAction::Row(i))
                && is_hover;

            if is_pressed {
                ctx.fill_rect(
                    item_frame,
                    // 按压态：token 主色 + 固定 alpha（替换原硬编码 55,110,255 以支持换肤）。
                    resolved.primary.with_alpha(chrome.row_pressed_alpha),
                    Some(Radius::uniform(chrome.row_radius)),
                );
            } else if is_active {
                // 活动态：token 主色 + 固定 alpha（替换原硬编码 55,110,255 以支持换肤）。
                ctx.fill_rect(
                    item_frame,
                    resolved.primary.with_alpha(chrome.row_active_alpha),
                    Some(Radius::uniform(chrome.row_radius)),
                );
                ctx.fill_rect(
                    Rect::new(
                        item_frame.x,
                        item_frame.y
                            + chrome
                                .active_bar_vertical_inset
                                .min(item_frame.h * chrome.center_ratio),
                        chrome.active_bar_width.min(item_frame.w),
                        (item_frame.h - chrome.active_bar_vertical_inset * 2.0).max(0.0),
                    ),
                    resolved.primary,
                    Some(Radius::uniform(chrome.active_bar_radius)),
                );
            } else if is_hover {
                ctx.fill_rect(
                    item_frame,
                    resolved.fill_hover,
                    Some(Radius::uniform(chrome.row_radius)),
                );
            }

            let icon = self.items[i].icon.as_deref().unwrap_or("");
            let icon_slot = if icon.is_empty() {
                0.0
            } else {
                chrome.row_icon_slot.min(item_frame.w)
            };
            let text_x = if icon.is_empty() {
                item_frame.x + chrome.row_text_indent.min(item_frame.w * 0.25)
            } else {
                item_frame.x + icon_slot + chrome.row_icon_text_gap.min(item_frame.w * 0.1)
            };
            if !icon.is_empty() {
                let icon_size = chrome.row_icon_size.min(item_frame.h);
                crate::ui::widgets::general::icon::Icon::paint_in_frame(
                    ctx,
                    icon,
                    Rect::new(
                        item_frame.x
                            + self
                                .visual
                                .geometry
                                .row_horizontal_inset
                                .min(item_frame.w * 0.2),
                        item_frame.y + (item_frame.h - icon_size) * chrome.center_ratio,
                        icon_size,
                        icon_size,
                    ),
                    resolved.text_secondary,
                    resolved.icon_font_size,
                );
            }

            let color = if is_active {
                resolved.primary
            } else {
                resolved.text_secondary
            };
            let text_frame = Rect::new(
                text_x,
                item_frame.y,
                (item_frame.x + item_frame.w - text_x - chrome.row_text_right_pad).max(0.0),
                item_frame.h,
            );
            Self::paint_single_line(
                ctx,
                &self.items[i].text,
                text_frame,
                color,
                chrome.row_font_size,
            );
        }

        ctx.pop_clip();

        if let Some(footer) = geometry.footer {
            let horizontal_inset = chrome
                .footer_inset
                .min(footer.w * chrome.footer_inset_ratio);
            Self::paint_single_line(
                ctx,
                &self.footer_text,
                Rect::new(
                    footer.x + horizontal_inset,
                    footer.y,
                    (footer.w - horizontal_inset * 2.0).max(0.0),
                    footer.h,
                ),
                resolved.text_tertiary,
                chrome.footer_font_size,
            );
        }
        if self.focused && tree.keyboard_focus_visible() {
            let inset = chrome
                .focus_inset
                .min(frame.w * chrome.center_ratio)
                .min(frame.h * chrome.center_ratio);
            let focus_frame = Rect::new(
                frame.x + inset,
                frame.y + inset,
                (frame.w - inset * 2.0).max(0.0),
                (frame.h - inset * 2.0).max(0.0),
            );
            ctx.stroke_rect(
                focus_frame,
                resolved.primary,
                chrome.focus_stroke_width,
                Some(Radius::uniform(resolved.focus_radius)),
            );
        }
        ctx.pop_clip();
    }
}

// 把列表数据、受控选择与 UIX 静态视觉融合为单一根节点。
fn build_selectable_list_view(
    mut kernel: SelectableList,
    visual: &'static SelectableListVisual,
) -> ViewNode {
    if !kernel.item_height_authored {
        kernel.item_height = visual.geometry.default_item_height;
    }
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for SelectableList {
    fn build(self) -> ViewNode {
        // UIX 拥有公开根与静态视觉；Rust 保留状态、输入、虚拟化与底层绘制。
        let kernel = self;
        crate::uix!("src/ui/widgets/display/selectable_list/selectable_list.uix")
    }
}

impl SelectableList {
    // 测试目标观察 UIX 声明的关键视觉契约，不扩大公开 API。
    #[cfg(test)]
    fn visual_contract_for_test(&self) -> (f32, f32, f32, f32, f32, f32) {
        (
            self.visual.geometry.default_width,
            self.visual.geometry.default_height,
            self.visual.geometry.min_height,
            self.visual.geometry.default_item_height,
            self.visual.geometry.row_gap,
            self.visual.chrome.row_radius,
        )
    }

    // 测试目标确认实例共享同一份 UIX 视觉表。
    #[cfg(test)]
    fn shares_visual_with_for_test(&self, other: &Self) -> bool {
        std::ptr::eq(self.visual, other.visual)
    }
}

// 集中验证稳定 id 受控绑定与非受控兼容边界。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/display/selectable_list/tests.rs"]
mod tests;
