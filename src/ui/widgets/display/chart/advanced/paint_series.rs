use crate::core::{Point, Rect};
use crate::draw::{FillRule, PathBuilder};
use crate::ui::widget_runtime::paint_context::PaintContext;

use super::super::bar_chart::BarData;
use super::super::line_chart::LineData;
use super::{
    ChartKind, ChartPayload, ChartPlaceholder, LineStyle, PointStyle, RadarShape, TooltipTrigger,
    catmull_rom_points, finite_or_zero, normalized_ratio, palette_color,
};

// 把散点标记半径限制在绘图区可完整容纳的范围内。
fn scatter_marker_radius(raw: f32, bubble: bool, plot: Rect) -> f32 {
    let radius = if bubble {
        finite_or_zero(raw).clamp(2.0, 24.0)
    } else {
        finite_or_zero(raw).clamp(1.0, 12.0)
    };
    radius.min((plot.w.min(plot.h).max(0.0) * 0.5).max(0.0))
}

// 数据坐标使用标记半径内缩，极值点仍落在轴端但不会被组件裁剪。
fn inset_scatter_plot(plot: Rect, inset: f32) -> Rect {
    let inset = inset
        .max(0.0)
        .min(plot.w.max(0.0) * 0.5)
        .min(plot.h.max(0.0) * 0.5);
    Rect::new(
        plot.x + inset,
        plot.y + inset,
        (plot.w - inset * 2.0).max(0.0),
        (plot.h - inset * 2.0).max(0.0),
    )
}

impl ChartPlaceholder {
    /// 高级图表总入口：背景、标题、图例布局、缩放平移、分类型绘制与交互层。
    pub(crate) fn paint(&self, ctx: &mut PaintContext, frame: Rect) {
        // 空尺寸直接跳过。
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        self.last_frame.set(Some(frame));
        let bg = self.background.unwrap_or(ctx.tokens().color_bg_container());
        ctx.fill_rect(frame, bg, None);
        // 绘图区 = 帧减去内边距。
        let mut plot = Rect::new(
            frame.x + self.padding,
            frame.y + self.padding,
            (frame.w - self.padding * 2.0).max(0.0),
            (frame.h - self.padding * 2.0).max(0.0),
        );
        // 标题占据一行并下移绘图区。
        if !self.title.is_empty() {
            ctx.draw_text(
                &self.title,
                Point::new(plot.x, plot.y),
                ctx.tokens().color_text(),
                15.0,
            );
            plot.y += 20.0;
            plot.h = (plot.h - 20.0).max(0.0);
        }
        // 副标题占一行（字号更小）。
        if !self.subtitle.is_empty() && plot.h > 0.0 {
            ctx.draw_text(
                &self.subtitle,
                Point::new(plot.x, plot.y),
                ctx.tokens().color_text_secondary(),
                11.0,
            );
            plot.y += 16.0;
            plot.h = (plot.h - 16.0).max(0.0);
        }
        // 标题/副标题占满后无剩余绘图空间。
        if plot.w <= 0.0 || plot.h <= 0.0 {
            return;
        }
        // 图例切分：仅在无绘图空间时单独绘制图例。
        let (mut plot, legend_rect) = self.legend_layout(plot);
        if plot.w <= 0.0 || plot.h <= 0.0 {
            if let Some(legend_rect) = legend_rect {
                self.paint_legend(ctx, legend_rect);
            }
            return;
        }
        // 缩放：放大时水平扩展绘图区并保持居中。
        let base_width = plot.w;
        let zoom = self.zoom.get();
        if zoom > 1.0 {
            plot.w = base_width * zoom;
            plot.x += (base_width - plot.w) * 0.5;
        }
        // 平移偏移。
        plot.x += self.pan_offset.get();
        ctx.push_clip(frame);
        // 入场动画：按透明度进度裁剪绘图区宽度。
        let progress = self
            .animation_player
            .as_ref()
            .map_or(1.0, |player| player.opacity_progress.clamp(0.0, 1.0));
        let animated = self.animation_config.is_some() && progress < 1.0;
        if animated {
            ctx.push_clip(Rect::new(plot.x, plot.y, plot.w * progress, plot.h));
        }
        // 按图表类型分发到对应绘制实现。
        if self.has_data() {
            match self.kind {
                ChartKind::Bar => self.paint_bars(ctx, plot),
                ChartKind::Line | ChartKind::Area => self.paint_lines(ctx, plot),
                ChartKind::Scatter => self.paint_scatter(ctx, plot),
                ChartKind::Radar => self.paint_radar(ctx, plot),
                ChartKind::Heatmap => self.paint_heatmap(ctx, plot),
                ChartKind::Funnel => self.paint_funnel(ctx, plot),
                ChartKind::Waterfall => self.paint_waterfall(ctx, plot),
                ChartKind::Combo => self.paint_combo(ctx, plot),
                ChartKind::Treemap => self.paint_treemap(ctx, plot),
                ChartKind::Gauge => self.paint_gauge(ctx, plot),
                ChartKind::Generic => unreachable!("generic charts do not contain data"),
            }
        } else {
            // 无数据：空态提示。
            ctx.stroke_rect(plot, ctx.tokens().color_border(), 1.0, None);
            // 空态提示字号：统一使用主题 font_size_sm token。
            ctx.text_center(
                "暂无数据",
                plot,
                ctx.tokens().color_text_secondary(),
                ctx.tokens().font_size_sm(),
            );
        }
        if animated {
            ctx.pop_clip();
        }
        if let Some(legend_rect) = legend_rect {
            self.paint_legend(ctx, legend_rect);
        }
        // 交互层（框选、十字线、tooltip）在数据之上绘制。
        self.paint_interaction(ctx, frame);
        ctx.pop_clip();
    }

