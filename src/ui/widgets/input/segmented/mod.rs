//! Segmented widget — 分段选择器，支持 disabled/hover/keyboard/focus。

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::{Color, Radius};
use crate::platform::windowing::ControlSize;
use crate::ui::reactive::state::State;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widgets::binding::{capture_dependency, write_if_changed};
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent, View, ViewNode,
    WidgetId, WidgetTree,
};
use crate::widget;
use std::cell::Cell;

// Segmented 使用的主题圆角角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SegmentedRadiusRole {
    Small,
}

impl SegmentedRadiusRole {
    fn resolve(self, tokens: &dyn crate::ui::ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

// 保存 UIX 声明的分段排版、滑块、分隔线与焦点几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SegmentedVisual {
    base_font_size: f32,
    segment_extra_width: f32,
    thumb_inset: f32,
    thumb_radius: f32,
    divider_inset: f32,
    divider_width: f32,
    focus_stroke_width: f32,
    container_radius: SegmentedRadiusRole,
    container: ColorValue,
    container_disabled: ColorValue,
    selected: ColorValue,
    selected_disabled: ColorValue,
    primary: ColorValue,
    primary_hover: ColorValue,
    text: ColorValue,
    text_disabled: ColorValue,
    divider: ColorValue,
}

// 同目录 UIX 生成唯一分段选择视觉值及静态借用。
crate::uix_items!("src/ui/widgets/input/segmented/segmented.uix");

// 保存每帧一次解析后的颜色与圆角。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ResolvedSegmentedVisual {
    container: Color,
    container_disabled: Color,
    selected: Color,
    selected_disabled: Color,
    primary: Color,
    primary_hover: Color,
    text: Color,
    text_disabled: Color,
    divider: Color,
    container_radius: f32,
}

impl SegmentedVisual {
    fn resolve(self, tokens: &dyn crate::ui::ThemeTokens) -> ResolvedSegmentedVisual {
        ResolvedSegmentedVisual {
            container: self.container.resolve(tokens),
            container_disabled: self.container_disabled.resolve(tokens),
            selected: self.selected.resolve(tokens),
            selected_disabled: self.selected_disabled.resolve(tokens),
            primary: self.primary.resolve(tokens),
            primary_hover: self.primary_hover.resolve(tokens),
            text: self.text.resolve(tokens),
            text_disabled: self.text_disabled.resolve(tokens),
            divider: self.divider.resolve(tokens),
            container_radius: self.container_radius.resolve(tokens),
        }
    }

    fn visual_scale_for_height(self, height: f32) -> f32 {
        (height / crate::ui::widget_runtime::config::control_height(ControlSize::Medium)).max(0.0)
    }

    fn font_size_for_height(self, height: f32) -> f32 {
        self.base_font_size * self.visual_scale_for_height(height).sqrt()
    }

    fn segment_width_for_height(self, option: &str, height: f32) -> f32 {
        let scale = self.visual_scale_for_height(height);
        let font_size = self.font_size_for_height(height);
        crate::draw::resources::font::text_backend::estimate_text_metrics(
            option,
            f32::INFINITY,
            font_size,
        )
        .max_line_width
            + self.segment_extra_width * scale
    }
}

// 向 UIX 提供圆角和零分配主题角色。
const fn segmented_small_radius() -> SegmentedRadiusRole {
    SegmentedRadiusRole::Small
}
const fn segmented_container() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
const fn segmented_container_disabled() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillQuaternary)
}
const fn segmented_selected() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}
const fn segmented_selected_disabled() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillSecondary)
}
const fn segmented_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
const fn segmented_primary_hover() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryHover)
}
const fn segmented_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
const fn segmented_text_disabled() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextQuaternary)
}
const fn segmented_divider() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BorderSecondary)
}

