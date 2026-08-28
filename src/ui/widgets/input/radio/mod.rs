//! Radio widget — 单选组，支持 horizontal/vertical、disabled、hover。

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Color;
use crate::draw::resources::font::text_backend::estimate_text_metrics;
use crate::platform::windowing::ControlSize;
use crate::ui::SnapshotFields;
use crate::ui::reactive::state::State;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widgets::binding::{capture_dependency, write_if_changed};
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, View, ViewNode, WidgetId,
    WidgetTree,
};
use crate::widget;
use std::cell::Cell;

/// 方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadioDirection {
    /// 从左到右排列各个选项。
    Horizontal,
    /// 从上到下排列各个选项。
    Vertical,
}

// 保存 UIX 声明的单选圆点、焦点圈、文字排版与固有尺寸策略。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RadioVisual {
    default_direction: RadioDirection,
    minimum_width: f32,
    dirty_scale_outset: f32,
    dirty_antialias_padding: f32,
    outer_radius: f32,
    dot_radius: f32,
    center_leading_extra: f32,
    focus_outset: f32,
    focus_stroke_width: f32,
    ring_stroke_width: f32,
    label_offset: f32,
    base_font_size: f32,
    item_extra_width: f32,
    center_ratio: f32,
    border_secondary: ColorValue,
    text_disabled: ColorValue,
    primary: ColorValue,
    text: ColorValue,
    primary_hover: ColorValue,
    border: ColorValue,
    focus_border: ColorValue,
}

// 同目录 UIX 生成唯一单选组视觉值及静态借用。
crate::uix_items!("src/ui/widgets/input/radio/radio.uix");

// 保存每帧一次解析后的主题颜色，全部选项共享。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolvedRadioVisual {
    border_secondary: Color,
    text_disabled: Color,
    primary: Color,
    text: Color,
    primary_hover: Color,
    border: Color,
    focus_border: Color,
}

impl RadioVisual {
    fn resolve(self, tokens: &dyn crate::ui::ThemeTokens) -> ResolvedRadioVisual {
        ResolvedRadioVisual {
            border_secondary: self.border_secondary.resolve(tokens),
            text_disabled: self.text_disabled.resolve(tokens),
            primary: self.primary.resolve(tokens),
            text: self.text.resolve(tokens),
            primary_hover: self.primary_hover.resolve(tokens),
            border: self.border.resolve(tokens),
            focus_border: self.focus_border.resolve(tokens),
        }
    }
}

// 向 UIX 提供默认方向和零分配主题角色。
const fn radio_default_direction() -> RadioDirection {
    RadioDirection::Horizontal
}
const fn radio_border_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BorderSecondary)
}
const fn radio_text_disabled() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextQuaternary)
}
const fn radio_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
const fn radio_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
const fn radio_primary_hover() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryHover)
}
const fn radio_border() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}
const fn radio_focus_border() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryBorder)
}

