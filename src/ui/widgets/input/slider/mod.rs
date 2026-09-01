//! Slider input widget.

use std::cell::Cell;
use std::ops::RangeInclusive;

use crate::core::{Constraints, Rect, Size};
use crate::draw::{Color, Radius};
use crate::platform::windowing::ControlSize;
use crate::ui::reactive::state::State;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::widget;
// Slider 只依赖基础层提示气泡原语，不依赖反馈组件实现。
use crate::ui::SnapshotFields;
use crate::ui::widgets::TooltipPlacement;
use crate::ui::widgets::binding::{capture_dependency, write_if_changed};
use crate::ui::widgets::tooltip_primitives::{
    paint_tooltip_bubble, tooltip_bubble_rect, tooltip_fallback_surface,
};
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, View, ViewNode, WidgetId,
    WidgetTree,
};

// 保存 UIX 声明的固有尺寸、轨道/拇指、刻度与 Tooltip 呈现参数。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SliderVisual {
    intrinsic_width: f32,
    marks_height: f32,
    small_track_height: f32,
    medium_track_height: f32,
    large_track_height: f32,
    small_thumb_radius: f32,
    medium_thumb_radius: f32,
    large_thumb_radius: f32,
    center_ratio: f32,
    mark_radius: f32,
    mark_stroke_width: f32,
    thumb_stroke_width: f32,
    focus_stroke_width: f32,
    focus_radius: f32,
    mark_label_font_size: f32,
    tooltip_background_alpha: u8,
    tooltip_arrow: bool,
    tooltip_z_index: i32,
    primary: ColorValue,
    primary_hover: ColorValue,
    track: ColorValue,
    thumb: ColorValue,
    label: ColorValue,
    tooltip_background: ColorValue,
    tooltip_text: ColorValue,
}

// 同目录 UIX 生成唯一单值滑块视觉值及静态借用。
crate::uix_items!("src/ui/widgets/input/slider/slider.uix");

// 保存每帧一次解析后的主题颜色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolvedSliderVisual {
    primary: Color,
    primary_hover: Color,
    track: Color,
    thumb: Color,
    label: Color,
    tooltip_background: Color,
    tooltip_text: Color,
}

impl SliderVisual {
    fn resolve(self, tokens: &dyn crate::ui::ThemeTokens) -> ResolvedSliderVisual {
        ResolvedSliderVisual {
            primary: self.primary.resolve(tokens),
            primary_hover: self.primary_hover.resolve(tokens),
            track: self.track.resolve(tokens),
            thumb: self.thumb.resolve(tokens),
            label: self.label.resolve(tokens),
            tooltip_background: self
                .tooltip_background
                .resolve(tokens)
                .with_alpha(self.tooltip_background_alpha),
            tooltip_text: self.tooltip_text.resolve(tokens),
        }
    }

    fn track_height(self, size: ControlSize) -> f32 {
        match size {
            ControlSize::Small => self.small_track_height,
            ControlSize::Medium => self.medium_track_height,
            ControlSize::Large => self.large_track_height,
        }
    }

    fn thumb_radius(self, size: ControlSize) -> f32 {
        match size {
            ControlSize::Small => self.small_thumb_radius,
            ControlSize::Medium => self.medium_thumb_radius,
            ControlSize::Large => self.large_thumb_radius,
        }
    }
}

// 向 UIX 提供零分配主题角色。
const fn slider_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
const fn slider_primary_hover() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryHover)
}
const fn slider_track() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
const fn slider_thumb() -> ColorValue {
    ColorValue::Palette(PaletteColor::White)
}
const fn slider_label() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
const fn slider_tooltip_background() -> ColorValue {
    ColorValue::Palette(PaletteColor::Black)
}
const fn slider_tooltip_text() -> ColorValue {
    ColorValue::Palette(PaletteColor::White)
}