widget! {
    /// Segmented — 水平分段选择器。
    pub struct Segmented {
        options: Vec<String>,
        selected: usize,
        value_binding: Option<State<String>>,
        disabled: bool,
        disabled_options: Vec<bool>,
        segmented_size: ControlSize,
        hovered_idx: Option<usize>,
        focused: bool,
        pending_change: Cell<Option<usize>>,
        control_rect: Cell<Rect>,
        #[snapshot(skip)]
        /// UIX 声明的容器、滑块、文字与分隔线视觉。
        pub(crate) visual: &'static SegmentedVisual,
    }

    tab_index => (&self) -> i32 { 1 }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        self.sync_bound_value();
        if matches!(event, SystemEvent::FocusOut) {
            self.focused = false;
            return EventResult::Handled;
        }
        if matches!(event, SystemEvent::PointerLeave) {
            let changed = self.hovered_idx.take().is_some();
            return if changed {
                EventResult::Handled
            } else {
                EventResult::NotHandled
            };
        }
        if self.disabled { return EventResult::NotHandled; }
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(idx) = self.segment_at(*pos) {
                    if self.select_index(idx) {
                        return EventResult::Handled;
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                let next = self
                    .segment_at(*pos)
                    .filter(|index| !self.is_segment_disabled(*index));
                if self.hovered_idx == next {
                    EventResult::NotHandled
                } else {
                    self.hovered_idx = next;
                    EventResult::Handled
                }
            }
            SystemEvent::PointerEnter => { EventResult::NotHandled }
            SystemEvent::FocusIn => { self.focused = true; EventResult::Handled }
            SystemEvent::KeyDown { key, .. } => {
                match key {
                    KeyCode::Right | KeyCode::Down => {
                        self.move_selection(true);
                        EventResult::Handled
                    }
                    KeyCode::Left | KeyCode::Up => {
                        self.move_selection(false);
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
            .take()
            .map(|idx| SemanticEvent::change(id, idx.to_string()))
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        self.capture_bound_value_dependency();
        let control_height = frame.h.max(0.0).min(self.control_height());
        let control_rect = Rect::new(frame.x, frame.y, frame.w.max(0.0), control_height);
        self.control_rect
            .set(Rect::new(0.0, 0.0, control_rect.w, control_rect.h));
        if control_rect.w <= 0.0 || control_rect.h <= 0.0 {
            return;
        }

        let visual = self.visual.resolve(ctx.tokens());
        let r = Some(Radius::uniform(visual.container_radius));

        // 整体背景
        let container_bg = if self.disabled {
            visual.container_disabled
        } else {
            visual.container
        };
        ctx.fill_rect(control_rect, container_bg, r);
        ctx.push_clip(control_rect);

        let mut x = control_rect.x;
        let visual_scale = self.visual.visual_scale_for_height(control_rect.h);
        let font_size = self.visual.font_size_for_height(control_rect.h);
        // 先求名义总宽，再在循环内直接缩放，避免每帧构造宽度 Vec。
        let nominal_total = self.nominal_width_total(control_rect.h);
        let ratio = if nominal_total > 0.0 {
            control_rect.w / nominal_total
        } else {
            0.0
        };
        let equal_width = if self.options.is_empty() {
            0.0
        } else {
            control_rect.w / self.options.len() as f32
        };
        let last = self.options.len().saturating_sub(1);
        let mut used = 0.0;

        for (i, opt) in self.options.iter().enumerate() {
            let seg_w = self.scaled_segment_width(
                i,
                last,
                control_rect.h,
                control_rect.w,
                nominal_total,
                ratio,
                equal_width,
                used,
            );
            used += seg_w;
            let seg_disabled = self.is_segment_disabled(i);
            let is_hovered = self.hovered_idx == Some(i) && !seg_disabled;
            let segment_rect = Rect::new(x, control_rect.y, seg_w, control_rect.h);

            if i == self.selected {
                // 选中项：白色背景 + 主色文字
                let thumb_bg = if seg_disabled {
                    visual.selected_disabled
                } else {
                    visual.selected
                };
                let inset = self.visual.thumb_inset * visual_scale;
                ctx.fill_rect(
                    Rect::new(
                        x + inset,
                        control_rect.y + inset,
                        (seg_w - inset * 2.0).max(0.0),
                        (control_rect.h - inset * 2.0).max(0.0),
                    ),
                    thumb_bg,
                    Some(Radius::uniform(self.visual.thumb_radius * visual_scale)),
                );
                let tc = if seg_disabled {
                    visual.text_disabled
                } else {
                    visual.primary
                };
                ctx.push_clip(segment_rect);
                ctx.text_center(opt, segment_rect, tc, font_size);
                ctx.pop_clip();
            } else if seg_disabled {
                ctx.push_clip(segment_rect);
                ctx.text_center(opt, segment_rect, visual.text_disabled, font_size);
                ctx.pop_clip();
            } else if is_hovered {
                ctx.push_clip(segment_rect);
                ctx.text_center(opt, segment_rect, visual.primary_hover, font_size);
                ctx.pop_clip();
            } else {
                ctx.push_clip(segment_rect);
                ctx.text_center(opt, segment_rect, visual.text, font_size);
                ctx.pop_clip();
            }

            // 分隔线（非选中项之间）
            if i > 0 && i != self.selected && i - 1 != self.selected {
                let inset = self.visual.divider_inset * visual_scale;
                ctx.draw_line(
                    x,
                    control_rect.y + inset,
                    x,
                    control_rect.y + control_rect.h - inset,
                    visual.divider,
                    self.visual.divider_width,
                );
            }

            x += seg_w;
        }
        ctx.pop_clip();

        // focus 边框指示
        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(
                control_rect,
                visual.primary,
                self.visual.focus_stroke_width,
                r,
            );
        }
    }
}

impl Segmented {
    fn select_index(&mut self, index: usize) -> bool {
        if index >= self.options.len() || self.is_segment_disabled(index) {
            return false;
        }
        if self.selected != index {
            self.selected = index;
            self.write_bound_value();
            self.pending_change.set(Some(index));
        }
        true
    }

    fn move_selection(&mut self, forward: bool) {
        let len = self.options.len();
        if len == 0 {
            return;
        }
        let start = if self.selected < len {
            if forward {
                (self.selected + 1) % len
            } else {
                (self.selected + len - 1) % len
            }
        } else if forward {
            0
        } else {
            len - 1
        };
        for offset in 0..len {
            let index = if forward {
                (start + offset) % len
            } else {
                (start + len - offset) % len
            };
            if !self.is_segment_disabled(index) {
                self.select_index(index);
                break;
            }
        }
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
        if self.options.is_empty() {
            return Size::new(0.0, self.control_height());
        }
        let w = self
            .options
            .iter()
            .map(|option| {
                self.visual
                    .segment_width_for_height(option, self.control_height())
            })
            .sum::<f32>();
        Size::new(w, self.control_height())
    }

    fn segment_at(&self, pos: Point) -> Option<usize> {
        let control = self.control_rect.get();
        if pos.x < control.x
            || pos.x >= control.x + control.w
            || pos.y < control.y
            || pos.y >= control.y + control.h
        {
            return None;
        }
        let nominal_total = self.nominal_width_total(control.h);
        let ratio = if nominal_total > 0.0 {
            control.w / nominal_total
        } else {
            0.0
        };
        let equal_width = control.w / self.options.len() as f32;
        let last = self.options.len() - 1;
        let mut cum_x = 0.0f32;
        for i in 0..self.options.len() {
            let seg_w = self.scaled_segment_width(
                i,
                last,
                control.h,
                control.w,
                nominal_total,
                ratio,
                equal_width,
                cum_x,
            );
            let end = cum_x + seg_w;
            if pos.x >= cum_x && pos.x < end {
                return Some(i);
            }
            cum_x += seg_w;
        }
        None
    }

    fn is_segment_disabled(&self, idx: usize) -> bool {
        self.disabled || self.disabled_options.get(idx).copied().unwrap_or(false)
    }

    fn control_height(&self) -> f32 {
        crate::ui::widget_runtime::config::control_height(self.segmented_size)
    }

    // 计算所有分段的名义宽度总和，不分配临时宽度列表。
    fn nominal_width_total(&self, height: f32) -> f32 {
        self.options
            .iter()
            .map(|option| self.visual.segment_width_for_height(option, height))
            .sum()
    }

    // 按当前 frame 缩放单个分段，并让末项精确消费浮点余量。
    #[allow(
        clippy::too_many_arguments,
        reason = "分段宽度热路径显式传入缓存值以避免临时分配"
    )]
    fn scaled_segment_width(
        &self,
        index: usize,
        last: usize,
        height: f32,
        frame_width: f32,
        nominal_total: f32,
        ratio: f32,
        equal_width: f32,
        used: f32,
    ) -> f32 {
        if nominal_total <= 0.0 {
            return equal_width.max(0.0);
        }
        if index == last {
            return (frame_width - used).max(0.0);
        }
        (self
            .visual
            .segment_width_for_height(&self.options[index], height)
            * ratio)
            .max(0.0)
    }

    fn reset_nominal_geometry(&self) {
        let size = self.intrinsic_size();
        self.control_rect.set(Rect::new(0.0, 0.0, size.w, size.h));
    }
}

