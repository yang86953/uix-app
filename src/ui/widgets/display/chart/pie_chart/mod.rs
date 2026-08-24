//! PieChart — pie / donut chart with proportional circular sectors.

use std::cell::Cell;

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Color;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{EventResult, MouseButton, SnapshotFields, SystemEvent, WidgetTree};
use crate::widget;

use super::advanced::{
    BrushConfig, InteractionConfig, LabelPosition, LegendPosition, RoseStyle, TooltipConfig,
    TooltipDatum, TooltipTrigger,
};
use super::value_label::ChartValueLabel;

mod presentation;
use presentation::*;

#[cfg(test)]
#[path = "../../../../../../tests/unit/ui/widgets/display/chart/pie_chart/tests.rs"]
mod tests;

// 使用一个字节记录会覆盖 UIX 默认值的 Rust 调用方声明。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct PieChartAuthored(u8);

impl PieChartAuthored {
    const DONUT: u8 = 1 << 0;
    const LABEL_VISIBLE: u8 = 1 << 1;
    const LABEL_POSITION: u8 = 1 << 2;
    const LEGEND: u8 = 1 << 3;
    const PADDING: u8 = 1 << 4;

    fn contains(self, flag: u8) -> bool {
        self.0 & flag != 0
    }

    fn set(&mut self, flag: u8, authored: bool) {
        if authored {
            self.0 |= flag;
        } else {
            self.0 &= !flag;
        }
    }
}

// 将环图内部标签限制在内外圆之间；窄环无法容纳完整文字时返回空值。
fn safe_donut_label_radius(
    preferred: f32,
    hole_radius: f32,
    outer_radius: f32,
    radial_half_extent: f32,
    label_gap: f32,
) -> Option<f32> {
    let min_radius = hole_radius + radial_half_extent + label_gap;
    let max_radius = outer_radius - radial_half_extent - label_gap;
    (min_radius <= max_radius).then(|| preferred.clamp(min_radius, max_radius))
}

#[derive(Debug, Clone, PartialEq)]
/// 饼图中的单个分类扇区数据。
pub struct PieData {
    /// 显示在标签、图例和提示框中的分类名称。
    pub label: String,
    /// 用于计算扇区占比的原始数值。
    pub value: f32,
    /// 绘制该扇区时使用的颜色。
    pub color: Color,
}

impl PieData {
    /// 使用分类名称、数值和扇区颜色创建数据项。
    pub fn new(label: impl Into<String>, value: f32, color: Color) -> Self {
        Self {
            label: label.into(),
            value,
            color,
        }
    }
}