pub(super) fn decimal_places(value: f64) -> i32 {
    if !value.is_finite() || value == 0.0 {
        return 0;
    }
    let text = value.abs().to_string();
    let (mantissa, exponent) = text
        .split_once(['e', 'E'])
        .map_or((text.as_str(), 0), |(mantissa, exponent)| {
            (mantissa, exponent.parse::<i32>().unwrap_or(0))
        });
    let fraction = mantissa
        .split_once('.')
        .map_or(0, |(_, fraction)| fraction.len() as i32);
    (fraction - exponent).max(0)
}

widget! {
    /// Horizontal slider.
    pub struct Slider {
        min: f64,
        max: f64,
        step: f64,
        value: f64,
        value_binding: Option<State<f64>>,
        dragging: bool,
        hovered: bool,
        focused: bool,
        marks: Vec<(f64, String)>,
        tooltip: Option<TooltipPlacement>,
        slider_size: ControlSize,
        last_frame: Cell<Option<Rect>>,
        last_tooltip_rect: Cell<Option<Rect>>,
        // 缓存当前逻辑表面，统一拖动提示的绘制、脏区与登记边界。
        surface_rect: Cell<Option<Rect>>,
        pending_change: Cell<Option<f64>>,
        #[snapshot(skip)]
        /// UIX 声明的轨道、拇指、刻度、焦点与 Tooltip 视觉。
        pub(crate) visual: &'static SliderVisual,
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
                let Some(frame) = self.last_frame.get() else {
                    return EventResult::NotHandled;
                };
                if !frame.contains(*pos) || self.max <= self.min {
                    return EventResult::NotHandled;
                }
                self.dragging = true;
                self.focused = true;
                self.update_from_pos(pos.x, frame);
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if let Some(frame) = self.last_frame.get() {
                    if self.dragging {
                        self.update_from_pos(pos.x, frame);
                    }
                    self.hovered = frame.contains(*pos);
                } else {
                    self.hovered = false;
                }
                EventResult::Handled
            }
            SystemEvent::PointerUp {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if self.dragging {
                    if let Some(frame) = self.last_frame.get() {
                        self.update_from_pos(pos.x, frame);
                    }
                }
                self.dragging = false;
                EventResult::Handled
            }
            SystemEvent::PointerLeave => {
                self.hovered = false;
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
            SystemEvent::KeyDown { key, .. } => match key {
                KeyCode::Right | KeyCode::Up => {
                    self.step_by(1.0);
                    EventResult::Handled
                }
                KeyCode::Left | KeyCode::Down => {
                    self.step_by(-1.0);
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .take()
            .map(|value| SemanticEvent::change(id, value.to_string()))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        // 读取本次绘制使用的逻辑表面尺寸。
        let surface_size = ctx.logical_surface_size();
        // 缓存当前表面，供脏区和浮层登记复用。
        self.surface_rect.set(Some(Rect::new(
            // 表面横坐标固定为窗口原点。
            0.0,
            // 表面纵坐标固定为窗口原点。
            0.0,
            // 使用绘制上下文的逻辑宽度。
            surface_size.w,
            // 使用绘制上下文的逻辑高度。
            surface_size.h,
        )));
        // 继续捕获受控值依赖。
        self.capture_bound_value_dependency();
        // 受控显示值在绘制期解析：外部 set 只触发本节点 Paint 失效。
        let value = self.display_value();
        let control_height = frame.h.max(0.0).min(self.control_height());
        let control_rect = Rect::new(frame.x, frame.y, frame.w.max(0.0), control_height);
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, control_rect.w, control_rect.h)));
        self.last_tooltip_rect.set(None);
        if control_rect.w <= 0.0 || control_rect.h <= 0.0 || self.max <= self.min {
            return;
        }

        let visual = self.visual.resolve(ctx.tokens());
        let track_h = self.track_height(control_rect);
        let thumb_r = self.thumb_radius(control_rect);
        let cy = control_rect.y + control_rect.h * self.visual.center_ratio;

        let pct = ((value - self.min) / (self.max - self.min)).clamp(0.0, 1.0) as f32;
        let (track_x, track_w) = self.track_span(control_rect);
        let thumb_x = track_x + pct * track_w;

        ctx.push_clip(control_rect);
        ctx.fill_rect(
            Rect::new(track_x, cy - track_h * 0.5, track_w, track_h),
            visual.track,
            Some(Radius::uniform(track_h * self.visual.center_ratio)),
        );
        ctx.fill_rect(
            Rect::new(track_x, cy - track_h * 0.5, thumb_x - track_x, track_h),
            visual.primary,
            Some(Radius::uniform(track_h * self.visual.center_ratio)),
        );
        let mark_radius = (self.visual.mark_radius * self.visual_scale(control_rect))
            .min(control_rect.h * self.visual.center_ratio)
            .max(0.0);
        for (mark, _) in &self.marks {
            let mark_pct = ((*mark - self.min) / (self.max - self.min)).clamp(0.0, 1.0) as f32;
            let mark_x = track_x + mark_pct * track_w;
            ctx.fill_circle(
                mark_x,
                cy,
                mark_radius,
                if *mark <= value {
                    visual.primary
                } else {
                    visual.track
                },
            );
            ctx.stroke_rect(
                Rect::new(
                    mark_x - mark_radius,
                    cy - mark_radius,
                    mark_radius * 2.0,
                    mark_radius * 2.0,
                ),
                visual.primary,
                self.visual.mark_stroke_width,
                Some(Radius::uniform(mark_radius)),
            );
        }
        let thumb_color = if self.dragging {
            visual.primary_hover
        } else if self.hovered {
            visual.primary
        } else {
            visual.thumb
        };
        ctx.fill_circle(thumb_x, cy, thumb_r, thumb_color);
        ctx.stroke_rect(
            Rect::new(thumb_x - thumb_r, cy - thumb_r, thumb_r * 2.0, thumb_r * 2.0),
            visual.primary,
            self.visual.thumb_stroke_width,
            Some(Radius::uniform(thumb_r)),
        );

        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(
                control_rect,
                visual.primary,
                self.visual.focus_stroke_width,
                Some(Radius::uniform(self.visual.focus_radius)),
            );
        }
        ctx.pop_clip();

        let label_height = (frame.h - control_rect.h).max(0.0);
        if label_height > 0.0 && !self.marks.is_empty() {
            let label_top = control_rect.y + control_rect.h;
            let font_size = (self.visual.mark_label_font_size * self.visual_scale(control_rect))
                .min(label_height);
            ctx.push_clip(frame);
            for (index, (mark, label)) in self.marks.iter().enumerate() {
                let pct = ((*mark - self.min) / (self.max - self.min)).clamp(0.0, 1.0) as f32;
                let center = track_x + pct * track_w;
                let left = if index == 0 {
                    frame.x
                } else {
                    let previous = self.marks[index - 1].0;
                    let previous_pct =
                        ((previous - self.min) / (self.max - self.min)).clamp(0.0, 1.0) as f32;
                    (track_x + previous_pct * track_w + center) * 0.5
                };
                let right = if index + 1 == self.marks.len() {
                    frame.x + frame.w
                } else {
                    let next = self.marks[index + 1].0;
                    let next_pct =
                        ((next - self.min) / (self.max - self.min)).clamp(0.0, 1.0) as f32;
                    (center + track_x + next_pct * track_w) * 0.5
                };
                let label_rect = Rect::new(left, label_top, (right - left).max(0.0), label_height);
                if label_rect.w > 0.0 && font_size > 0.0 {
                    ctx.push_clip(label_rect);
                    ctx.text_center(label, label_rect, visual.label, font_size);
                    ctx.pop_clip();
                }
            }
            ctx.pop_clip();
        }

        let tooltip_rect = self.tooltip_target(control_rect).map(|(placement, target)| {
            let text = self.value.to_string();
            paint_tooltip_bubble(
                ctx,
                &text,
                target,
                placement,
                visual.tooltip_background,
                visual.tooltip_text,
                self.visual.tooltip_arrow,
            )
        });
        self.last_tooltip_rect.set(tooltip_rect);
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        let mut dirty = frame;
        if let Some(previous) = self.last_tooltip_rect.get() {
            dirty = dirty.union(&previous);
        }
        if let Some((placement, target)) = self.tooltip_target(frame) {
            dirty = dirty.union(&tooltip_bubble_rect(
                &self.value.to_string(),
                self.visual.tooltip_arrow,
                placement,
                target,
                self.tooltip_surface_or_fallback(target),
            ));
        }
        dirty
    }

    overlay_entry => (&self, id: WidgetId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        let (placement, target) = self.tooltip_target(frame)?;
        Some(
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Tooltip)
                .bounds(tooltip_bubble_rect(
                    &self.value.to_string(),
                    self.visual.tooltip_arrow,
                    placement,
                    target,
                    self.tooltip_surface_or_fallback(target),
                ))
                .z_index(self.visual.tooltip_z_index),
        )
    }

    // 使用组件树提供的同帧表面创建滑块提示登记。
    overlay_entry_for_surface => (&self, id: WidgetId, frame: Rect, surface: Rect) -> Option<crate::ui::OverlayEntry> {
        // 缓存当前逻辑表面，使登记与随后绘制使用同一边界。
        self.surface_rect.set(Some(surface));
        // 复用统一的滑块提示登记逻辑。
        self.overlay_entry(id, frame)
    }
}

