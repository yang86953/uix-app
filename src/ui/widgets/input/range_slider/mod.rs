//! Two-thumb range slider.

use std::cell::Cell;
use std::ops::RangeInclusive;

use crate::core::{Constraints, Rect, Size};
use crate::draw::{Color, Radius};
use crate::platform::windowing::ControlSize;
use crate::ui::SnapshotFields;
use crate::ui::reactive::state::State;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, View, ViewNode, WidgetId,
    WidgetTree,
};
use crate::widget;

use super::slider::decimal_places;

// 保存 UIX 声明的固有宽度、轨道/拇指尺寸映射与焦点几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RangeSliderVisual {
    intrinsic_width: f32,
    small_track_height: f32,
    medium_track_height: f32,
    large_track_height: f32,
    small_thumb_radius: f32,
    medium_thumb_radius: f32,
    large_thumb_radius: f32,
    center_ratio: f32,
    thumb_stroke_width: f32,
    focus_stroke_width: f32,
    focus_radius: f32,
    primary: ColorValue,
    primary_hover: ColorValue,
    track: ColorValue,
    thumb: ColorValue,
}

// 同目录 UIX 生成唯一范围滑块视觉值及静态借用。
crate::uix_items!("src/ui/widgets/input/range_slider/range_slider.uix");

// 保存每帧一次解析后的主题颜色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolvedRangeSliderVisual {
    primary: Color,
    primary_hover: Color,
    track: Color,
    thumb: Color,
}

impl RangeSliderVisual {
    fn resolve(self, tokens: &dyn crate::ui::ThemeTokens) -> ResolvedRangeSliderVisual {
        ResolvedRangeSliderVisual {
            primary: self.primary.resolve(tokens),
            primary_hover: self.primary_hover.resolve(tokens),
            track: self.track.resolve(tokens),
            thumb: self.thumb.resolve(tokens),
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
const fn range_slider_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
const fn range_slider_primary_hover() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryHover)
}
const fn range_slider_track() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
const fn range_slider_thumb() -> ColorValue {
    ColorValue::Palette(PaletteColor::White)
}

/// 滑块拇指标识：左（起始）或右（结束）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeSliderThumb {
    /// 表示范围的起始值拇指。
    Start,
    /// 表示范围的结束值拇指。
    End,
}