    /// 交互层绘制：框选矩形、十字线、悬停/点击 tooltip。
    pub(crate) fn paint_interaction(&self, ctx: &mut PaintContext, frame: Rect) {
        // 框选：从起点到当前悬浮点绘制半透明矩形。
        if let Some(start) = self.brush_start.get() {
            if let Some(end) = self.hovered_pos.get() {
                let x = start.x.min(end.x).clamp(frame.x, frame.x + frame.w);
                let y = start.y.min(end.y).clamp(frame.y, frame.y + frame.h);
                let right = start.x.max(end.x).clamp(frame.x, frame.x + frame.w);
                let bottom = start.y.max(end.y).clamp(frame.y, frame.y + frame.h);
                ctx.fill_rect(
                    Rect::new(x, y, (right - x).max(0.0), (bottom - y).max(0.0)),
                    // 框选填充：token 主色 + 固定 alpha（替换原硬编码 22,119,255，随主题换肤）。
                    ctx.tokens().color_primary().with_alpha(48),
                    None,
                );
            }
        }
        let hovered = self.hovered_pos.get().filter(|pos| frame.contains(*pos));
        // 十字线：沿悬浮点画横竖参考线。
        if self
            .interaction
            .as_ref()
            .is_some_and(|config| config.crosshair)
        {
            if let Some(pos) = hovered {
                let color = ctx.tokens().color_primary();
                ctx.draw_line(pos.x, frame.y, pos.x, frame.y + frame.h, color, 1.0);
                ctx.draw_line(frame.x, pos.y, frame.x + frame.w, pos.y, color, 1.0);
            }
        }
        // tooltip：按触发模式取位置并绘制气泡。
        if let Some(config) = &self.tooltip_config {
            let pos = match config.trigger_mode() {
                TooltipTrigger::Hover => hovered,
                TooltipTrigger::Click => self.tooltip_pos.get().filter(|pos| frame.contains(*pos)),
            };
            let Some(pos) = pos else { return };
            if let Some(text) = self.tooltip_text_at(pos, frame) {
                let size = ctx.measure_text(&text, 10.0);
                // 气泡位置偏向指针右下，越界时向内收拢。
                let x = (pos.x + 12.0).min(frame.x + frame.w - size.w - 12.0);
                let y = (pos.y - size.h - 12.0).max(frame.y + 4.0);
                let rect = Rect::new(x.max(frame.x + 4.0), y, size.w + 8.0, size.h + 8.0);
                ctx.fill_rect(rect, ctx.tokens().color_bg_elevated(), None);
                ctx.stroke_rect(rect, ctx.tokens().color_border(), 1.0, None);
                ctx.draw_text(
                    &text,
                    Point::new(rect.x + 4.0, rect.y + 4.0),
                    ctx.tokens().color_text(),
                    10.0,
                );
            }
        }
    }