impl Slider {
    fn update_from_pos(&mut self, px: f32, frame: Rect) {
        let (track_x, track_w) = self.track_span(frame);
        let pct = if track_w > 0.0 {
            f64::from(((px - track_x) / track_w).clamp(0.0, 1.0))
        } else if px <= track_x {
            0.0
        } else {
            1.0
        };
        let raw = self.min + pct * (self.max - self.min);
        if self.step > 0.0 {
            let stepped = self.min + ((raw - self.min) / self.step).round() * self.step;
            self.set_value(self.normalize_step_value(stepped));
        } else {
            self.set_value(raw);
        }
    }

    fn step_by(&mut self, direction: f64) {
        if self.step <= 0.0 {
            return;
        }

        let position = (self.value - self.min) / self.step;
        let nearest = position.round();
        let on_grid = (position - nearest).abs() <= 1e-9 * position.abs().max(1.0);
        let index = if on_grid {
            nearest + direction.signum()
        } else if direction > 0.0 {
            position.ceil()
        } else {
            position.floor()
        };
        self.set_value(self.normalize_step_value(self.min + index * self.step));
    }

    fn normalize_step_value(&self, value: f64) -> f64 {
        let precision = decimal_places(self.min)
            .max(decimal_places(self.step))
            .min(15);
        let factor = 10.0f64.powi(precision);
        let scaled = value * factor;
        if factor.is_finite() && scaled.is_finite() {
            scaled.round() / factor
        } else {
            value
        }
    }

