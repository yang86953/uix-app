use std::cell::Cell;
use std::rc::Rc;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Color;
use crate::ui::animation::{AnimationConfig, TransitionPlayer};
use crate::ui::component::paint_context::PaintContext;
use crate::ui::{EventResult, MouseButton, SystemEvent, WidgetTree};

use super::super::bar_chart::BarData;
use super::super::line_chart::LineData;
use super::{
    BrushConfig, ChartKind, ChartPayload, ChartSeries, ComboSeries, FunnelAlign, FunnelShape,
    GaugeRange, GaugeType, InteractionConfig, LabelPosition, LegendPosition, LineStyle, PointStyle,
    RadarAxis, RadarData, RadarShape, RoseStyle, ScatterData, TooltipConfig, TooltipTrigger,
};

component! {
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
    }

    measure => (&self, constraints: Constraints) -> Size {
        let width = super::super::responsive_extent(self.responsive, constraints.max.w, self.width);
        let height = super::super::responsive_extent(self.responsive, constraints.max.h, self.height);
        constraints.clamp(Size::new(width, height))
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        let Some(frame) = self.last_frame.get() else {
            return EventResult::NotHandled;
        };
        match event {
            SystemEvent::PointerMove { pos, .. } => {
                let inside = frame.contains(*pos);
                if inside {
                    self.hovered_pos.set(Some(*pos));
                } else {
                    self.hovered_pos.set(None);
                }
                if let Some(start) = self.pan_start.get() {
                    let offset = (self.pan_origin.get() + pos.x - start.x).clamp(-frame.w, frame.w);
                    self.pan_offset.set(offset);
                    return EventResult::Handled;
                }
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
            SystemEvent::PointerLeave => {
                let changed = self.hovered_pos.take().is_some();
                if changed { EventResult::Handled } else { EventResult::NotHandled }
            }
            SystemEvent::PointerDown { pos, button: MouseButton::Left, .. }
                if frame.contains(*pos) =>
            {
                let mut handled = false;
                if self.interaction.as_ref().is_some_and(|config| config.pan) {
                    self.pan_start.set(Some(*pos));
                    self.pan_origin.set(self.pan_offset.get());
                    handled = true;
                }
                if self.brush_config.as_ref().is_some_and(|config| config.enabled) {
                    self.brush_start.set(Some(*pos));
                    handled = true;
                }
                if let Some(config) = self.interaction.as_ref() {
                    if let Some(callback) = config.on_click {
                        let label = self.data_label_at(*pos, frame);
                        callback(&label);
                        handled = true;
                    }
                }
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
            SystemEvent::PointerUp { pos, button: MouseButton::Left, .. } => {
                let was_panning = self.pan_start.take().is_some();
                let Some(start) = self.brush_start.take() else {
                    return if was_panning {
                        EventResult::Handled
                    } else {
                        EventResult::NotHandled
                    };
                };
                if let Some(config) = self.brush_config.as_ref().filter(|config| config.enabled) {
                    if let Some(callback) = config.on_select {
                        let start_x = ((start.x - frame.x) / frame.w.max(f32::EPSILON)).clamp(0.0, 1.0);
                        let end_x = ((pos.x - frame.x) / frame.w.max(f32::EPSILON)).clamp(0.0, 1.0);
                        callback((start_x.min(end_x), start_x.max(end_x)));
                    }
                }
                EventResult::Handled
            }
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

    wants_continuous_pointer_move => (&self) -> bool {
        self.interaction.as_ref().is_some_and(|config| config.crosshair || config.pan)
            || self.brush_config.as_ref().is_some_and(|config| config.enabled)
            || self
                .tooltip_config
                .as_ref()
                .is_some_and(|config| config.trigger_mode() == TooltipTrigger::Hover)
    }

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

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.animation_dirty.replace(false) { frame } else { Rect::zero() }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.paint(ctx, frame);
    }
}