    /// 坐标轴绘制：x/y 轴线、轴标题与参考线。
    pub(crate) fn paint_axes(&self, ctx: &mut PaintContext, plot: Rect) {
        let axis = ctx.tokens().color_border();
        ctx.fill_rect(
            Rect::new(plot.x, plot.y + plot.h - 1.0, plot.w, 1.0),
            axis,
            None,
        );
        ctx.fill_rect(Rect::new(plot.x, plot.y, 1.0, plot.h), axis, None);
        // x 轴标题（底部居中）。
        if !self.x_axis.is_empty() {
            ctx.text_center(
                &self.x_axis,
                Rect::new(plot.x, plot.y + plot.h, plot.w, 16.0),
                ctx.tokens().color_text_secondary(),
                10.0,
            );
        }
        // y 轴标题（左上角）。
        if !self.y_axis.is_empty() {
            ctx.draw_text(
                &self.y_axis,
                Point::new(plot.x, plot.y - 2.0),
                ctx.tokens().color_text_secondary(),
                10.0,
            );
        }
        // 参考线：按最大绝对值归一化位置，虚线分 12 段绘制。
        let max = self
            .reference_lines
            .iter()
            .map(|(value, _, _)| value.abs())
            .fold(1.0_f32, f32::max);
        for (value, label, style) in &self.reference_lines {
            let y = plot.y + plot.h - plot.h * (value / max).clamp(-1.0, 1.0).abs();
            let color = ctx.tokens().color_warning();
            let segments = if *style == LineStyle::Dashed { 12 } else { 1 };
            for segment in 0..segments {
                // 虚线跳过偶数段间隙。
                if *style == LineStyle::Dashed && segment % 2 == 1 {
                    continue;
                }
                let start = plot.x + plot.w * segment as f32 / segments as f32;
                let end = plot.x + plot.w * (segment + 1) as f32 / segments as f32;
                ctx.draw_line(start, y, end, y, color, 1.0);
            }
            if !label.is_empty() {
                ctx.draw_text(label, Point::new(plot.x + 4.0, y - 2.0), color, 9.0);
            }
        }
    }