    fn set_value(&mut self, value: f64) {
        let value = self.clamp_value(value);
        if value != self.value {
            self.value = value;
            self.write_bound_value();
            self.pending_change.set(Some(self.value));
        }
    }

    fn clamp_value(&self, value: f64) -> f64 {
        if value.is_finite() {
            value.clamp(self.min, self.max)
        } else {
            self.min
        }
    }

    fn sync_bound_value(&mut self) {
        if let Some(state) = self.value_binding.as_ref() {
            self.value = self.clamp_value(state.get());
        }
    }

    fn capture_bound_value_dependency(&self) {
        capture_dependency(self.value_binding.as_ref());
    }

    /// 受控显示值：绘制期直接读绑定 State（同一次读取完成 Paint 依赖捕获），
    /// 外部 set 无需整树重建即可反映到本节点；非受控回落 `self.value`。
    fn display_value(&self) -> f64 {
        match self.value_binding.as_ref() {
            Some(state) => self.clamp_value(state.get()),
            None => self.value,
        }
    }

    fn write_bound_value(&self) {
        write_if_changed(self.value_binding.as_ref(), self.value);
    }

    fn control_height(&self) -> f32 {
        crate::ui::widget_runtime::config::control_height(self.slider_size)
    }

    fn track_height(&self, frame: Rect) -> f32 {
        let nominal = self.visual.track_height(self.slider_size);
        (nominal * self.visual_scale(frame)).min(frame.h)
    }