impl Default for Segmented {
    fn default() -> Self {
        Self::new(Vec::<String>::new())
    }
}

impl Segmented {
    /// 创建按声明顺序持有选项且默认选择首项的分段选择器。
    pub fn new<I, S>(options: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let config = crate::ui::widget_runtime::config::use_config();
        let visual = SEGMENTED_VISUAL_REF;
        let options = options
            .into_iter()
            .map(|option| option.as_ref().to_owned())
            .collect::<Vec<_>>();
        let control_height = crate::ui::widget_runtime::config::control_height(config.size);
        let control_width = options
            .iter()
            .map(|option| visual.segment_width_for_height(option, control_height))
            .sum();
        Self {
            options,
            selected: 0,
            value_binding: None,
            disabled: false,
            disabled_options: Vec::new(),
            segmented_size: config.size,
            hovered_idx: None,
            focused: false,
            pending_change: Cell::new(None),
            control_rect: Cell::new(Rect::new(0.0, 0.0, control_width, control_height)),
            visual,
        }
    }

    /// 替换全部选项，并按受控值重新解析当前索引与固有尺寸。
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
        self.reset_nominal_geometry();
        self
    }

    /// 设置非受控分段选择器的初始索引。
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

    /// 返回当前索引对应的拥有型选项值；索引无效时返回 `None`。
    pub fn current_value(&self) -> Option<String> {
        self.options.get(self.selected).cloned()
    }

    /// 返回当前有效选项索引。
    pub fn current_index(&self) -> Option<usize> {
        (self.selected < self.options.len()).then_some(self.selected)
    }

    /// 设置整个分段选择器是否禁用交互。
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    /// 设置分段项采用的控件尺寸规格并重算固有尺寸。
    pub fn size(mut self, size: ControlSize) -> Self {
        self.segmented_size = size;
        self.reset_nominal_geometry();
        self
    }
    /// 将指定索引的选项标记为不可交互。
    pub fn disable_option(mut self, idx: usize) -> Self {
        while self.disabled_options.len() <= idx {
            self.disabled_options.push(false);
        }
        self.disabled_options[idx] = true;
        self
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Segmented {
            options: self.options.clone(),
            selected: self.selected,
            disabled: self.disabled,
            disabled_options: self.disabled_options.clone(),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let controlled_selected = next.value_binding.as_ref().map(|_| next.selected);
        let previous_selected = self.selected;
        let previous_value = self.options.get(previous_selected).cloned();
        self.options = next.options;
        self.value_binding = next.value_binding;
        self.disabled = next.disabled;
        self.disabled_options = next.disabled_options;
        self.segmented_size = next.segmented_size;
        let preserved_selection = previous_value
            .and_then(|value| self.options.iter().position(|option| option == &value))
            .or_else(|| (previous_selected < self.options.len()).then_some(previous_selected))
            .unwrap_or(usize::MAX);
        self.selected = controlled_selected.unwrap_or(preserved_selection);
        if self.disabled {
            self.hovered_idx = None;
            self.focused = false;
        } else if self
            .hovered_idx
            .is_some_and(|index| index >= self.options.len())
        {
            self.hovered_idx = None;
        }
        self.reset_nominal_geometry();
        self.visual = next.visual;
    }
}

// 把 Segmented Rust 选择内核与 UIX 静态视觉组合为单一组件节点。
fn build_segmented_view(mut kernel: Segmented, visual: &'static SegmentedVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Segmented {
    fn build(self) -> ViewNode {
        build_segmented_uix_root(self)
    }
}

// 为 UIX 根提供稳定的 Rust 内核绑定名称。
fn build_segmented_uix_root(kernel: Segmented) -> ViewNode {
    crate::uix!("src/ui/widgets/input/segmented/segmented.uix")
}