    /// 柱状图绘制：支持分组/堆叠、横向/纵向，以及标签。
    pub(crate) fn paint_bars(&self, ctx: &mut PaintContext, plot: Rect) {
        self.paint_axes(ctx, plot);
        // 汇集载荷中的柱数据序列。
        let series: Vec<&[BarData]> = match &self.payload {
            ChartPayload::Bars(data) => vec![data.as_slice()],
            ChartPayload::BarSeries(series) => {
                series.iter().map(|series| series.data.as_slice()).collect()
            }
            ChartPayload::Empty if !self.bar_series.is_empty() => self
                .bar_series
                .iter()
                .map(|series| series.data.as_slice())
                .collect(),
            _ => Vec::new(),
        };
        let count = series.iter().map(|items| items.len()).max().unwrap_or(0);
        // 无数据或未渲染过则跳过。
        if count == 0 {
            return;
        }
        // 扫描全部值的全局最小值/最大值。
        let mut min_value: f32 = 0.0;
        let mut max_value: f32 = 0.0;
        for items in &series {
            for item in *items {
                let value = if item.value.is_finite() {
                    item.value
                } else {
                    0.0
                };
                min_value = min_value.min(value);
                max_value = max_value.max(value);
            }
        }
        let value_to_y =
            |value: f32| plot.y + plot.h - normalized_ratio(value, min_value, max_value) * plot.h;
        let value_to_x =
            |value: f32| plot.x + normalized_ratio(value, min_value, max_value) * plot.w;
        let category_w = plot.w / count as f32;
        let category_h = plot.h / count as f32;
        // 多序列且非堆叠时自动分组并列。
        let grouped = self.grouped || (!self.stacked && series.len() > 1);
        let group_count = if grouped { series.len() } else { 1 } as f32;
        // 堆叠累加器：正负值分别累积。
        let mut positive_stack = vec![0.0_f32; count];
        let mut negative_stack = vec![0.0_f32; count];
        for (series_index, items) in series.iter().enumerate() {
            for index in 0..count {
                let Some(item) = items.get(index) else {
                    continue;
                };
                let value = if item.value.is_finite() {
                    item.value
                } else {
                    0.0
                };
                // 堆叠时从累加器取起点，否则从 0 画到 value。
                let (start, end) = if self.stacked {
                    if value >= 0.0 {
                        let start = positive_stack[index];
                        positive_stack[index] += value;
                        (start, positive_stack[index])
                    } else {
                        let start = negative_stack[index];
                        negative_stack[index] += value;
                        (start, negative_stack[index])
                    }
                } else {
                    (0.0, value)
                };
                if self.horizontal {
                    // 横向柱：按行高与分组偏移定位。
                    let h = (category_h * (1.0 - self.category_gap)).max(1.0);
                    let y = plot.y
                        + index as f32 * category_h
                        + category_h * self.category_gap * 0.5
                        + if grouped {
                            series_index as f32 * h / group_count
                        } else {
                            0.0
                        };
                    let bar_h = if grouped {
                        (h / group_count) * (1.0 - self.bar_gap)
                    } else {
                        h
                    };
                    let x = value_to_x(start.min(end));
                    let right = value_to_x(start.max(end));
                    ctx.fill_rect(
                        Rect::new(x, y, (right - x).max(0.0), bar_h),
                        item.color,
                        None,
                    );
                    // 首个序列绘制类别标签。
                    if series_index == 0 {
                        ctx.draw_text(
                            &item.label,
                            Point::new(plot.x + plot.w + 4.0, y + bar_h * 0.5),
                            ctx.tokens().color_text_secondary(),
                            10.0,
                        );
                    }
                } else {
                    // 纵向柱：按列宽与分组偏移定位。
                    let w = (category_w * (1.0 - self.category_gap)).max(1.0);
                    let x = plot.x
                        + index as f32 * category_w
                        + category_w * self.category_gap * 0.5
                        + if grouped {
                            series_index as f32 * w / group_count
                        } else {
                            0.0
                        };
                    let bar_w = if grouped {
                        (w / group_count) * (1.0 - self.bar_gap)
                    } else {
                        w
                    };
                    let y = value_to_y(start.max(end));
                    let bottom = value_to_y(start.min(end));
                    ctx.fill_rect(
                        Rect::new(x, y, bar_w, (bottom - y).max(0.0)),
                        item.color,
                        None,
                    );
                    // 首个序列绘制类别标签。
                    if series_index == 0 {
                        ctx.text_center(
                            &item.label,
                            Rect::new(x, plot.y + plot.h + 2.0, bar_w, 14.0),
                            ctx.tokens().color_text_secondary(),
                            10.0,
                        );
                    }
                }
            }
        }
    }