    fn thumb_radius(&self, frame: Rect) -> f32 {
        let nominal = self.visual.thumb_radius(self.slider_size);
        (nominal * self.visual_scale(frame))
            .min(frame.w * self.visual.center_ratio)
            .min(frame.h * self.visual.center_ratio)
            .max(0.0)
    }

    fn visual_scale(&self, frame: Rect) -> f32 {
        if self.control_height() > 0.0 {
            (frame.h / self.control_height()).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    fn track_span(&self, frame: Rect) -> (f32, f32) {
        let inset = self.thumb_radius(frame);
        (frame.x + inset, (frame.w - inset * 2.0).max(0.0))
    }

    fn tooltip_target(&self, frame: Rect) -> Option<(TooltipPlacement, Rect)> {
        let placement = self.tooltip.filter(|_| self.dragging)?;
        let control_height = frame.h.max(0.0).min(self.control_height());
        let control_rect = Rect::new(frame.x, frame.y, frame.w.max(0.0), control_height);
        if control_rect.w <= 0.0 || control_rect.h <= 0.0 || self.max <= self.min {
            return None;
        }
        let (track_x, track_w) = self.track_span(control_rect);
        let pct = ((self.value - self.min) / (self.max - self.min)).clamp(0.0, 1.0) as f32;
        let thumb_x = track_x + pct * track_w;
        let thumb_r = self.thumb_radius(control_rect);
        Some((
            placement,
            Rect::new(
                thumb_x - thumb_r,
                control_rect.y + control_rect.h * self.visual.center_ratio - thumb_r,
                thumb_r * 2.0,
                thumb_r * 2.0,
            ),
        ))
    }

    // 返回当前表面，首次登记前按提示文字构造有限回退。
    fn tooltip_surface_or_fallback(&self, target: Rect) -> Rect {
        // 优先使用组件树或绘制上下文提供的真实表面。
        self.surface_rect
            // 读取可复制的可选表面缓存。
            .get()
            // 首次登记前根据当前值文字构造有限回退。
            .unwrap_or_else(|| tooltip_fallback_surface(&self.value.to_string(), target))
    }
}

impl Slider {
    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Slider {
            min: self.min,
            max: self.max,
            step: self.step,
            value: self.value,
            marks: self.marks.clone(),
            tooltip: self.tooltip,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let controlled_value = next.value_binding.as_ref().map(|_| next.value);
        self.min = next.min;
        self.max = next.max;
        self.step = next.step;
        self.marks = next.marks;
        self.tooltip = next.tooltip;
        self.value_binding = next.value_binding;
        self.value = controlled_value.unwrap_or_else(|| self.clamp_value(self.value));
        self.slider_size = next.slider_size;
        self.visual = next.visual;
    }
}

impl Default for Slider {
    fn default() -> Self {
        Self::new(0.0..=100.0)
    }
}

impl Slider {
    /// Create a two-thumb slider bound with [`crate::ui::widgets::input::range_slider::RangeSlider`]
    /// `start` and `end` bounds.
    pub fn range(range: RangeInclusive<f64>) -> super::RangeSlider {
        super::RangeSlider::new(range)
    }

    /// 创建使用指定闭区间且初始值位于下界的单值滑块。
    pub fn new(range: RangeInclusive<f64>) -> Self {
        let (min, max) = Self::normalize_range(range);
        let config = crate::ui::widget_runtime::config::use_config();
        let visual = SLIDER_VISUAL_REF;
        Self {
            min,
            max,
            step: 1.0,
            value: min,
            value_binding: None,
            dragging: false,
            hovered: false,
            focused: false,
            marks: Vec::new(),
            tooltip: None,
            slider_size: config.size,
            last_frame: Cell::new(None),
            last_tooltip_rect: Cell::new(None),
            // 新滑块尚未接收布局或绘制表面。
            surface_rect: Cell::new(None),
            pending_change: Cell::new(None),
            visual,
        }
    }

    /// 设置键盘和拖动操作使用的离散步长。
    pub fn step(mut self, step: f64) -> Self {
        self.step = if step.is_finite() && step > 0.0 {
            step
        } else {
            0.0
        };
        self
    }

    /// 将滑块值绑定到外部 `State<f64>`。
    ///
    /// 初始值用无跟踪读取：构建期 `get()` 会把绑定 State 登记为根 View
    /// 结构依赖，外部每次 `set`（如播放进度回填）都会触发整树重建；
    /// 受控值的后续变更由绘制期读取经 Paint 租约走节点级失效。
    pub fn value(mut self, state: &State<f64>) -> Self {
        self.value_binding = Some(state.clone());
        self.value = self.clamp_value(state.get_untracked());
        self
    }

    /// 设置非受控滑块的初始值。
    pub fn default_value(mut self, value: f64) -> Self {
        self.value_binding = None;
        self.value = self.clamp_value(value);
        self
    }

    /// 返回滑块当前经过区间约束后的值。
    pub fn current_value(&self) -> f64 {
        self.value
    }

    /// 设置滑块采用的控件尺寸。
    pub fn size(mut self, size: ControlSize) -> Self {
        self.slider_size = size;
        self
    }

    /// 配置轨道刻度；区间外与非有限值忽略，重复值以后配置的标签为准。
    pub fn marks<L>(mut self, marks: Vec<(f64, L)>) -> Self
    where
        L: Into<String>,
    {
        let mut normalized: Vec<(f64, String)> = Vec::new();
        for (value, label) in marks {
            if !value.is_finite() || value < self.min || value > self.max {
                continue;
            }
            let value = if value == 0.0 { 0.0 } else { value };
            let label = label.into();
            if let Some(existing) = normalized
                .iter_mut()
                .find(|(existing, _)| *existing == value)
            {
                existing.1 = label;
            } else {
                normalized.push((value, label));
            }
        }
        normalized.sort_by(|left, right| left.0.total_cmp(&right.0));
        self.marks = normalized;
        self
    }

    /// 配置拖动期间显示当前值的提示位置。
    pub fn tooltip(mut self, placement: TooltipPlacement) -> Self {
        self.tooltip = Some(placement);
        self
    }

    fn normalize_range(range: RangeInclusive<f64>) -> (f64, f64) {
        let (start, end) = range.into_inner();
        let start = if start.is_finite() { start } else { 0.0 };
        let end = if end.is_finite() { end } else { 100.0 };
        if start <= end {
            (start, end)
        } else {
            (end, start)
        }
    }

    fn intrinsic_size(&self) -> Size {
        let marks_height = if self.marks.is_empty() {
            0.0
        } else {
            self.visual.marks_height
        };
        Size::new(
            self.visual.intrinsic_width,
            self.control_height() + marks_height,
        )
    }
}

// 把 Slider Rust 值与 Overlay 内核和 UIX 静态视觉组合为单一组件节点。
fn build_slider_view(mut kernel: Slider, visual: &'static SliderVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Slider {
    fn build(self) -> ViewNode {
        build_slider_uix_root(self)
    }
}

// 为 UIX 根提供稳定的 Rust 内核绑定名称。
fn build_slider_uix_root(kernel: Slider) -> ViewNode {
    crate::uix!("src/ui/widgets/input/slider/slider.uix")
}