widget! {
    /// 按具名数据项绘制饼图、环图或南丁格尔玫瑰图的组件。
    pub struct PieChart {
        data: Vec<PieData>,
        fixed_size: f32,
        hole_radius: f32,
        rose: bool,
        rose_style: RoseStyle,
        start_angle: f32,
        end_angle: f32,
        total: Option<f32>,
        label_visible: bool,
        label_position: LabelPosition,
        legend: LegendPosition,
        background: Option<Color>,
        padding: f32,
        title: String,
        subtitle: String,
        responsive: bool,
        interaction: Option<InteractionConfig>,
        brush_config: Option<BrushConfig>,
        tooltip_config: Option<TooltipConfig>,
        animation_enabled: bool,
        #[snapshot(skip)]
        last_frame: Cell<Option<Rect>>,
        #[snapshot(skip)]
        hovered_pos: Cell<Option<Point>>,
        #[snapshot(skip)]
        tooltip_pos: Cell<Option<Point>>,
        #[snapshot(skip)]
        brush_start: Cell<Option<Point>>,
        #[snapshot(skip)]
        pan_start: Cell<Option<Point>>,
        #[snapshot(skip)]
        pan_origin: Cell<f32>,
        #[snapshot(skip)]
        pan_offset: Cell<f32>,
        #[snapshot(skip)]
        zoom: Cell<f32>,
        #[snapshot(skip)]
        visual: &'static PieChartVisual,
        #[snapshot(skip)]
        authored: PieChartAuthored,
    }

    measure => (&self, constraints: Constraints) -> Size {
        let intrinsic = self.intrinsic_size();
        let width = super::responsive_extent(self.responsive, constraints.max.w, intrinsic.w);
        let height = super::responsive_extent(self.responsive, constraints.max.h, intrinsic.h);
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
                    self.pan_offset.set(
                        (self.pan_origin.get() + pos.x - start.x).clamp(-frame.w, frame.w),
                    );
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
                        callback(self.data_label_at(*pos, frame));
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

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let Some(summary) = self.data_summary() else { return; };
        if frame.w <= 0.0 || frame.h <= 0.0 { return; }
        self.last_frame.set(Some(frame));
        let resolved = self.visual.resolve(ctx.tokens());
        let layout = self.visual.layout;
        let label_visual = self.visual.label;
        let chrome = self.visual.chrome;
        let typography = self.visual.typography;

        ctx.fill_rect(
            frame,
            self.background.unwrap_or(resolved.background),
            None,
        );
        let mut content = Rect::new(
            frame.x + self.padding,
            frame.y + self.padding,
            (frame.w - self.padding * 2.0).max(0.0),
            (frame.h - self.padding * 2.0).max(0.0),
        );
        if !self.title.is_empty() {
            ctx.draw_text(
                &self.title,
                Point::new(content.x, content.y),
                resolved.text,
                typography.title,
            );
            content.y += layout.title_height;
            content.h = (content.h - layout.title_height).max(0.0);
        }
        if !self.subtitle.is_empty() && content.h > 0.0 {
            ctx.draw_text(
                &self.subtitle,
                Point::new(content.x, content.y),
                resolved.text_secondary,
                typography.subtitle,
            );
            content.y += layout.subtitle_height;
            content.h = (content.h - layout.subtitle_height).max(0.0);
        }
        if content.w <= 0.0 || content.h <= 0.0 { return; }

        let legend_rect = self.reserve_legend(&mut content, summary);
        let chart_r = ((content.w.min(content.h) * layout.center_ratio
            - layout.chart_edge_inset)
            * self.zoom.get())
        .max(0.0);
        if chart_r <= 0.0 { return; }

        let cx = content.x + content.w * layout.center_ratio + self.pan_offset.get();
        let cy = content.y + content.h * layout.center_ratio;

        let font_size = (chart_r * label_visual.font_ratio).max(label_visual.minimum_font_size);
        let start = self.start_angle.to_radians() - std::f32::consts::FRAC_PI_2;
        let sweep = self.sweep_degrees().to_radians();
        let mut sa = start;
        let rose_max_fraction = summary.max_fraction();
        ctx.push_clip(frame);

        for d in self.valid_data() {
            let fraction = summary.fraction(d.value);
            let angle_fraction = if self.rose {
                1.0 / summary.count as f32
            } else {
                fraction
            };
            let a = angle_fraction * sweep;
            let ea = sa + a;
            let ma = sa + a * 0.5;
            let radius = self.slice_radius(fraction, rose_max_fraction, chart_r);
            ctx.fill_sector(cx, cy, radius, sa, ea, d.color);

            if self.label_visible
                && a.abs() > std::f32::consts::TAU * label_visual.minimum_sweep_ratio
            {
                let half_a = a * 0.5;
                let centroid_r = if half_a > 0.001 {
                    radius * label_visual.centroid_factor * half_a.sin() / half_a
                } else {
                    radius * label_visual.centroid_factor
                };
                let percent = fraction * 100.0;
                if percent >= label_visual.minimum_percent {
                    let value_label = Self::format_value(f64::from(percent.round()))
                        .with_suffix(typography.percent_suffix);
                    let value_size = ctx.measure_text(value_label.as_str(), font_size);
                    let label_width = value_size.w;
                    let (sin_ma, cos_ma) = ma.sin_cos();
                    let label_r = match self.label_position {
                        LabelPosition::Inside if self.hole_radius > 0.0 => {
                            // 按当前角度投影文字半外框，保证整个标签不与白色内环相交。
                            let radial_half_extent = cos_ma.abs()
                                * label_width
                                * layout.center_ratio
                                + sin_ma.abs()
                                    * font_size
                                    * label_visual.radial_font_extent_ratio;
                            let Some(label_r) = safe_donut_label_radius(
                                centroid_r,
                                chart_r * self.hole_radius,
                                radius,
                                radial_half_extent,
                                label_visual.donut_gap,
                            ) else {
                                sa = ea;
                                continue;
                            };
                            label_r
                        }
                        LabelPosition::Inside => centroid_r,
                        LabelPosition::Outside | LabelPosition::Right => {
                            radius + label_visual.outside_offset
                        }
                    };
                    let lx = cx + label_r * cos_ma;
                    let ly = cy + label_r * sin_ma;
                    let text_x = lx - label_width * layout.center_ratio;
                    let text_rect = Rect::new(
                        text_x,
                        ly - font_size * label_visual.radial_font_extent_ratio,
                        label_width,
                        font_size * label_visual.line_height_ratio,
                    );
                    let text_y = ctx.visual_center_y(text_rect, font_size);
                    ctx.draw_text(
                        value_label.as_str(),
                        Point::new(text_x, text_y),
                        resolved.white,
                        font_size,
                    );
                }
            }
            sa = ea;
        }
        if self.hole_radius > 0.0 {
            let hr = chart_r * self.hole_radius;
            ctx.fill_circle(cx, cy, hr, resolved.background);
            ctx.stroke_circle(cx, cy, hr, resolved.border, chrome.hole_border);

            let center_label = Self::format_value(summary.total);
            let cl_fs = hr * label_visual.center_font_ratio;
            let cl_y = ctx.visual_center_y(Rect::new(cx - hr, cy - hr, hr * 2.0, hr * 2.0), cl_fs);
            let cl_sz = ctx.measure_text(center_label.as_str(), cl_fs);
            ctx.draw_text(
                center_label.as_str(),
                Point::new(cx - cl_sz.w * layout.center_ratio, cl_y),
                resolved.text,
                cl_fs,
            );
        }

        if let Some(legend_rect) = legend_rect {
            self.paint_legend(ctx, legend_rect, summary, resolved);
        }
        if self
            .interaction
            .as_ref()
            .is_some_and(|config| config.crosshair)
        {
            if let Some(pos) = self.hovered_pos.get().filter(|pos| frame.contains(*pos)) {
                ctx.stroke_circle(
                    pos.x,
                    pos.y,
                    chrome.crosshair_radius,
                    resolved.primary,
                    chrome.crosshair_stroke,
                );
            }
        }
        if let (Some(start), Some(end)) = (self.brush_start.get(), self.hovered_pos.get()) {
            let x = start.x.min(end.x).clamp(frame.x, frame.x + frame.w);
            let y = start.y.min(end.y).clamp(frame.y, frame.y + frame.h);
            let right = start.x.max(end.x).clamp(frame.x, frame.x + frame.w);
            let bottom = start.y.max(end.y).clamp(frame.y, frame.y + frame.h);
            ctx.fill_rect(
                Rect::new(x, y, (right - x).max(0.0), (bottom - y).max(0.0)),
                resolved.primary.with_alpha(chrome.brush_alpha),
                None,
            );
        }
        if let Some(config) = &self.tooltip_config {
            let pos = match config.trigger_mode() {
                TooltipTrigger::Hover => self.hovered_pos.get(),
                TooltipTrigger::Click => self.tooltip_pos.get(),
            };
            if let Some(pos) = pos.filter(|pos| frame.contains(*pos)) {
                self.paint_tooltip(ctx, frame, pos, resolved);
            }
        }
        ctx.pop_clip();
    }
}