widget! {
    /// Horizontal two-thumb range slider returned by [`Slider::range`](super::Slider::range).
    pub struct RangeSlider {
        min: f64,
        max: f64,
        step: f64,
        start_value: f64,
        end_value: f64,
        start_binding: Option<State<f64>>,
        end_binding: Option<State<f64>>,
        active_thumb: RangeSliderThumb,
        dragging: bool,
        hovered_thumb: Option<RangeSliderThumb>,
        focused: bool,
        slider_size: ControlSize,
        last_frame: Cell<Option<Rect>>,
        pending_change: Cell<Option<(f64, f64)>>,
        #[snapshot(skip)]
        /// UIX 声明的轨道、拇指、焦点与主题角色。
        pub(crate) visual: &'static RangeSliderVisual,
    }

    tab_index => (&self) -> i32 { 1 }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(self.visual.intrinsic_width, self.control_height()))
    }

    // 事件入口：指针拖动与键盘步进，并同步外部绑定值。
    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        self.sync_bound_values();
        match event {
            // 左键按下：就近选中拇指并开始拖动。
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let Some(frame) = self.last_frame.get() else {
                    return EventResult::NotHandled;
                };
                // 框外或范围非法时忽略。
                if !frame.contains(*pos) || self.max <= self.min {
                    return EventResult::NotHandled;
                }
                self.active_thumb = self.nearest_thumb(pos.x, frame);
                self.dragging = true;
                self.focused = true;
                self.hovered_thumb = Some(self.active_thumb);
                self.update_active_from_pos(pos.x, frame);
                EventResult::Handled
            }
            // 指针移动：拖动时更新值，否则更新悬浮拇指。
            SystemEvent::PointerMove { pos, .. } => {
                if let Some(frame) = self.last_frame.get() {
                    if self.dragging {
                        self.update_active_from_pos(pos.x, frame);
                        self.hovered_thumb = Some(self.active_thumb);
                    } else {
                        self.hovered_thumb = frame
                            .contains(*pos)
                            .then(|| self.nearest_thumb(pos.x, frame));
                    }
                } else {
                    self.hovered_thumb = None;
                }
                EventResult::Handled
            }
            // 左键抬起：结束拖动。
            SystemEvent::PointerUp {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if self.dragging {
                    if let Some(frame) = self.last_frame.get() {
                        self.update_active_from_pos(pos.x, frame);
                        self.hovered_thumb = frame
                            .contains(*pos)
                            .then(|| self.nearest_thumb(pos.x, frame));
                    }
                }
                self.dragging = false;
                EventResult::Handled
            }
            SystemEvent::PointerLeave => {
                self.hovered_thumb = None;
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
            // 方向键按步长增减当前拇指值。
            SystemEvent::KeyDown { key, .. } => match key {
                KeyCode::Right | KeyCode::Up => {
                    self.step_active(1.0);
                    EventResult::Handled
                }
                KeyCode::Left | KeyCode::Down => {
                    self.step_active(-1.0);
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            _ => EventResult::NotHandled,
        }
    }

    // 语义事件：取出待发范围变更并上报 change。
    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change.take().map(|(start, end)| {
            SemanticEvent::change(id, format!("{start}..{end}"))
        })
    }

    // 渲染：轨道、已选区间、双拇指与焦点框。
    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        self.capture_bound_value_dependencies();
        let control_rect = Rect::new(
            frame.x,
            frame.y,
            frame.w.max(0.0),
            frame.h.max(0.0).min(self.control_height()),
        );
        // 记录局部坐标系下的控件矩形（事件命中用）。
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, control_rect.w, control_rect.h)));
        // 空尺寸或范围非法时直接返回。
        if control_rect.w <= 0.0 || control_rect.h <= 0.0 || self.max <= self.min {
            return;
        }

        let visual = self.visual.resolve(ctx.tokens());
        let track_h = self.track_height(control_rect);
        let thumb_r = self.thumb_radius(control_rect);
        let cy = control_rect.y + control_rect.h * self.visual.center_ratio;
        let (track_x, track_w) = self.track_span(control_rect);
        let (start_x, end_x) = self.thumb_positions(control_rect);

        ctx.push_clip(control_rect);
        // 底色轨道。
        ctx.fill_rect(
            Rect::new(track_x, cy - track_h * 0.5, track_w, track_h),
            visual.track,
            Some(Radius::uniform(track_h * self.visual.center_ratio)),
        );
        // 已选区间高亮轨道。
        ctx.fill_rect(
            Rect::new(start_x, cy - track_h * 0.5, (end_x - start_x).max(0.0), track_h),
            visual.primary,
            Some(Radius::uniform(track_h * self.visual.center_ratio)),
        );

        // 顶层拇指优先显示拖动/悬浮中的那个，先画底层再画顶层。
        let top_thumb = if self.dragging {
            self.active_thumb
        } else {
            self.hovered_thumb.unwrap_or(self.active_thumb)
        };
        let bottom_thumb = match top_thumb {
            RangeSliderThumb::Start => RangeSliderThumb::End,
            RangeSliderThumb::End => RangeSliderThumb::Start,
        };
        for thumb in [bottom_thumb, top_thumb] {
            let x = match thumb {
                RangeSliderThumb::Start => start_x,
                RangeSliderThumb::End => end_x,
            };
            // 颜色：拖动中主题高亮，悬浮主题色，否则白色 token。
            let color = if self.dragging && self.active_thumb == thumb {
                visual.primary_hover
            } else if self.hovered_thumb == Some(thumb) {
                visual.primary
            } else {
                visual.thumb
            };
            ctx.fill_circle(x, cy, thumb_r, color);
            ctx.stroke_rect(
                Rect::new(x - thumb_r, cy - thumb_r, thumb_r * 2.0, thumb_r * 2.0),
                visual.primary,
                self.visual.thumb_stroke_width,
                Some(Radius::uniform(thumb_r)),
            );
        }

        // 聚焦时绘制控件焦点框。
        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(
                control_rect,
                visual.primary,
                self.visual.focus_stroke_width,
                Some(Radius::uniform(self.visual.focus_radius)),
            );
        }
        ctx.pop_clip();
    }
}

impl RangeSlider {
    /// 创建范围滑块；范围非法时自动归一化为升序。
    pub fn new(range: RangeInclusive<f64>) -> Self {
        let (min, max) = normalize_range(range);
        let config = crate::ui::widget_runtime::config::use_config();
        let visual = RANGE_SLIDER_VISUAL_REF;
        Self {
            min,
            max,
            step: 1.0,
            start_value: min,
            end_value: max,
            start_binding: None,
            end_binding: None,
            active_thumb: RangeSliderThumb::Start,
            dragging: false,
            hovered_thumb: None,
            focused: false,
            slider_size: config.size,
            last_frame: Cell::new(None),
            pending_change: Cell::new(None),
            visual,
        }
    }

    /// 设置步长；非法值视为无步长（连续取值）。
    pub fn step(mut self, step: f64) -> Self {
        self.step = if step.is_finite() && step > 0.0 {
            step
        } else {
            0.0
        };
        self
    }