    /// 折线/面积图绘制：支持堆叠、阶梯、平滑与面积填充。
    pub(crate) fn paint_lines(&self, ctx: &mut PaintContext, plot: Rect) {
        self.paint_axes(ctx, plot);
        // 汇集载荷中的线数据序列。
        let series: Vec<Vec<LineData>> = match &self.payload {
            ChartPayload::Lines(data) => vec![data.clone()],
            ChartPayload::LineSeries(series) => {
                series.iter().map(|series| series.data.clone()).collect()
            }
            ChartPayload::Empty if !self.line_series.is_empty() => self
                .line_series
                .iter()
                .map(|series| series.data.clone())
                .collect(),
            _ => Vec::new(),
        };
        let count = series.iter().map(Vec::len).max().unwrap_or(0);
        if count == 0 {
            return;
        }
        // 预计算每序列的上下边界（堆叠时为累计值）。
        let mut lower_values = vec![vec![0.0_f32; count]; series.len()];
        let mut upper_values = vec![vec![0.0_f32; count]; series.len()];
        let mut min_value = 0.0_f32;
        let mut max_value = 0.0_f32;
        for (series_index, data) in series.iter().enumerate() {
            for index in 0..count {
                let raw = data.get(index).map_or(0.0, |item| item.value);
                let value = if raw.is_finite() { raw } else { 0.0 };
                // 堆叠时下界取前一序列上界。
                let lower = if self.stacked && series_index > 0 {
                    upper_values[series_index - 1][index]
                } else {
                    0.0
                };
                let upper = if self.stacked { lower + value } else { value };
                lower_values[series_index][index] = if self.stacked { lower } else { 0.0 };
                upper_values[series_index][index] = upper;
                min_value = min_value.min(lower.min(upper));
                max_value = max_value.max(lower.max(upper));
            }
        }
        // 索引与值映射为屏幕坐标（单点居中）。
        let to_point = |index: usize, value: f32| {
            let x = if count == 1 {
                plot.x + plot.w * 0.5
            } else {
                plot.x + index as f32 * plot.w / (count - 1) as f32
            };
            Point::new(
                x,
                plot.y + plot.h - normalized_ratio(value, min_value, max_value) * plot.h,
            )
        };
        for (series_index, data) in series.iter().enumerate() {
            let points = (0..data.len().min(count))
                .map(|index| to_point(index, upper_values[series_index][index]))
                .collect::<Vec<_>>();
            if points.is_empty() {
                continue;
            }
            let color = palette_color(series_index);
            // 面积图：闭合上沿与下沿路径并填充。
            if self.kind == ChartKind::Area && points.len() >= 2 {
                let mut path = PathBuilder::new();
                path.move_to(points[0].x, points[0].y);
                for point in points.iter().skip(1) {
                    path.line_to(point.x, point.y);
                }
                for index in (0..points.len()).rev() {
                    let lower = to_point(index, lower_values[series_index][index]);
                    path.line_to(lower.x, lower.y);
                }
                path.close();
                // 面积填充：在系列色上应用 fill_opacity 透明度（用 with_alpha 收敛 alpha 混合）。
                let fill = color.with_alpha((255.0 * self.fill_opacity) as u8);
                ctx.fill_path(&path.build(), fill, FillRule::NonZero);
            }
            if self.step {
                // 阶梯线：先水平后垂直。
                for pair in points.windows(2) {
                    ctx.draw_line(pair[0].x, pair[0].y, pair[1].x, pair[0].y, color, 2.0);
                    ctx.draw_line(pair[1].x, pair[0].y, pair[1].x, pair[1].y, color, 2.0);
                }
            } else {
                // 平滑时对点集做 Catmull-Rom 插值后连线。
                let line_points = if self.smooth {
                    catmull_rom_points(&points, 8)
                } else {
                    points.clone()
                };
                for pair in line_points.windows(2) {
                    ctx.draw_line(pair[0].x, pair[0].y, pair[1].x, pair[1].y, color, 2.0);
                }
            }
            // 数据点标记。
            for point in points {
                ctx.fill_circle(point.x, point.y, self.point_size, color);
            }
        }
    }