// 把数据/交互状态与 UIX 静态视觉融合为单一 PieChart 根节点。
fn build_pie_chart_view(mut kernel: PieChart, visual: &'static PieChartVisual) -> ViewNode {
    if !kernel.authored.contains(PieChartAuthored::DONUT) {
        kernel.hole_radius = visual.defaults.hole_radius;
    }
    if !kernel.authored.contains(PieChartAuthored::LABEL_VISIBLE) {
        kernel.label_visible = visual.defaults.label_visible;
    }
    if !kernel.authored.contains(PieChartAuthored::LABEL_POSITION) {
        kernel.label_position = visual.defaults.label_position;
    }
    if !kernel.authored.contains(PieChartAuthored::LEGEND) {
        kernel.legend = visual.defaults.legend;
    }
    if !kernel.authored.contains(PieChartAuthored::PADDING) {
        kernel.padding = visual.defaults.padding;
    }
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

// 让声明式 View 构建统一进入同目录 UIX 根。
fn build_pie_chart_uix_root(kernel: PieChart) -> ViewNode {
    crate::uix!("src/ui/widgets/display/chart/pie_chart/pie_chart.uix")
}

impl View for PieChart {
    fn build(self) -> ViewNode {
        build_pie_chart_uix_root(self)
    }
}

#[cfg(test)]
mod label_geometry_tests {
    use super::safe_donut_label_radius;

    // 环图标签必须完整位于白色内环与扇区外缘之间。
    #[test]
    fn donut_label_radius_avoids_hole_and_outer_edge() {
        let radius =
            safe_donut_label_radius(40.0, 44.0, 80.0, 10.0, 2.0).expect("当前环宽足以容纳标签");
        assert_eq!(radius, 56.0);
        assert!(radius - 10.0 > 44.0);
        assert!(radius + 10.0 < 80.0);
    }

    // 环宽不足时不得把标签画进白色内环。
    #[test]
    fn narrow_donut_skips_unsafe_inside_label() {
        assert_eq!(safe_donut_label_radius(48.0, 44.0, 54.0, 6.0, 2.0), None);
    }
}

// 保存一次线性扫描得到的有效数据摘要，后续绘制和命中只借用原数据。
#[derive(Debug, Clone, Copy, PartialEq)]
struct PieDataSummary {
    total: f64,
    max_value: f32,
    count: usize,
    has_label: bool,
}

impl PieDataSummary {
    fn fraction(self, value: f32) -> f32 {
        (f64::from(value) / self.total) as f32
    }

    fn max_fraction(self) -> f32 {
        self.fraction(self.max_value).max(f32::EPSILON)
    }
}

// 保存命中与绘制共享的圆心和外半径。
#[derive(Debug, Clone, Copy, PartialEq)]
struct PieChartGeometry {
    center: Point,
    radius: f32,
}

impl Default for PieChart {
    fn default() -> Self {
        Self::new()
    }
}
impl PieChart {
    /// 创建空的完整饼图，并默认显示内部标签和右侧图例。
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            fixed_size: 0.0,
            hole_radius: PIE_CHART_VISUAL.defaults.hole_radius,
            rose: false,
            rose_style: RoseStyle::Radius,
            start_angle: 0.0,
            end_angle: 360.0,
            total: None,
            label_visible: PIE_CHART_VISUAL.defaults.label_visible,
            label_position: PIE_CHART_VISUAL.defaults.label_position,
            legend: PIE_CHART_VISUAL.defaults.legend,
            background: None,
            padding: PIE_CHART_VISUAL.defaults.padding,
            title: String::new(),
            subtitle: String::new(),
            responsive: false,
            interaction: None,
            brush_config: None,
            tooltip_config: None,
            animation_enabled: false,
            last_frame: Cell::new(None),
            hovered_pos: Cell::new(None),
            tooltip_pos: Cell::new(None),
            brush_start: Cell::new(None),
            pan_start: Cell::new(None),
            pan_origin: Cell::new(0.0),
            pan_offset: Cell::new(0.0),
            zoom: Cell::new(1.0),
            visual: PIE_CHART_VISUAL_REF,
            authored: PieChartAuthored::default(),
        }
    }
    /// 替换饼图的全部分类数据。
    pub fn data(mut self, d: Vec<PieData>) -> Self {
        self.data = d;
        self
    }
    /// 设置固定正方形边长；非法或非正值恢复为默认尺寸。
    pub fn size(mut self, s: f32) -> Self {
        self.fixed_size = if s.is_finite() && s > 0.0 { s } else { 0.0 };
        self
    }
    /// 设置圆孔半径占图表半径的比例，并夹取到零至 0.9。
    pub fn donut(mut self, r: f32) -> Self {
        self.hole_radius = if r.is_finite() {
            r.clamp(0.0, 0.9)
        } else {
            0.0
        };
        self.authored.set(PieChartAuthored::DONUT, true);
        self
    }

    /// 设置是否用等角扇区和数据驱动半径呈现南丁格尔玫瑰图。
    pub fn rose(mut self, value: bool) -> Self {
        self.rose = value;
        self
    }
    /// 设置玫瑰图按半径或面积映射数据占比。
    pub fn rose_style(mut self, style: RoseStyle) -> Self {
        self.rose_style = style;
        self
    }
    /// 设置扇区扫描的起始角度；非有限输入被忽略。
    pub fn start_angle(mut self, angle: f32) -> Self {
        if angle.is_finite() {
            self.start_angle = angle;
        }
        self
    }
    /// 设置未指定显式扫描终点时使用的结束角度。
    pub fn end_angle(mut self, angle: f32) -> Self {
        if angle.is_finite() {
            self.end_angle = angle;
        }
        self
    }
    /// 设置替代 `end_angle` 的显式扫描终止角；非有限输入被忽略。
    pub fn total(mut self, total: f32) -> Self {
        if total.is_finite() {
            self.total = Some(total);
        }
        self
    }
    /// 设置是否绘制各扇区的数据标签。
    pub fn label_visible(mut self, visible: bool) -> Self {
        self.label_visible = visible;
        self.authored.set(PieChartAuthored::LABEL_VISIBLE, true);
        self
    }
    /// 设置数据标签相对扇区的位置。
    pub fn label_position(mut self, position: LabelPosition) -> Self {
        self.label_position = position;
        self.authored.set(PieChartAuthored::LABEL_POSITION, true);
        self
    }
    /// 设置图例相对绘图区的保留位置。
    pub fn legend(mut self, position: LegendPosition) -> Self {
        self.legend = position;
        self.authored.set(PieChartAuthored::LEGEND, true);
        self
    }
    /// 设置图表内容区域的背景颜色。
    pub fn bg(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }
    /// 设置图表内容内边距；非有限值归零，负值夹取为零。
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = if padding.is_finite() {
            padding.max(0.0)
        } else {
            0.0
        };
        self.authored.set(PieChartAuthored::PADDING, true);
        self
    }
    /// 设置显示在绘图区上方的标题。
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }
    /// 设置显示在标题下方的副标题。
    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = subtitle.into();
        self
    }
    /// 设置图表是否使用父约束提供的响应式尺寸。
    pub fn responsive(mut self, responsive: bool) -> Self {
        self.responsive = responsive;
        self
    }
    /// 启用并配置缩放、平移或数据点击交互。
    pub fn interactive(mut self, config: InteractionConfig) -> Self {
        self.interaction = Some(config);
        self
    }
    /// 启用并配置绘图区范围刷选交互。
    pub fn brush(mut self, config: BrushConfig) -> Self {
        self.brush_config = Some(config);
        self
    }
    /// 启用并配置扇区数据提示框。
    pub fn tooltip(mut self, config: TooltipConfig) -> Self {
        self.tooltip_config = Some(config);
        self
    }
    /// 启用图表动画；当前保留配置参数供后续动画策略使用。
    pub fn animation(mut self, _animation: crate::ui::animation::AnimationConfig) -> Self {
        self.animation_enabled = true;
        self
    }

    fn intrinsic_size(&self) -> Size {
        let size = if self.fixed_size > 0.0 {
            self.fixed_size
        } else {
            self.visual.defaults.size
        };
        Size::new(size, size)
    }

    fn reserve_legend(&self, content: &mut Rect, summary: PieDataSummary) -> Option<Rect> {
        let layout = self.visual.layout;
        if self.legend == LegendPosition::None
            || !summary.has_label
            || matches!(self.legend, LegendPosition::Left | LegendPosition::Right)
                && content.w < layout.legend_side_min_content
        {
            return None;
        }
        match self.legend {
            LegendPosition::Top => {
                let height = layout.legend_row_height.min(content.h);
                let rect = Rect::new(content.x, content.y, content.w, height);
                content.y += height;
                content.h = (content.h - height).max(0.0);
                Some(rect)
            }
            LegendPosition::Bottom => {
                let height = layout.legend_row_height.min(content.h);
                let rect = Rect::new(content.x, content.y + content.h - height, content.w, height);
                content.h = (content.h - height).max(0.0);
                Some(rect)
            }
            LegendPosition::Left | LegendPosition::Right => {
                let width = (content.w * layout.legend_side_ratio)
                    .clamp(layout.legend_side_min, layout.legend_side_max)
                    .min(content.w);
                let x = if self.legend == LegendPosition::Left {
                    let x = content.x;
                    content.x += width;
                    x
                } else {
                    content.x + content.w - width
                };
                content.w = (content.w - width).max(0.0);
                Some(Rect::new(x, content.y, width, content.h))
            }
            LegendPosition::None => None,
        }
    }

    fn legend_entry_width(&self, ctx: &mut PaintContext, data: &PieData, fraction: f32) -> f32 {
        let typography = self.visual.typography;
        let label = data.label.trim();
        let percent =
            Self::format_value(f64::from(fraction) * 100.0).with_suffix(typography.percent_suffix);
        let mut width = ctx.measure_text(percent.as_str(), typography.legend).w;
        if !label.is_empty() {
            width += ctx.measure_text(label, typography.legend).w
                + ctx
                    .measure_text(typography.label_separator, typography.legend)
                    .w;
        }
        width
    }

    // 直接绘制标签、空格、百分比和后缀，避免构造临时 String。
    fn paint_legend_entry(
        &self,
        ctx: &mut PaintContext,
        data: &PieData,
        fraction: f32,
        mut x: f32,
        y: f32,
        color: Color,
    ) -> f32 {
        let typography = self.visual.typography;
        let start = x;
        let label = data.label.trim();
        if !label.is_empty() {
            ctx.draw_text(label, Point::new(x, y), color, typography.legend);
            x += ctx.measure_text(label, typography.legend).w;
            ctx.draw_text(
                typography.label_separator,
                Point::new(x, y),
                color,
                typography.legend,
            );
            x += ctx
                .measure_text(typography.label_separator, typography.legend)
                .w;
        }
        let percent =
            Self::format_value(f64::from(fraction) * 100.0).with_suffix(typography.percent_suffix);
        ctx.draw_text(percent.as_str(), Point::new(x, y), color, typography.legend);
        x + ctx.measure_text(percent.as_str(), typography.legend).w - start
    }

    fn paint_legend(
        &self,
        ctx: &mut PaintContext,
        rect: Rect,
        summary: PieDataSummary,
        resolved: ResolvedPieChartVisual,
    ) {
        let layout = self.visual.layout;
        let typography = self.visual.typography;
        if matches!(self.legend, LegendPosition::Top | LegendPosition::Bottom) {
            let separator_width = ctx
                .measure_text(typography.legend_separator, typography.legend)
                .w;
            let entries_width = self
                .valid_data()
                .map(|data| self.legend_entry_width(ctx, data, summary.fraction(data.value)))
                .sum::<f32>();
            let gaps = separator_width * summary.count.saturating_sub(1) as f32;
            let mut x = rect.x + (rect.w - entries_width - gaps).max(0.0) * layout.center_ratio;
            let y = ctx.visual_center_y(rect, typography.legend);
            for (index, data) in self.valid_data().enumerate() {
                if index > 0 {
                    ctx.draw_text(
                        typography.legend_separator,
                        Point::new(x, y),
                        resolved.text,
                        typography.legend,
                    );
                    x += separator_width;
                }
                x += self.paint_legend_entry(
                    ctx,
                    data,
                    summary.fraction(data.value),
                    x,
                    y,
                    resolved.text,
                );
            }
            return;
        }
        for (index, data) in self.valid_data().enumerate() {
            let y = rect.y + index as f32 * layout.legend_item_height;
            if y + layout.legend_item_height > rect.y + rect.h {
                break;
            }
            ctx.fill_rect(
                Rect::new(
                    rect.x + layout.legend_swatch_x,
                    y + layout.legend_swatch_y,
                    layout.legend_swatch_size,
                    layout.legend_swatch_size,
                ),
                data.color,
                None,
            );
            self.paint_legend_entry(
                ctx,
                data,
                summary.fraction(data.value),
                rect.x + layout.legend_text_x,
                y + layout.legend_text_y,
                resolved.text,
            );
        }
    }

    fn valid_data(&self) -> impl Iterator<Item = &PieData> {
        self.data
            .iter()
            .filter(|item| item.value.is_finite() && item.value > 0.0)
    }

    fn data_summary(&self) -> Option<PieDataSummary> {
        let mut total = 0.0;
        let mut max_value = 0.0_f32;
        let mut count = 0_usize;
        let mut has_label = false;
        for data in self.valid_data() {
            total += f64::from(data.value);
            max_value = max_value.max(data.value);
            count += 1;
            has_label |= !data.label.trim().is_empty();
        }
        (count > 0 && total.is_finite() && total > 0.0).then_some(PieDataSummary {
            total,
            max_value,
            count,
            has_label,
        })
    }

    fn sweep_degrees(&self) -> f32 {
        (self.total.unwrap_or(self.end_angle) - self.start_angle).clamp(-360.0, 360.0)
    }

    fn chart_geometry(&self, frame: Rect, summary: PieDataSummary) -> Option<PieChartGeometry> {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return None;
        }
        let layout = self.visual.layout;
        let mut content = Rect::new(
            frame.x + self.padding,
            frame.y + self.padding,
            (frame.w - self.padding * 2.0).max(0.0),
            (frame.h - self.padding * 2.0).max(0.0),
        );
        if !self.title.is_empty() {
            content.y += layout.title_height;
            content.h = (content.h - layout.title_height).max(0.0);
        }
        if !self.subtitle.is_empty() {
            content.y += layout.subtitle_height;
            content.h = (content.h - layout.subtitle_height).max(0.0);
        }
        let _ = self.reserve_legend(&mut content, summary);
        let radius = ((content.w.min(content.h) * layout.center_ratio - layout.chart_edge_inset)
            * self.zoom.get())
        .max(0.0);
        (radius > 0.0).then_some(PieChartGeometry {
            center: Point::new(
                content.x + content.w * layout.center_ratio + self.pan_offset.get(),
                content.y + content.h * layout.center_ratio,
            ),
            radius,
        })
    }

    fn data_at(&self, pos: Point, frame: Rect) -> Option<(&PieData, f32)> {
        let summary = self.data_summary()?;
        let geometry = self.chart_geometry(frame, summary)?;
        let dx = pos.x - geometry.center.x;
        let dy = pos.y - geometry.center.y;
        let distance = (dx * dx + dy * dy).sqrt();
        if distance < geometry.radius * self.hole_radius || distance > geometry.radius {
            return None;
        }

        let sweep = self.sweep_degrees().to_radians();
        let sweep_abs = sweep.abs();
        if sweep_abs <= f32::EPSILON {
            return None;
        }
        let start = self.start_angle.to_radians() - std::f32::consts::FRAC_PI_2;
        let angle = (dy.atan2(dx) - start).rem_euclid(std::f32::consts::TAU);
        let angle = if sweep < 0.0 {
            (std::f32::consts::TAU - angle).rem_euclid(std::f32::consts::TAU)
        } else {
            angle
        };
        if angle > sweep_abs {
            return None;
        }

        let rose_max_fraction = summary.max_fraction();
        let mut cursor = 0.0;
        for data in self.valid_data() {
            let fraction = summary.fraction(data.value);
            let angle_fraction = if self.rose {
                1.0 / summary.count as f32
            } else {
                fraction
            };
            cursor += angle_fraction * sweep_abs;
            if angle <= cursor {
                let radius = self.slice_radius(fraction, rose_max_fraction, geometry.radius);
                return if distance <= radius {
                    Some((data, fraction))
                } else {
                    None
                };
            }
        }
        None
    }

    fn data_label_at(&self, pos: Point, frame: Rect) -> &str {
        self.data_at(pos, frame)
            .map_or("", |(data, _)| data.label.as_str())
    }

    fn slice_radius(&self, fraction: f32, max_fraction: f32, chart_radius: f32) -> f32 {
        if !self.rose {
            return chart_radius;
        }
        let ratio = (fraction / max_fraction.max(f32::EPSILON)).clamp(0.0, 1.0);
        let factor = match self.rose_style {
            RoseStyle::Radius => ratio,
            RoseStyle::Area => ratio.sqrt(),
        };
        chart_radius * factor
    }

    fn format_value(value: f64) -> ChartValueLabel {
        ChartValueLabel::from_f64(value)
    }

    fn paint_tooltip(
        &self,
        ctx: &mut PaintContext,
        frame: Rect,
        pos: Point,
        resolved: ResolvedPieChartVisual,
    ) {
        let Some(config) = &self.tooltip_config else {
            return;
        };
        let Some(datum) = self.tooltip_datum_at(pos, frame) else {
            return;
        };
        let text = config.format(&datum);
        let typography = self.visual.typography;
        let chrome = self.visual.chrome;
        let size = ctx.measure_text(&text, typography.tooltip);
        let x =
            (pos.x + chrome.tooltip_offset).min(frame.x + frame.w - size.w - chrome.tooltip_offset);
        let y = (pos.y - size.h - chrome.tooltip_offset).max(frame.y + chrome.tooltip_edge_inset);
        let rect = Rect::new(
            x.max(frame.x + chrome.tooltip_edge_inset),
            y,
            size.w + chrome.tooltip_padding * 2.0,
            size.h + chrome.tooltip_padding * 2.0,
        );
        ctx.fill_rect(rect, resolved.elevated, None);
        ctx.stroke_rect(rect, resolved.border, chrome.tooltip_border, None);
        ctx.draw_text(
            &text,
            Point::new(
                rect.x + chrome.tooltip_padding,
                rect.y + chrome.tooltip_padding,
            ),
            resolved.text,
            typography.tooltip,
        );
    }

    fn tooltip_datum_at(&self, pos: Point, frame: Rect) -> Option<TooltipDatum> {
        let (data, fraction) = self.data_at(pos, frame)?;
        Some(TooltipDatum {
            label: data.label.clone(),
            value: Some(data.value),
            percentage: Some(fraction),
            ..TooltipDatum::default()
        })
    }

    // 测试目标保留饼图切片观测入口，供图表数据测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn slices_for_test(&self) -> Vec<(String, f32)> {
        let Some(summary) = self.data_summary() else {
            return Vec::new();
        };
        self.valid_data()
            .map(|data| (data.label.clone(), summary.fraction(data.value)))
            .collect()
    }

    // 测试目标保留饼图总扫掠角观测入口，供图表几何测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn sweep_degrees_for_test(&self) -> f32 {
        self.sweep_degrees()
    }

    // 测试目标保留饼图扇区几何观测入口，供图表布局测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn sector_geometry_for_test(&self) -> Vec<(f32, f32)> {
        let Some(summary) = self.data_summary() else {
            return Vec::new();
        };
        let max_fraction = summary.max_fraction();
        self.valid_data()
            .map(|data| {
                let fraction = summary.fraction(data.value);
                let angle_fraction = if self.rose {
                    1.0 / summary.count as f32
                } else {
                    fraction
                };
                (
                    angle_fraction * self.sweep_degrees(),
                    self.slice_radius(fraction, max_fraction, 1.0),
                )
            })
            .collect()
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let last_frame = self.last_frame.get();
        let hovered_pos = self.hovered_pos.get();
        let tooltip_pos = self.tooltip_pos.get();
        let pan_offset = self.pan_offset.get();
        let zoom = self.zoom.get();
        let tracks_pointer = next.interaction.is_some()
            || next.brush_config.is_some()
            || next.tooltip_config.is_some();
        let keeps_pan = next.interaction.as_ref().is_some_and(|config| config.pan);
        let keeps_zoom = next.interaction.as_ref().is_some_and(|config| config.zoom);
        let keeps_click_tooltip = next
            .tooltip_config
            .as_ref()
            .is_some_and(|config| config.trigger_mode() == TooltipTrigger::Click);

        *self = next;
        self.last_frame.set(last_frame);
        self.hovered_pos
            .set(tracks_pointer.then_some(hovered_pos).flatten());
        self.tooltip_pos
            .set(keeps_click_tooltip.then_some(tooltip_pos).flatten());
        self.brush_start.set(None);
        self.pan_start.set(None);
        self.pan_origin.set(0.0);
        self.pan_offset
            .set(if keeps_pan { pan_offset } else { 0.0 });
        self.zoom.set(if keeps_zoom { zoom } else { 1.0 });
    }

    // 测试目标保留饼图 tooltip 文本观测入口，供交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn tooltip_text_for_test(&self, pos: Point, frame: Rect) -> Option<String> {
        let config = self.tooltip_config.as_ref()?;
        Some(config.format(&self.tooltip_datum_at(pos, frame)?))
    }

    // 测试目标保留饼图交互状态观测入口，供交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn interaction_state_for_test(&self) -> (f32, f32) {
        (self.zoom.get(), self.pan_offset.get())
    }

    // 测试目标保留饼图 tooltip 位置观测入口，供交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn tooltip_position_for_test(&self) -> Option<Point> {
        self.tooltip_pos.get()
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::PieChart {
            data: self.data.clone(),
            fixed_size: self.fixed_size,
            hole_radius: self.hole_radius,
        }
    }
}