    /// Bind the lower thumb to external state.
    pub fn start(mut self, state: &State<f64>) -> Self {
        self.start_binding = Some(state.clone());
        self.start_value = self.clamp_value(state.get());
        self.normalize_values();
        self
    }

    /// Bind the upper thumb to external state.
    pub fn end(mut self, state: &State<f64>) -> Self {
        self.end_binding = Some(state.clone());
        self.end_value = self.clamp_value(state.get());
        self.normalize_values();
        self
    }

    /// 设置控件尺寸规格（小/中/大）。
    pub fn size(mut self, size: ControlSize) -> Self {
        self.slider_size = size;
        self
    }

    /// 当前起止值。
    pub fn current_range(&self) -> (f64, f64) {
        (self.start_value, self.end_value)
    }

    /// 当前活动拇指。
    pub fn active_thumb(&self) -> RangeSliderThumb {
        self.active_thumb
    }

    /// 按指针位置更新当前活动拇指的值（含步长取整）。
    fn update_active_from_pos(&mut self, px: f32, frame: Rect) {
        let raw = self.value_from_pos(px, frame);
        // 有步长时四舍五入到最近的步长格点。
        let value = if self.step > 0.0 {
            let stepped = self.min + ((raw - self.min) / self.step).round() * self.step;
            self.normalize_step_value(stepped)
        } else {
            raw
        };
        self.set_thumb(self.active_thumb, value);
    }

    /// 键盘步进：在当前值附近取格点并向指定方向移动一步。
    fn step_active(&mut self, direction: f64) {
        // 无步长时键盘步进无效。
        if self.step <= 0.0 {
            return;
        }
        let current = match self.active_thumb {
            RangeSliderThumb::Start => self.start_value,
            RangeSliderThumb::End => self.end_value,
        };
        let position = (current - self.min) / self.step;
        // 已在格点上则直接 ±1，否则朝目标方向取相邻格点。
        let nearest = position.round();
        let on_grid = (position - nearest).abs() <= 1e-9 * position.abs().max(1.0);
        let index = if on_grid {
            nearest + direction.signum()
        } else if direction > 0.0 {
            position.ceil()
        } else {
            position.floor()
        };
        self.set_thumb(
            self.active_thumb,
            self.normalize_step_value(self.min + index * self.step),
        );
    }

    /// 设置拇指值：夹紧范围、保证起止不相交，并回写外部绑定状态。
    fn set_thumb(&mut self, thumb: RangeSliderThumb, value: f64) {
        let value = self.clamp_value(value);
        let changed = match thumb {
            // 起始拇指不得超过结束值。
            RangeSliderThumb::Start => {
                let value = value.min(self.end_value);
                if value == self.start_value {
                    false
                } else {
                    self.start_value = value;
                    if let Some(state) = self.start_binding.as_ref() {
                        if state.get() != value {
                            state.set(value);
                        }
                    }
                    true
                }
            }
            // 结束拇指不得小于起始值。
            RangeSliderThumb::End => {
                let value = value.max(self.start_value);
                if value == self.end_value {
                    false
                } else {
                    self.end_value = value;
                    if let Some(state) = self.end_binding.as_ref() {
                        if state.get() != value {
                            state.set(value);
                        }
                    }
                    true
                }
            }
        };
        // 值有变化时登记待发的 change 语义事件。
        if changed {
            self.pending_change
                .set(Some((self.start_value, self.end_value)));
        }
    }

    /// 从外部绑定状态同步当前值（事件处理前调用）。
    fn sync_bound_values(&mut self) {
        if let Some(state) = self.start_binding.as_ref() {
            self.start_value = self.clamp_value(state.get());
        }
        if let Some(state) = self.end_binding.as_ref() {
            self.end_value = self.clamp_value(state.get());
        }
        self.normalize_values();
    }

    /// 读取绑定值以登记响应式依赖（渲染期调用）。
    fn capture_bound_value_dependencies(&self) {
        if let Some(state) = self.start_binding.as_ref() {
            let _ = state.get();
        }
        if let Some(state) = self.end_binding.as_ref() {
            let _ = state.get();
        }
    }

    /// 夹紧并保证起止值不交叉。
    fn normalize_values(&mut self) {
        self.start_value = self.clamp_value(self.start_value);
        self.end_value = self.clamp_value(self.end_value);
        if self.start_value > self.end_value {
            self.start_value = self.end_value;
        }
    }

    /// 将值夹紧到 [min, max]；非有限值回退为 min。
    fn clamp_value(&self, value: f64) -> f64 {
        if value.is_finite() {
            value.clamp(self.min, self.max)
        } else {
            self.min
        }
    }