    /// 散点/气泡图绘制：按数据极值归一化坐标，支持多种点样式。
    pub(crate) fn paint_scatter(&self, ctx: &mut PaintContext, plot: Rect) {
        self.paint_axes(ctx, plot);
        // 汇集 (x, y, 半径, 是否气泡) 元组序列。
        let series: Vec<Vec<(f32, f32, f32, bool)>> = match &self.payload {
            ChartPayload::Scatter(data) => vec![
                data.iter()
                    .map(|item| (item.x, item.y, self.point_size, false))
                    .collect(),
            ],
            ChartPayload::Bubble(data) => vec![
                data.iter()
                    .map(|item| (item.x, item.y, item.size * self.bubble_scale, true))
                    .collect(),
            ],
            ChartPayload::ScatterSeries(series) => series
                .iter()
                .map(|series| {
                    series
                        .data
                        .iter()
                        .map(|item| (item.x, item.y, self.point_size, false))
                        .collect()
                })
                .collect(),
            _ => Vec::new(),
        };
        // 过滤非法点并计算 x/y 极值。
        let points = series
            .iter()
            .flatten()
            .filter(|point| point.0.is_finite() && point.1.is_finite())
            .collect::<Vec<_>>();
        if points.is_empty() {
            return;
        }
        let min_x = points
            .iter()
            .map(|point| point.0)
            .fold(f32::INFINITY, f32::min);
        let max_x = points
            .iter()
            .map(|point| point.0)
            .fold(f32::NEG_INFINITY, f32::max);
        let min_y = points
            .iter()
            .map(|point| point.1)
            .fold(f32::INFINITY, f32::min);
        let max_y = points
            .iter()
            .map(|point| point.1)
            .fold(f32::NEG_INFINITY, f32::max);
        let dx = (max_x - min_x).max(1.0);
        let dy = (max_y - min_y).max(1.0);
        // 坐标极值需要为最大标记预留完整半径，避免气泡贴边时只剩半圆。
        let marker_inset = points
            .iter()
            .map(|point| scatter_marker_radius(point.2, point.3, plot))
            .fold(0.0f32, f32::max);
        let data_plot = inset_scatter_plot(plot, marker_inset);
        for (series_index, data) in series.iter().enumerate() {
            let point_color = palette_color(series_index);
            for (x, y, radius, bubbles) in data {
                if !x.is_finite() || !y.is_finite() {
                    continue;
                }
                // 坐标线性映射到绘图区（y 轴翻转）。
                let px = data_plot.x + (x - min_x) / dx * data_plot.w;
                let py = data_plot.y + data_plot.h - (y - min_y) / dy * data_plot.h;
                // 半径与内缩计算共享同一上限，保证最终像素全部留在绘图区内。
                let radius = scatter_marker_radius(*radius, *bubbles, plot);
                match self.point_style {
                    PointStyle::Circle => ctx.fill_circle(px, py, radius, point_color),
                    PointStyle::Diamond => {
                        // 菱形以矩形近似。
                        ctx.fill_rect(
                            Rect::new(px - radius, py - radius, radius * 2.0, radius * 2.0),
                            point_color,
                            None,
                        );
                    }
                    PointStyle::Cross => {
                        ctx.draw_line(px - radius, py, px + radius, py, point_color, 2.0);
                        ctx.draw_line(px, py - radius, px, py + radius, point_color, 2.0);
                    }
                }
            }
        }
    }