widget! {
    /// Radio — 单选按钮组。
    pub struct Radio {
        group_name: String,
        options: Vec<String>,
        selected: usize,
        value_binding: Option<State<String>>,
        disabled: bool,
        direction: RadioDirection,
        item_h: f32,
        hovered_idx: Option<usize>,
        focused: bool,
        pending_change: Cell<Option<usize>>,
        #[snapshot(skip)]
        /// UIX 声明的圆点、焦点、排版与主题角色。
        pub(crate) visual: &'static RadioVisual,
    }


    tab_index => (&self) -> i32 { 1 }
    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        self.sync_bound_value();
        // 与 Segmented 一致：disabled 拦截前先清理 FocusOut，避免禁用后残留焦点态。
        if matches!(event, SystemEvent::FocusOut) {
            self.focused = false;
            return EventResult::Handled;
        }
        if self.disabled { return EventResult::NotHandled; }
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(idx) = self.option_at(pos.x, pos.y) {
                    self.select_index(idx);
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                self.hovered_idx = self.option_at(pos.x, pos.y);
                EventResult::Handled
            }
            SystemEvent::PointerEnter => { EventResult::Handled }
            SystemEvent::PointerLeave => { self.hovered_idx = None; EventResult::Handled }
            SystemEvent::FocusIn => { self.focused = true; EventResult::Handled }
            SystemEvent::KeyDown { key, .. } => {
            match key {
                KeyCode::Right | KeyCode::Down => {
                    if let Some(next) = self.adjacent_index(true) {
                        self.select_index(next);
                    }
                    EventResult::Handled
                }
                KeyCode::Left | KeyCode::Up => {
                        if let Some(previous) = self.adjacent_index(false) {
                            self.select_index(previous);
                        }
                        EventResult::Handled
                    }
                    // 跳到组内首项。
                    KeyCode::Home => {
                        self.select_index(0);
                        EventResult::Handled
                    }
                    // 跳到组内末项。
                    KeyCode::End => {
                        self.select_index(self.options.len().saturating_sub(1));
                        EventResult::Handled
                    }
                    // Space 无独立激活分支：Radio 的激活语义是「方向键选中即激活」，
                    // 无需按键确认；保持现状避免与方向键选择产生两套状态来源。
                    _ => EventResult::NotHandled,
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .take()
            .map(|idx| SemanticEvent::change(id, idx.to_string()))
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    dirty_rect => (&self, frame: Rect) -> Rect {
        // 焦点圆会越过首个选项左边界；脏区必须覆盖全部抗锯齿像素，避免移动后残留。
        let margin = self.visual_scale() * self.visual.dirty_scale_outset
            + self.visual.dirty_antialias_padding;
        Rect::new(
            frame.x - margin,
            frame.y - margin,
            frame.w + margin * 2.0,
            frame.h + margin * 2.0,
        )
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        self.capture_bound_value_dependency();
        let cy = frame.y + self.item_h * self.visual.center_ratio;
        // 主题角色只在整组绘制开始时解析一次，避免选项循环重复查询。
        let visual = self.visual.resolve(ctx.tokens());

        match self.direction {
            RadioDirection::Horizontal => {
                let mut x = frame.x;
                for (i, opt) in self.options.iter().enumerate() {
                    let w = self.item_width(opt);
                    self.render_radio_item(
                        ctx,
                        i,
                        opt,
                        x,
                        cy,
                        w,
                        tree.keyboard_focus_visible(),
                        &visual,
                    );
                    x += w;
                }
            }
            RadioDirection::Vertical => {
                for (i, opt) in self.options.iter().enumerate() {
                    let y = frame.y
                        + i as f32 * self.item_h
                        + self.item_h * self.visual.center_ratio;
                    let w = frame.w;
                    self.render_radio_item(
                        ctx,
                        i,
                        opt,
                        frame.x,
                        y,
                        w,
                        tree.keyboard_focus_visible(),
                        &visual,
                    );
                }
            }
        }
    }
}

impl Radio {
    fn adjacent_index(&self, forward: bool) -> Option<usize> {
        let len = self.options.len();
        if len == 0 {
            return None;
        }
        // 有界导航：到边界即停止，不循环回绕（与 Tabs 的有界模式一致）。
        // 无效选择（绑定值不在选项内）先收敛到合法边界再移动。
        let clamped = self.selected.min(len - 1);
        Some(if forward {
            clamped.saturating_add(1).min(len - 1)
        } else {
            clamped.saturating_sub(1)
        })
    }

    fn select_index(&mut self, index: usize) {
        if index >= self.options.len() || self.selected == index {
            return;
        }
        self.selected = index;
        self.write_bound_value();
        self.pending_change.set(Some(index));
    }

    fn sync_bound_value(&mut self) {
        let Some(value) = self.value_binding.as_ref().map(State::get) else {
            return;
        };
        self.selected = self
            .options
            .iter()
            .position(|option| option == &value)
            .unwrap_or(usize::MAX);
    }

    fn capture_bound_value_dependency(&self) {
        capture_dependency(self.value_binding.as_ref());
    }

    fn write_bound_value(&self) {
        let Some(value) = self.options.get(self.selected) else {
            return;
        };
        write_if_changed(self.value_binding.as_ref(), value.clone());
    }

    fn intrinsic_size(&self) -> Size {
        match self.direction {
            RadioDirection::Horizontal => {
                // 直接累计宽度，避免测量阶段为临时数字列表分配堆内存。
                let w = self
                    .options
                    .iter()
                    .map(|option| self.item_width(option))
                    .sum::<f32>()
                    .max(self.visual.minimum_width);
                Size::new(w, self.item_h)
            }
            RadioDirection::Vertical => {
                // 一次遍历求最大宽度，不再收集临时 Vec。
                let w = self
                    .options
                    .iter()
                    .map(|option| self.item_width(option))
                    .fold(self.visual.minimum_width, f32::max);
                Size::new(w, self.item_h * self.options.len() as f32)
            }
        }
    }

    fn option_at(&self, px: f32, py: f32) -> Option<usize> {
        match self.direction {
            RadioDirection::Horizontal => {
                if py < 0.0 || py > self.item_h {
                    return None;
                }
                let mut cum_x = 0.0f32;
                for (i, opt) in self.options.iter().enumerate() {
                    let w = self.item_width(opt);
                    if px >= cum_x && px <= cum_x + w {
                        return Some(i);
                    }
                    cum_x += w;
                }
                None
            }
            RadioDirection::Vertical => {
                let idx = (py / self.item_h) as usize;
                if idx < self.options.len() && py >= 0.0 {
                    Some(idx)
                } else {
                    None
                }
            }
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "radio item geometry is passed explicitly during one paint operation"
    )]
    fn render_radio_item(
        &self,
        ctx: &mut PaintContext,
        i: usize,
        opt: &str,
        x: f32,
        cy: f32,
        segment_width: f32,
        focus_visible: bool,
        visual: &ResolvedRadioVisual,
    ) {
        let scale = self.visual_scale();
        let r = self.visual.outer_radius * scale;
        let dot_r = self.visual.dot_radius * scale;
        let selected = i == self.selected;
        let hovered = self.hovered_idx == Some(i);

        let (ring_color, dot_color, text_c) = if self.disabled {
            (
                visual.border_secondary,
                visual.border_secondary,
                visual.text_disabled,
            )
        } else if selected {
            (visual.primary, visual.primary, visual.text)
        } else if hovered {
            (visual.primary_hover, visual.primary_hover, visual.text)
        } else {
            (visual.border, visual.border, visual.text)
        };

        // 外圈、焦点圈与内点共享同一个圆心，避免圆角矩形独立像素对齐后产生偏心。
        let center = Point::new(x + r + self.visual.center_leading_extra * scale, cy);
        if self.focused && focus_visible && selected {
            let focus_outset = self.visual.focus_outset * scale;
            ctx.stroke_circle(
                center.x,
                center.y,
                r + focus_outset,
                visual.focus_border,
                self.visual.focus_stroke_width,
            );
        }

        // 外圈
        ctx.stroke_circle(
            center.x,
            center.y,
            r,
            ring_color,
            self.visual.ring_stroke_width,
        );

        // 选中填充点
        if selected {
            ctx.fill_circle(center.x, center.y, dot_r, dot_color);
        }
        // 使用 em-box 高度（font_size）垂直居中，而非字体度量高度
        let row_rect = Rect::new(
            x,
            cy - self.item_h * self.visual.center_ratio,
            segment_width,
            self.item_h,
        );
        let font_size = self.font_size();
        let text_y = ctx.visual_center_y(row_rect, font_size);
        ctx.draw_text(
            opt,
            Point::new(x + self.visual.label_offset * scale, text_y),
            text_c,
            font_size,
        );
    }
}

impl Default for Radio {
    fn default() -> Self {
        Self::new()
    }
}

impl Radio {
    /// 创建空选项、水平排列且可用的单选组。
    pub fn new() -> Self {
        let config = crate::ui::widget_runtime::config::use_config();
        Self {
            group_name: String::new(),
            options: Vec::new(),
            selected: 0,
            value_binding: None,
            disabled: false,
            direction: RADIO_VISUAL_REF.default_direction,
            item_h: crate::ui::widget_runtime::config::control_height(config.size),
            hovered_idx: None,
            focused: false,
            pending_change: Cell::new(None),
            visual: RADIO_VISUAL_REF,
        }
    }

    /// 创建具名受控单选组。
    pub fn group<I, S>(name: impl Into<String>, options: I, state: &State<String>) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self::new().group_name(name).options(options).value(state)
    }

    /// 设置用于标识单选组的名称。
    pub fn group_name(mut self, name: impl Into<String>) -> Self {
        self.group_name = name.into();
        self
    }

    /// 替换单选组的候选文本，并同步受控选中值。
    pub fn options<I, S>(mut self, options: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.options = options
            .into_iter()
            .map(|option| option.as_ref().to_owned())
            .collect();
        self.sync_bound_value();
        self
    }

    /// 设置非受控单选组的初始索引。
    pub fn default_selected(mut self, idx: usize) -> Self {
        self.value_binding = None;
        self.selected = idx;
        self
    }

    /// 将当前选项值绑定到外部 `State<String>`。
    pub fn value(mut self, state: &State<String>) -> Self {
        self.value_binding = Some(state.clone());
        self.sync_bound_value();
        self
    }

    /// 返回当前选中选项的文本。
    pub fn current_value(&self) -> Option<String> {
        self.options.get(self.selected).cloned()
    }

    /// 返回当前有效的选中索引。
    pub fn current_index(&self) -> Option<usize> {
        (self.selected < self.options.len()).then_some(self.selected)
    }

    /// 设置是否禁止单选组响应选择交互。
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }

    /// 设置单选项使用的标准控件尺寸。
    pub fn size(mut self, size: ControlSize) -> Self {
        self.item_h = crate::ui::widget_runtime::config::control_height(size);
        self
    }

    /// 将选项排列方向设置为垂直。
    pub fn vertical(mut self) -> Self {
        self.direction = RadioDirection::Vertical;
        self
    }

    fn visual_scale(&self) -> f32 {
        self.item_h / crate::ui::widget_runtime::config::control_height(ControlSize::Medium)
    }

    fn font_size(&self) -> f32 {
        self.visual.base_font_size * self.visual_scale().sqrt()
    }

    fn item_width(&self, option: &str) -> f32 {
        let scale = self.visual_scale();
        estimate_text_metrics(option, f32::INFINITY, self.font_size()).max_line_width
            + self.visual.item_extra_width * scale
    }
}

impl Radio {
    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Radio {
            group_name: self.group_name.clone(),
            options: self.options.clone(),
            selected: self.selected,
            disabled: self.disabled,
            direction: self.direction,
            item_h: self.item_h,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let controlled_selected = next.value_binding.as_ref().map(|_| next.selected);
        self.group_name = next.group_name;
        self.options = next.options;
        self.value_binding = next.value_binding;
        self.selected = controlled_selected.unwrap_or(if self.selected < self.options.len() {
            self.selected
        } else {
            usize::MAX
        });
        self.disabled = next.disabled;
        self.direction = next.direction;
        self.item_h = next.item_h;
        self.visual = next.visual;
    }
}

// 把 Radio Rust 选择内核与 UIX 静态视觉组合为单一组件节点。
fn build_radio_view(mut kernel: Radio, visual: &'static RadioVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Radio {
    fn build(self) -> ViewNode {
        build_radio_uix_root(self)
    }
}

// 为 UIX 根提供稳定的 Rust 内核绑定名称。
fn build_radio_uix_root(kernel: Radio) -> ViewNode {
    crate::uix!("src/ui/widgets/input/radio/radio.uix")
}
