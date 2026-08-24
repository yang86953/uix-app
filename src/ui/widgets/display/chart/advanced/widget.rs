use std::cell::Cell;
use std::rc::Rc;

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Color;
use crate::ui::animation::{AnimationConfig, TransitionPlayer};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{EventResult, MouseButton, SystemEvent, WidgetTree};
use crate::widget;

use super::super::bar_chart::BarData;
use super::super::line_chart::LineData;
use super::{
    AdvancedChartVisual, BrushConfig, ChartKind, ChartPayload, ChartSeries, ComboSeries,
    FunnelAlign, FunnelShape, GaugeRange, GaugeType, InteractionConfig, LabelPosition,
    LegendPosition, LineStyle, PointStyle, RadarAxis, RadarData, RadarShape, RoseStyle,
    ScatterData, TooltipConfig, TooltipTrigger,
};

widget! {
    /// 高级图表合成组件：持有全部图表配置、载荷数据与交互状态。
    pub struct ChartPlaceholder {
        pub(crate) width: f32,
        pub(crate) height: f32,
        pub(crate) kind: ChartKind,
        pub(crate) payload: ChartPayload,
        pub(crate) background: Option<Color>,
        pub(crate) padding: f32,
        pub(crate) legend: LegendPosition,
        pub(crate) title: String,
        pub(crate) subtitle: String,
        pub(crate) responsive: bool,
        pub(crate) grouped: bool,
        pub(crate) stacked: bool,
        pub(crate) horizontal: bool,
        pub(crate) smooth: bool,
        pub(crate) step: bool,
        pub(crate) bar_gap: f32,
        pub(crate) category_gap: f32,
        pub(crate) rose: bool,
        pub(crate) rose_style: RoseStyle,
        pub(crate) start_angle: f32,
        pub(crate) end_angle: f32,
        pub(crate) total: Option<f32>,
        pub(crate) label_visible: bool,
        pub(crate) label_position: LabelPosition,
        pub(crate) x_axis: String,
        pub(crate) y_axis: String,
        pub(crate) y_axis_right: String,
        pub(crate) bubble_scale: f32,
        pub(crate) point_size: f32,
        pub(crate) point_style: PointStyle,
        pub(crate) grid_levels: usize,
        pub(crate) fill_opacity: f32,
        pub(crate) color_min: Color,
        pub(crate) color_max: Color,
        pub(crate) calendar_mode: bool,
        pub(crate) year: i32,
        pub(crate) cell_size: f32,
        pub(crate) cell_gap: f32,
        pub(crate) show_values: bool,
        pub(crate) show_conversion_rate: bool,
        pub(crate) funnel_align: FunnelAlign,
        pub(crate) funnel_shape: FunnelShape,
        pub(crate) funnel_gap: f32,
        pub(crate) treemap_gap: f32,
        pub(crate) bar_series: Vec<ChartSeries<Vec<BarData>>>,
        pub(crate) line_series: Vec<ChartSeries<Vec<LineData>>>,
        pub(crate) combo_series: Vec<ComboSeries<Vec<LineData>>>,
        pub(crate) scatter_series: Vec<ChartSeries<Vec<ScatterData>>>,
        pub(crate) radar_axes: Vec<RadarAxis>,
        pub(crate) radar_series: Vec<ChartSeries<Vec<RadarData>>>,
        pub(crate) radar_shape: RadarShape,
        pub(crate) gauge_ranges: Vec<GaugeRange>,
        pub(crate) gauge_value: f32,
        pub(crate) gauge_min: f32,
        pub(crate) gauge_max: f32,
        pub(crate) gauge_type: GaugeType,
        pub(crate) pointer_width: f32,
        pub(crate) pointer_color: Option<Color>,
        pub(crate) value_format: Option<Rc<dyn Fn(f32) -> String>>,
        pub(crate) interaction: Option<InteractionConfig>,
        pub(crate) brush_config: Option<BrushConfig>,
        pub(crate) tooltip_config: Option<TooltipConfig>,
        pub(crate) x_labels: Vec<String>,
        pub(crate) y_labels: Vec<String>,
        pub(crate) color_stops: Vec<(f32, Color)>,
        pub(crate) reference_lines: Vec<(f32, String, LineStyle)>,
        pub(crate) animation_config: Option<AnimationConfig>,
        // 以下为运行时交互/动画状态，不参与快照同步。
        #[snapshot(skip)]
        pub(crate) animation_player: Option<TransitionPlayer>,
        #[snapshot(skip)]
        pub(crate) animation_dirty: Cell<bool>,
        #[snapshot(skip)]
        pub(crate) last_frame: Cell<Option<Rect>>,
        #[snapshot(skip)]
        pub(crate) hovered_pos: Cell<Option<Point>>,
        #[snapshot(skip)]
        pub(crate) tooltip_pos: Cell<Option<Point>>,
        #[snapshot(skip)]
        pub(crate) brush_start: Cell<Option<Point>>,
        #[snapshot(skip)]
        pub(crate) pan_start: Cell<Option<Point>>,
        #[snapshot(skip)]
        pub(crate) pan_origin: Cell<f32>,
        #[snapshot(skip)]
        pub(crate) pan_offset: Cell<f32>,
        #[snapshot(skip)]
        pub(crate) zoom: Cell<f32>,
        // 同目录 UIX 生成的唯一静态视觉表。
        #[snapshot(skip)]
        pub(crate) visual: &'static AdvancedChartVisual,
    }

    // 测量：响应式时取约束最大尺寸，否则取配置尺寸。
    measure => (&self, constraints: Constraints) -> Size {
        let width = super::super::responsive_extent(self.responsive, constraints.max.w, self.width);
        let height = super::super::responsive_extent(self.responsive, constraints.max.h, self.height);
        constraints.clamp(Size::new(width, height))
    }

    // 事件入口：悬浮跟踪、拖拽平移、框选、点击回调与滚轮缩放。
    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        // 未渲染过则无法参与交互。
        let Some(frame) = self.last_frame.get() else {
            return EventResult::NotHandled;
        };
        match event {
            // 指针移动：更新悬浮点；平移中换算偏移；悬停类交互声明处理。
            SystemEvent::PointerMove { pos, .. } => {
                let inside = frame.contains(*pos);
                if inside {
                    self.hovered_pos.set(Some(*pos));
                } else {
                    self.hovered_pos.set(None);
                }
                // 平移拖拽中：按拖动距离更新偏移。
                if let Some(start) = self.pan_start.get() {
                    let offset = (self.pan_origin.get() + pos.x - start.x).clamp(-frame.w, frame.w);
                    self.pan_offset.set(offset);
                    return EventResult::Handled;
                }
                // 有交互/框选/悬停 tooltip 时消费移动事件。
                let hover_tooltip = self
                    .tooltip_config
                    .as_ref()
                    .is_some_and(|config| config.trigger_mode() == TooltipTrigger::Hover);
                if inside
                    && (self.interaction.is_some()
                        || self.brush_config.is_some()
                        || hover_tooltip)
                {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            // 指针离开：清除悬浮点。
            SystemEvent::PointerLeave => {
                let changed = self.hovered_pos.take().is_some();
                if changed { EventResult::Handled } else { EventResult::NotHandled }
            }
            // 左键按下（框内）：依次处理平移、框选起点、点击回调与点击 tooltip。
            SystemEvent::PointerDown { pos, button: MouseButton::Left, .. }
                if frame.contains(*pos) =>
            {
                let mut handled = false;
                // 启用平移时记录拖拽起点与初始偏移。
                if self.interaction.as_ref().is_some_and(|config| config.pan) {
                    self.pan_start.set(Some(*pos));
                    self.pan_origin.set(self.pan_offset.get());
                    handled = true;
                }
                // 启用框选时记录框选起点。
                if self.brush_config.as_ref().is_some_and(|config| config.enabled) {
                    self.brush_start.set(Some(*pos));
                    handled = true;
                }
                // 点击回调：回传指针处数据标签。
                if let Some(config) = self.interaction.as_ref() {
                    if let Some(callback) = config.on_click {
                        let label = self.data_label_at(*pos, frame);
                        callback(&label);
                        handled = true;
                    }
                }
                // 点击式 tooltip：在显示/隐藏之间切换。
                if self
                    .tooltip_config
                    .as_ref()
                    .is_some_and(|config| config.trigger_mode() == TooltipTrigger::Click)
                {
                    self.tooltip_pos
                        .set(self.tooltip_pos.get().is_none().then_some(*pos));
                    handled = true;
                }
                if handled { EventResult::Handled } else { EventResult::NotHandled }
            }
            // 左键抬起：结束平移；有框选起点时归一化回调选区。
            SystemEvent::PointerUp { pos, button: MouseButton::Left, .. } => {
                let was_panning = self.pan_start.take().is_some();
                let Some(start) = self.brush_start.take() else {
                    return if was_panning {
                        EventResult::Handled
                    } else {
                        EventResult::NotHandled
                    };
                };
                // 选区回调：换算为 [0,1] 区间的左/右端点。
                if let Some(config) = self.brush_config.as_ref().filter(|config| config.enabled) {
                    if let Some(callback) = config.on_select {
                        let start_x = ((start.x - frame.x) / frame.w.max(f32::EPSILON)).clamp(0.0, 1.0);
                        let end_x = ((pos.x - frame.x) / frame.w.max(f32::EPSILON)).clamp(0.0, 1.0);
                        callback((start_x.min(end_x), start_x.max(end_x)));
                    }
                }
                EventResult::Handled
            }
            // 滚轮（框内）：缩放级别在 [1, 8] 间按增量调整。
            SystemEvent::Wheel { pos, delta } if frame.contains(*pos) => {
                if self.interaction.as_ref().is_some_and(|config| config.zoom) {
                    let factor = if delta.y.is_finite() {
                        (1.0 - delta.y * 0.001).max(0.01)
                    } else {
                        1.0
                    };
                    let next = (self.zoom.get() * factor).clamp(1.0, 8.0);
                    self.zoom.set(next);
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    // 需要持续指针移动：十字线/平移/框选/悬停 tooltip 任一启用时。
    wants_continuous_pointer_move => (&self) -> bool {
        self.interaction.as_ref().is_some_and(|config| config.crosshair || config.pan)
            || self.brush_config.as_ref().is_some_and(|config| config.enabled)
            || self
                .tooltip_config
                .as_ref()
                .is_some_and(|config| config.trigger_mode() == TooltipTrigger::Hover)
    }

    // 动画帧推进：动画未结束时标记脏区并返回继续。
    update_animation => (&mut self, dt: f64) -> bool {
        let Some(player) = self.animation_player.as_mut() else {
            self.animation_dirty.set(false);
            return false;
        };
        if player.finished {
            self.animation_dirty.set(false);
            return false;
        }
        player.update(if dt.is_finite() { dt.max(0.0) } else { 0.0 });
        self.animation_dirty.set(true);
        !player.finished
    }

    // 脏区：动画进行中返回整帧，否则无脏区。
    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.animation_dirty.replace(false) { frame } else { Rect::zero() }
    }

    // 渲染：委托给高级绘制实现。
    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.paint(ctx, frame);
    }
}