    /// 雷达图绘制：网格（多边形/圆形）、轴线标签与各系列数据多边形。
    pub(crate) fn paint_radar(&self, ctx: &mut PaintContext, plot: Rect) {
        // 轴数取轴列表与各系列数据的最大长度（至少 3）。
        let count = self
            .radar_axes
            .len()
            .max(
                self.radar_series
                    .iter()
                    .map(|series| series.data.len())
                    .max()
                    .unwrap_or(0),
            )
            .max(3);
        let center = Point::new(plot.x + plot.w * 0.5, plot.y + plot.h * 0.5);
        let radius = plot.w.min(plot.h) * 0.38;
        let grid = ctx.tokens().color_border();
        // 分层网格：圆形或正多边形。
        for level in 1..=self.grid_levels.max(1) {
            let r = radius * level as f32 / self.grid_levels.max(1) as f32;
            if self.radar_shape == RadarShape::Circle {
                ctx.stroke_circle(center.x, center.y, r, grid, 1.0);
            } else {
                for axis in 0..count {
                    let a = -std::f32::consts::FRAC_PI_2
                        + axis as f32 * std::f32::consts::TAU / count as f32;
                    let next = -std::f32::consts::FRAC_PI_2
                        + (axis + 1) as f32 * std::f32::consts::TAU / count as f32;
                    ctx.draw_line(
                        center.x + r * a.cos(),
                        center.y + r * a.sin(),
                        center.x + r * next.cos(),
                        center.y + r * next.sin(),
                        grid,
                        1.0,
                    );
                }
            }
        }
        // 轴射线与轴标签（自顶部顺时针均分）。
        for axis in 0..count {
            let angle =
                -std::f32::consts::FRAC_PI_2 + axis as f32 * std::f32::consts::TAU / count as f32;
            ctx.draw_line(
                center.x,
                center.y,
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
                grid,
                1.0,
            );
            if let Some(label) = self.radar_axes.get(axis).map(|axis| axis.label.as_str()) {
                let label_center = Point::new(
                    center.x + (radius + 12.0) * angle.cos(),
                    center.y + (radius + 12.0) * angle.sin(),
                );
                let size = ctx.measure_text(label, 9.0);
                ctx.draw_text(
                    label,
                    Point::new(label_center.x - size.w * 0.5, label_center.y),
                    ctx.tokens().color_text_secondary(),
                    9.0,
                );
            }
        }
        // 各系列：按轴范围归一化值并闭合为多边形。
        for (series_index, series) in self.radar_series.iter().enumerate() {
            let points = series
                .data
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    // 取该轴配置的范围，缺省 [0, 100]。
                    let range = self
                        .radar_axes
                        .get(index)
                        .map(|axis| {
                            let start = if axis.range.start().is_finite() {
                                *axis.range.start()
                            } else {
                                0.0
                            };
                            let end = if axis.range.end().is_finite() {
                                *axis.range.end()
                            } else {
                                100.0
                            };
                            (start, end)
                        })
                        .unwrap_or((0.0, 100.0));
                    let value = if value.value.is_finite() {
                        value.value
                    } else {
                        range.0
                    };
                    let range_min = range.0.min(range.1);
                    let range_max = range.0.max(range.1);
                    let ratio = normalized_ratio(value, range_min, range_max);
                    let a = -std::f32::consts::FRAC_PI_2
                        + index as f32 * std::f32::consts::TAU / count as f32;
                    Point::new(
                        center.x + radius * ratio * a.cos(),
                        center.y + radius * ratio * a.sin(),
                    )
                })
                .collect::<Vec<_>>();
            if points.is_empty() {
                continue;
            }
            let color = palette_color(series_index);
            // 三点以上填充闭合多边形，否则退化为点。
            if points.len() >= 3 {
                let mut path = PathBuilder::new();
                path.move_to(points[0].x, points[0].y);
                for point in points.iter().skip(1) {
                    path.line_to(point.x, point.y);
                }
                path.close();
                // 面积填充：在系列色上应用 fill_opacity 透明度（用 with_alpha 收敛 alpha 混合）。
                let fill = color.with_alpha((255.0 * self.fill_opacity) as u8);
                ctx.fill_path(&path.build(), fill, FillRule::NonZero);
            } else {
                ctx.fill_circle(points[0].x, points[0].y, 3.0, color);
            }
            // 首尾闭合的轮廓线。
            for pair in points
                .iter()
                .chain(points.first())
                .collect::<Vec<_>>()
                .windows(2)
            {
                ctx.draw_line(pair[0].x, pair[0].y, pair[1].x, pair[1].y, color, 1.5);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 极值数据的坐标区应按最大气泡半径从四边内缩。
    #[test]
    fn bubble_plot_inset_keeps_marker_inside_frame() {
        let plot = Rect::new(10.0, 20.0, 100.0, 80.0);
        let radius = scatter_marker_radius(80.0, true, plot);
        assert_eq!(radius, 24.0);
        assert_eq!(
            inset_scatter_plot(plot, radius),
            Rect::new(34.0, 44.0, 52.0, 32.0)
        );
    }
}