    /// 按步长精度四舍五入，消除浮点残差。
    fn normalize_step_value(&self, value: f64) -> f64 {
        // 取 min/step 的最大小数位作为舍入精度（上限 15 位）。
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

    /// 指针 x 坐标转范围值（按轨道比例线性映射）。
    fn value_from_pos(&self, px: f32, frame: Rect) -> f64 {
        let (track_x, track_w) = self.track_span(frame);
        let pct = if track_w > 0.0 {
            f64::from(((px - track_x) / track_w).clamp(0.0, 1.0))
        } else if px <= track_x {
            0.0
        } else {
            1.0
        };
        self.min + pct * (self.max - self.min)
    }

    /// 返回离指针最近的拇指；距离相等时倾向起始拇指。
    fn nearest_thumb(&self, px: f32, frame: Rect) -> RangeSliderThumb {
        let (start_x, end_x) = self.thumb_positions(frame);
        let start_distance = (px - start_x).abs();
        let end_distance = (px - end_x).abs();
        if start_distance < end_distance || (start_distance == end_distance && px < start_x) {
            RangeSliderThumb::Start
        } else {
            RangeSliderThumb::End
        }
    }

    /// 两个拇指在轨道上的 x 坐标。
    fn thumb_positions(&self, frame: Rect) -> (f32, f32) {
        let (track_x, track_w) = self.track_span(frame);
        let span = self.max - self.min;
        let start_pct = ((self.start_value - self.min) / span).clamp(0.0, 1.0) as f32;
        let end_pct = ((self.end_value - self.min) / span).clamp(0.0, 1.0) as f32;
        (track_x + start_pct * track_w, track_x + end_pct * track_w)
    }

    /// 控件高度（按尺寸规格）。
    fn control_height(&self) -> f32 {
        crate::ui::widget_runtime::config::control_height(self.slider_size)
    }

    /// 轨道厚度（按尺寸规格与视觉缩放）。
    fn track_height(&self, frame: Rect) -> f32 {
        let nominal = self.visual.track_height(self.slider_size);
        (nominal * self.visual_scale(frame)).min(frame.h)
    }

    /// 拇指半径（按尺寸规格与视觉缩放）。
    fn thumb_radius(&self, frame: Rect) -> f32 {
        let nominal = self.visual.thumb_radius(self.slider_size);
        (nominal * self.visual_scale(frame))
            .min(frame.w * self.visual.center_ratio)
            .min(frame.h * self.visual.center_ratio)
            .max(0.0)
    }

    /// 视觉缩放比例：控件被压缩时按比例缩小轨道与拇指。
    fn visual_scale(&self, frame: Rect) -> f32 {
        if self.control_height() > 0.0 {
            (frame.h / self.control_height()).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// 轨道横向范围：两端各留出拇指半径的内边距。
    fn track_span(&self, frame: Rect) -> (f32, f32) {
        let inset = self.thumb_radius(frame);
        (frame.x + inset, (frame.w - inset * 2.0).max(0.0))
    }
}

impl Default for RangeSlider {
    fn default() -> Self {
        Self::new(0.0..=100.0)
    }
}

impl RangeSlider {
    /// 导出快照字段（供快照同步使用）。
    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::RangeSlider {
            min: self.min,
            max: self.max,
            step: self.step,
            start: self.start_value,
            end: self.end_value,
            active_thumb: self.active_thumb,
            size: self.slider_size,
        }
    }

    /// 同步快照：有绑定时保留现有值，无绑定时采用新值。
    pub(crate) fn sync_from(&mut self, next: Self) {
        // 绑定存在时值由外部状态驱动，忽略快照中的值。
        let start_value = next
            .start_binding
            .as_ref()
            .map_or(self.start_value, |_| next.start_value);
        let end_value = next
            .end_binding
            .as_ref()
            .map_or(self.end_value, |_| next.end_value);
        self.min = next.min;
        self.max = next.max;
        self.step = next.step;
        self.start_binding = next.start_binding;
        self.end_binding = next.end_binding;
        self.start_value = start_value;
        self.end_value = end_value;
        self.slider_size = next.slider_size;
        self.visual = next.visual;
        self.normalize_values();
    }
}

// 把 RangeSlider Rust 范围内核与 UIX 静态视觉组合为单一组件节点。
fn build_range_slider_view(
    mut kernel: RangeSlider,
    visual: &'static RangeSliderVisual,
) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for RangeSlider {
    fn build(self) -> ViewNode {
        build_range_slider_uix_root(self)
    }
}

// 为 UIX 根提供稳定的 Rust 内核绑定名称。
fn build_range_slider_uix_root(kernel: RangeSlider) -> ViewNode {
    crate::uix!("src/ui/widgets/input/range_slider/range_slider.uix")
}

// 验证范围滑块的 UIX 视觉注入契约。
#[cfg(test)]
#[path = "../../../../../tests/unit/ui/widgets/input/range_slider__tests.rs"]
mod tests;

/// 归一化范围：非有限值回退默认值，逆序自动交换。
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
