use crate::core::{Point, Rect};
// 高级图表主体只直接构造填充规则与路径。
use crate::draw::{FillRule, PathBuilder};
use crate::ui::widget_runtime::paint_context::PaintContext;

use super::super::bar_chart::BarData;
use super::super::line_chart::LineData;
use super::{
    AxisSide, ChartPayload, ChartPlaceholder, ChartType, FunnelAlign, FunnelShape, GaugeType,
    LabelPosition, LineStyle, MAX_HEATMAP_DIMENSION, TooltipTrigger, TreemapNode, WaterfallKind,
    calendar_day_count, is_leap_year, january_first_weekday, normalized_ratio, palette_color,
};

impl ChartPlaceholder {
    /// 热力图绘制：普通网格或日历模式，含坐标标签与数值。
    pub(crate) fn paint_heatmap(&self, ctx: &mut PaintContext, plot: Rect) {
        let ChartPayload::Heatmap(cells) = &self.payload else {
            return;
        };
        // 日历模式：把天数换算为 (周列, 星期行)；普通模式直接取网格坐标。
        let leap = is_leap_year(self.year);
        let calendar_days = calendar_day_count(self.year);
        let calendar_offset = january_first_weekday(self.year);
        let calendar = if self.calendar_mode {
            cells
                .iter()
                .filter(|cell| cell.x < calendar_days)
                .map(|cell| {
                    let ordinal = calendar_offset + cell.x;
                    (ordinal / 7, ordinal % 7, cell.value)
                })
                .collect::<Vec<_>>()
        } else {
            cells
                .iter()
                .map(|cell| (cell.x, cell.y, cell.value))
                .collect::<Vec<_>>()
        };
        // 网格列数：日历按周数，普通模式按最大 x（受限）。
        let max_x = if self.calendar_mode {
            (calendar_offset + calendar_days - 1) / 7 + 1
        } else {
            calendar
                .iter()
                .map(|cell| cell.0)
                .max()
                .unwrap_or(0)
                .saturating_add(1)
                .min(MAX_HEATMAP_DIMENSION)
        };
        // 网格行数：日历固定 7，普通模式按最大 y（受限）。
        let max_y = if self.calendar_mode {
            7
        } else {
            calendar
                .iter()
                .map(|cell| cell.1)
                .max()
                .unwrap_or(0)
                .saturating_add(1)
                .min(MAX_HEATMAP_DIMENSION)
        };
        // 全局最大值（至少为 1 避免除零）。
        let max = cells
            .iter()
            .map(|cell| {
                if cell.value.is_finite() {
                    cell.value
                } else {
                    0.0
                }
            })
            .fold(0.0_f32, f32::max)
            .max(1.0);
        // 坐标标签占用区域。
        let label_left = if self.y_labels.is_empty() { 0.0 } else { 42.0 };
        let label_bottom = if self.x_labels.is_empty() { 0.0 } else { 20.0 };
        let cells_plot = Rect::new(
            plot.x + label_left,
            plot.y,
            (plot.w - label_left).max(0.0),
            (plot.h - label_bottom).max(0.0),
        );
        // 单元格尺寸：日历模式用配置尺寸，普通模式自适应铺满。
        let configured = if self.calendar_mode {
            self.cell_size
        } else {
            f32::INFINITY
        };
        let w = ((cells_plot.w - self.cell_gap * max_x as f32) / max_x as f32)
            .min(configured)
            .max(0.0);
        let h = if self.calendar_mode {
            ((cells_plot.h - self.cell_gap * max_y as f32) / max_y as f32)
                .min(configured)
                .max(0.0)
        } else {
            ((cells_plot.h - self.cell_gap * max_y as f32) / max_y as f32).max(0.0)
        };
        // 逐个绘制单元格：按归一化值取色。
        for (x, y, value) in &calendar {
            if *x >= max_x || *y >= max_y {
                continue;
            }
            let finite_value = if value.is_finite() { *value } else { 0.0 };
            let t = (finite_value / max).clamp(0.0, 1.0);
            let color = self.heatmap_color(t);
            let rect = Rect::new(
                cells_plot.x + *x as f32 * (w + self.cell_gap),
                cells_plot.y + *y as f32 * (h + self.cell_gap),
                w.max(0.0),
                h.max(0.0),
            );
            ctx.fill_rect(rect, color, None);
            // 数值标签（深色块上反白）：白色 token。
            if self.show_values {
                ctx.text_center(
                    &format!("{}", finite_value),
                    rect,
                    ctx.tokens().color_white(),
                    9.0,
                );
            }
        }
        // x 轴标签（底部）。
        for (index, label) in self.x_labels.iter().take(max_x).enumerate() {
            let rect = Rect::new(
                cells_plot.x + index as f32 * (w + self.cell_gap),
                cells_plot.y + cells_plot.h + 2.0,
                w,
                label_bottom,
            );
            ctx.text_center(label, rect, ctx.tokens().color_text_secondary(), 9.0);
        }
        // y 轴标签（左侧）。
        for (index, label) in self.y_labels.iter().take(max_y).enumerate() {
            let rect = Rect::new(
                plot.x,
                cells_plot.y + index as f32 * (h + self.cell_gap),
                label_left - 4.0,
                h,
            );
            ctx.text_center(label, rect, ctx.tokens().color_text_secondary(), 9.0);
        }
        // 日历模式：每月首行绘制「N月」标签（2 月按闰年处理）。
        if self.calendar_mode {
            let month_lengths = [31_usize, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
            let mut day = 0_usize;
            for (month, length) in month_lengths.into_iter().enumerate() {
                let length = if month == 1 && leap { 29 } else { length };
                let week = (calendar_offset + day) / 7;
                let label = format!("{}月", month + 1);
                ctx.draw_text(
                    &label,
                    Point::new(
                        cells_plot.x + week as f32 * (w + self.cell_gap),
                        cells_plot.y - 12.0,
                    ),
                    ctx.tokens().color_text_secondary(),
                    9.0,
                );
                day += length;
            }
        }
    }

    /// 漏斗图绘制：按值比例收窄的梯形，支持对齐、对称与转化率。
    pub(crate) fn paint_funnel(&self, ctx: &mut PaintContext, plot: Rect) {
        let ChartPayload::Funnel(data) = &self.payload else {
            return;
        };
        // 最大值（至少 1）用于归一化宽度。
        let max = data
            .iter()
            .map(|item| {
                if item.value.is_finite() {
                    item.value.max(0.0)
                } else {
                    0.0
                }
            })
            .fold(1.0, f32::max);
        // 段高与段间间隙（间隙不超过单段高）。
        let gap = self
            .funnel_gap
            .min(plot.h / data.len().max(1) as f32)
            .max(0.0);
        let h = (plot.h - gap * data.len().saturating_sub(1) as f32) / data.len().max(1) as f32;
        for (index, item) in data.iter().enumerate() {
            let value = if item.value.is_finite() {
                item.value.max(0.0)
            } else {
                0.0
            };
            // 本段宽度与下一段宽度（最小 5%）。
            let width = plot.w * (value / max).clamp(0.05, 1.0);
            let next_width = data.get(index + 1).map_or(0.05, |item| {
                let value = if item.value.is_finite() {
                    item.value.max(0.0)
                } else {
                    0.0
                };
                (value / max).clamp(0.05, 1.0)
            }) * plot.w;
            // 对称形状强制居中，否则按对齐配置。
            let align = if self.funnel_shape == FunnelShape::Symmetric {
                FunnelAlign::Center
            } else {
                self.funnel_align
            };
            let x = match align {
                FunnelAlign::Center => plot.x + (plot.w - width) * 0.5,
                FunnelAlign::Left => plot.x,
                FunnelAlign::Right => plot.x + plot.w - width,
            };
            let y = plot.y + index as f32 * (h + gap);
            let bottom_x = match align {
                FunnelAlign::Center => plot.x + (plot.w - next_width) * 0.5,
                FunnelAlign::Left => plot.x,
                FunnelAlign::Right => plot.x + plot.w - next_width,
            };
            // 梯形路径：上宽下窄。
            let mut path = PathBuilder::new();
            path.move_to(x, y);
            path.line_to(x + width, y);
            path.line_to(bottom_x + next_width, y + h.max(0.0));
            path.line_to(bottom_x, y + h.max(0.0));
            path.close();
            let funnel_color = ctx.tokens().color_primary();
            ctx.fill_path(&path.build(), funnel_color, FillRule::NonZero);
            // 标签：可含转化率（相对上一段）。
            if self.label_visible {
                let mut label = format!("{} {}", item.label, value);
                if self.show_conversion_rate && index > 0 {
                    let previous = if data[index - 1].value.is_finite() {
                        data[index - 1].value.abs().max(f32::EPSILON)
                    } else {
                        f32::EPSILON
                    };
                    label.push_str(&format!(" ({:.0}%)", value / previous * 100.0));
                }
                let label_rect = Rect::new(x, y, width, h);
                // 右侧标签绘制在梯形外，否则居中。
                if self.label_position == LabelPosition::Right {
                    ctx.draw_text(
                        &label,
                        Point::new(x + width + 4.0, y + h * 0.5),
                        ctx.tokens().color_text(),
                        10.0,
                    );
                } else {
                    // 轴标签（深色块上反白）：白色 token。
                    ctx.text_center(&label, label_rect, ctx.tokens().color_white(), 10.0);
                }
            }
        }
    }

    /// 瀑布图绘制：增量/减量/总计三种段的起止范围与连接线。
    pub(crate) fn paint_waterfall(&self, ctx: &mut PaintContext, plot: Rect) {
        let ChartPayload::Waterfall(data) = &self.payload else {
            return;
        };
        if data.is_empty() {
            return;
        }
        let plot = self.paint_axes(ctx, plot);
        // 逐个计算段的起止值并跟踪累计值与全局范围。
        let mut ranges = Vec::with_capacity(data.len());
        let mut cumulative = 0.0;
        let mut min_value = 0.0_f32;
        let mut max_value = 0.0_f32;
        for item in data {
            let value = if item.value.is_finite() {
                item.value
            } else {
                0.0
            };
            // 总计段从 0 开始；其余段从累计值开始。
            let start = match item.kind {
                WaterfallKind::Total => 0.0,
                _ => cumulative,
            };
            let end = match item.kind {
                WaterfallKind::Total => value,
                _ => cumulative + value,
            };
            cumulative = end;
            min_value = min_value.min(start.min(end));
            max_value = max_value.max(start.max(end));
            ranges.push((start, end));
        }
        let to_x = |value: f32| plot.x + normalized_ratio(value, min_value, max_value) * plot.w;
        let to_y =
            |value: f32| plot.y + plot.h - normalized_ratio(value, min_value, max_value) * plot.h;
        let category_w = plot.w / data.len() as f32;
        let category_h = plot.h / data.len() as f32;
        let bar_w = category_w * 0.7;
        let bar_h = category_h * 0.7;
        for (index, (item, (start, end))) in data.iter().zip(ranges.iter()).enumerate() {
            // 按段类型着色：增/减/总计。
            let color = match item.kind {
                WaterfallKind::Increase => ctx.tokens().color_success(),
                WaterfallKind::Decrease => ctx.tokens().color_error(),
                WaterfallKind::Total => ctx.tokens().color_primary(),
            };
            let rect = if self.horizontal {
                let x = to_x((*start).min(*end));
                Rect::new(
                    x,
                    plot.y + index as f32 * category_h + category_h * 0.15,
                    (to_x((*start).max(*end)) - x).max(1.0),
                    bar_h.max(1.0),
                )
            } else {
                let y = to_y((*start).max(*end));
                Rect::new(
                    plot.x + index as f32 * category_w + category_w * 0.15,
                    y,
                    bar_w.max(1.0),
                    (to_y((*start).min(*end)) - y).max(1.0),
                )
            };
            ctx.fill_rect(rect, color, None);
            // 类别标签。
            if self.horizontal {
                ctx.draw_text(
                    &item.label,
                    Point::new(rect.x + rect.w + 4.0, rect.y + rect.h * 0.5),
                    ctx.tokens().color_text_secondary(),
                    9.0,
                );
            } else {
                ctx.text_center(
                    &item.label,
                    Rect::new(rect.x, plot.y + plot.h + 2.0, rect.w, 14.0),
                    ctx.tokens().color_text_secondary(),
                    9.0,
                );
            }
            // 段间连接线（从本段末端到下一段起点）。
            if let Some((next_start, _)) = ranges.get(index + 1) {
                let connector_color = ctx.tokens().color_border();
                if self.horizontal {
                    let y = rect.y + rect.h;
                    ctx.draw_line(
                        rect.x + rect.w,
                        y,
                        rect.x + rect.w,
                        plot.y + (index + 1) as f32 * category_h + category_h * 0.15,
                        connector_color,
                        1.0,
                    );
                    let _ = next_start;
                } else {
                    let x = rect.x + rect.w;
                    ctx.draw_line(
                        x,
                        to_y(*end),
                        plot.x + (index + 1) as f32 * category_w + category_w * 0.15,
                        to_y(*next_start),
                        connector_color,
                        1.0,
                    );
                }
            }
        }
    }

    /// 组合图绘制：柱 + 线/面积按左右轴共存，虚线/面积样式可选。
    pub(crate) fn paint_combo(&self, ctx: &mut PaintContext, plot: Rect) {
        let plot = self.paint_axes(ctx, plot);
        // 右侧轴标题。
        if !self.y_axis_right.is_empty() {
            let size = ctx.measure_text(&self.y_axis_right, 10.0);
            ctx.draw_text(
                &self.y_axis_right,
                Point::new(plot.x + plot.w - size.w, plot.y - 16.0),
                ctx.tokens().color_text_secondary(),
                10.0,
            );
        }
        // 汇集柱/线数据：优先组合系列，否则回退到独立系列。
        let mut bars: Vec<(String, Vec<BarData>, AxisSide)> = Vec::new();
        let mut lines: Vec<(String, Vec<LineData>, AxisSide, LineStyle, bool)> = Vec::new();
        if !self.combo_series.is_empty() {
            for (index, series) in self.combo_series.iter().enumerate() {
                match series.chart_type {
                    ChartType::Bar => bars.push((
                        series.name.clone(),
                        series
                            .data
                            .iter()
                            .map(|item| BarData::new(&item.label, item.value, palette_color(index)))
                            .collect(),
                        series.axis,
                    )),
                    ChartType::Line | ChartType::Area => lines.push((
                        series.name.clone(),
                        series.data.clone(),
                        series.axis,
                        series.line_style,
                        series.chart_type == ChartType::Area,
                    )),
                }
            }
        } else {
            bars.extend(
                self.bar_series
                    .iter()
                    .map(|series| (series.name.clone(), series.data.clone(), AxisSide::Left)),
            );
            // 有线或右轴标题时线走右轴，否则左轴。
            let line_axis = if !self.y_axis_right.is_empty() || !self.bar_series.is_empty() {
                AxisSide::Right
            } else {
                AxisSide::Left
            };
            lines.extend(self.line_series.iter().map(|series| {
                (
                    series.name.clone(),
                    series.data.clone(),
                    line_axis,
                    LineStyle::Solid,
                    false,
                )
            }));
        }
        let count = bars
            .iter()
            .map(|(_, data, _)| data.len())
            .chain(lines.iter().map(|(_, data, _, _, _)| data.len()))
            .max()
            .unwrap_or(0);
        if count == 0 {
            return;
        }
        // 按轴分别统计值域。
        let range_for = |axis: AxisSide| {
            let mut min = 0.0_f32;
            let mut max = 0.0_f32;
            for (_, data, item_axis) in &bars {
                if *item_axis == axis {
                    for item in data {
                        let value = if item.value.is_finite() {
                            item.value
                        } else {
                            0.0
                        };
                        min = min.min(value);
                        max = max.max(value);
                    }
                }
            }
            for (_, data, item_axis, _, _) in &lines {
                if *item_axis == axis {
                    for item in data {
                        let value = if item.value.is_finite() {
                            item.value
                        } else {
                            0.0
                        };
                        min = min.min(value);
                        max = max.max(value);
                    }
                }
            }
            (min, max.max(min + f32::EPSILON))
        };
        let left_range = range_for(AxisSide::Left);
        let right_range = range_for(AxisSide::Right);
        let to_y = |value: f32, axis: AxisSide| {
            let (min, max) = if axis == AxisSide::Right {
                right_range
            } else {
                left_range
            };
            plot.y + plot.h - normalized_ratio(value, min, max) * plot.h
        };
        let category_w = plot.w / count as f32;
        let bar_count = bars.len().max(1) as f32;
        // 柱：分组并列，从各自轴基线绘制。
        for (bar_index, (_, data, axis)) in bars.iter().enumerate() {
            for (index, item) in data.iter().enumerate() {
                let value = if item.value.is_finite() {
                    item.value
                } else {
                    0.0
                };
                let w = category_w * (1.0 - self.category_gap);
                let x = plot.x
                    + index as f32 * category_w
                    + category_w * self.category_gap * 0.5
                    + bar_index as f32 * w / bar_count;
                let bar_w = (w / bar_count * (1.0 - self.bar_gap)).max(1.0);
                let y = to_y(value, *axis);
                let baseline = to_y(0.0, *axis);
                let rect = Rect::new(x, y.min(baseline), bar_w, (baseline - y).abs().max(1.0));
                ctx.fill_rect(rect, item.color, None);
                // 首个序列绘制类别标签。
                if bar_index == 0 {
                    ctx.text_center(
                        &item.label,
                        Rect::new(x, plot.y + plot.h + 2.0, bar_w, 14.0),
                        ctx.tokens().color_text_secondary(),
                        9.0,
                    );
                }
            }
        }
        // 线/面积：按轴映射点坐标。
        for (line_index, (_, data, axis, line_style, area)) in lines.iter().enumerate() {
            let points = data
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    let value = if item.value.is_finite() {
                        item.value
                    } else {
                        0.0
                    };
                    Point::new(
                        if count == 1 {
                            plot.x + plot.w * 0.5
                        } else {
                            plot.x + index as f32 * category_w + category_w * 0.5
                        },
                        to_y(value, *axis),
                    )
                })
                .collect::<Vec<_>>();
            if points.is_empty() {
                continue;
            }
            let color = palette_color(bars.len() + line_index);
            // 面积填充：闭合到基线。
            if *area && points.len() >= 2 {
                let baseline = to_y(0.0, *axis);
                let mut path = PathBuilder::new();
                path.move_to(points[0].x, baseline);
                for point in &points {
                    path.line_to(point.x, point.y);
                }
                if let Some(last) = points.last() {
                    path.line_to(last.x, baseline);
                }
                path.close();
                ctx.fill_path(
                    &path.build(),
                    // 面积填充：在系列色上应用 fill_opacity 透明度（用 with_alpha 收敛 alpha 混合）。
                    color.with_alpha((255.0 * self.fill_opacity) as u8),
                    FillRule::NonZero,
                );
            }
            // 连线：虚线按 8 段交替绘制。
            for pair in points.windows(2) {
                if *line_style == LineStyle::Dashed {
                    for segment in 0..8 {
                        if segment % 2 == 1 {
                            continue;
                        }
                        let start = segment as f32 / 8.0;
                        let end = (segment + 1) as f32 / 8.0;
                        let from = Point::new(
                            pair[0].x + (pair[1].x - pair[0].x) * start,
                            pair[0].y + (pair[1].y - pair[0].y) * start,
                        );
                        let to = Point::new(
                            pair[0].x + (pair[1].x - pair[0].x) * end,
                            pair[0].y + (pair[1].y - pair[0].y) * end,
                        );
                        ctx.draw_line(from.x, from.y, to.x, to.y, color, 2.0);
                    }
                } else {
                    ctx.draw_line(pair[0].x, pair[0].y, pair[1].x, pair[1].y, color, 2.0);
                }
            }
            // 数据点标记。
            for point in points {
                ctx.fill_circle(point.x, point.y, self.point_size, color);
            }
        }
    }

    /// 矩形树图入口：从根节点列表开始递归分块。
    pub(crate) fn paint_treemap(&self, ctx: &mut PaintContext, plot: Rect) {
        let ChartPayload::Treemap(nodes) = &self.payload else {
            return;
        };
        self.paint_treemap_nodes(ctx, nodes, plot, 0);
    }

    /// 矩形树递归分块：奇偶深度交替水平/垂直切分，权重取子节点总和。
    pub(crate) fn paint_treemap_nodes(
        &self,
        ctx: &mut PaintContext,
        nodes: &[TreemapNode],
        plot: Rect,
        depth: usize,
    ) {
        if nodes.is_empty() || plot.w <= 0.0 || plot.h <= 0.0 {
            return;
        }
        // 节点权重：有子节点取子节点值之和，否则取自身值。
        let total = nodes
            .iter()
            .map(|node| {
                let child_total = node
                    .children
                    .iter()
                    .map(|child| child.value.max(0.0))
                    .sum::<f32>();
                if child_total > 0.0 {
                    child_total
                } else {
                    node.value.max(0.0)
                }
            })
            .sum::<f32>()
            .max(f32::EPSILON);
        // 深度交替方向避免长条矩形。
        let horizontal = depth.is_multiple_of(2);
        let mut cursor = if horizontal { plot.x } else { plot.y };
        for (index, node) in nodes.iter().enumerate() {
            let weight = {
                let child_total = node
                    .children
                    .iter()
                    .map(|child| child.value.max(0.0))
                    .sum::<f32>();
                if child_total > 0.0 {
                    child_total
                } else {
                    node.value.max(0.0)
                }
            };
            // 按权重分配主方向上的跨度。
            let extent = if horizontal {
                plot.w * weight / total
            } else {
                plot.h * weight / total
            };
            let raw = if horizontal {
                Rect::new(cursor, plot.y, extent, plot.h)
            } else {
                Rect::new(plot.x, cursor, plot.w, extent)
            };
            // 内部留间隙。
            let gap = self.treemap_gap.min(raw.w * 0.5).min(raw.h * 0.5);
            let rect = Rect::new(
                raw.x + gap * 0.5,
                raw.y + gap * 0.5,
                (raw.w - gap).max(0.0),
                (raw.h - gap).max(0.0),
            );
            // 按深度与索引取色。
            let color = palette_color(depth * nodes.len() + index);
            ctx.fill_rect(rect, color, None);
            if node.children.is_empty() {
                // 叶子：居中显示标签（深色块上反白）：白色 token。
                if self.label_visible {
                    ctx.text_center(&node.label, rect, ctx.tokens().color_white(), 11.0);
                }
            } else {
                // 分支：顶部显示标签并递归分块子节点。
                if self.label_visible {
                    ctx.draw_text(
                        &node.label,
                        Point::new(rect.x + 4.0, rect.y + 12.0),
                        // 分支标题（深色块上反白）：白色 token。
                        ctx.tokens().color_white(),
                        10.0,
                    );
                }
                let inner = Rect::new(
                    rect.x + 2.0,
                    rect.y + 16.0,
                    rect.w - 4.0,
                    (rect.h - 18.0).max(0.0),
                );
                self.paint_treemap_nodes(ctx, &node.children, inner, depth + 1);
            }
            cursor += extent;
        }
    }

    /// 仪表盘绘制：扇区/环、分段色、指针与读数标签。
    pub(crate) fn paint_gauge(&self, ctx: &mut PaintContext, plot: Rect) {
        let center = Point::new(plot.x + plot.w * 0.5, plot.y + plot.h * 0.56);
        let radius = plot.w.min(plot.h) * 0.38;
        // 仪表盘类型决定起始角与扫过角度。
        let (start, sweep) = match self.gauge_type {
            GaugeType::Dashboard => (std::f32::consts::PI, std::f32::consts::PI),
            GaugeType::Full | GaugeType::Ring => {
                (-std::f32::consts::FRAC_PI_2, std::f32::consts::TAU)
            }
        };
        let range_min = self.gauge_min.min(self.gauge_max);
        let range_max = self.gauge_min.max(self.gauge_max);
        let ratio = normalized_ratio(self.gauge_value, range_min, range_max);
        let bg = ctx.tokens().color_fill_tertiary();
        // 底色扇区。
        ctx.fill_sector(center.x, center.y, radius, start, start + sweep, bg);
        if self.gauge_ranges.is_empty() {
            // 无分段：单色按比例填充。
            let color = ctx.tokens().color_primary();
            ctx.fill_sector(
                center.x,
                center.y,
                radius,
                start,
                start + sweep * ratio,
                color,
            );
        } else {
            // 分段：按各段范围与当前值裁剪填充。
            for segment in &self.gauge_ranges {
                if !segment.start.is_finite() || !segment.end.is_finite() {
                    continue;
                }
                let raw_start = normalized_ratio(segment.start, range_min, range_max);
                let raw_end = normalized_ratio(segment.end, range_min, range_max);
                let segment_start = raw_start.min(raw_end);
                let segment_end = raw_start.max(raw_end);
                let end = segment_end.min(ratio);
                if end > segment_start {
                    ctx.fill_sector(
                        center.x,
                        center.y,
                        radius,
                        start + sweep * segment_start,
                        start + sweep * end,
                        segment.color,
                    );
                }
            }
        }
        // 环型：中心掏空。
        if self.gauge_type == GaugeType::Ring {
            ctx.fill_circle(
                center.x,
                center.y,
                radius * 0.62,
                ctx.tokens().color_bg_container(),
            );
        }
        // 指针：从中心指向当前角度。
        if self.pointer_width > 0.0 {
            let angle = start + sweep * ratio;
            let pointer_color = self.pointer_color.unwrap_or(ctx.tokens().color_text());
            ctx.draw_line(
                center.x,
                center.y,
                center.x + radius * 0.9 * angle.cos(),
                center.y + radius * 0.9 * angle.sin(),
                pointer_color,
                self.pointer_width,
            );
            ctx.fill_circle(center.x, center.y, self.pointer_width * 1.5, pointer_color);
        }
        // 读数标签（可自定义格式化）。
        let label = self
            .value_format
            .as_ref()
            .map(|format| format(self.gauge_value))
            .unwrap_or_else(|| format!("{:.0}", self.gauge_value));
        ctx.text_center(
            &label,
            Rect::new(center.x - radius, center.y - 12.0, radius * 2.0, 24.0),
            ctx.tokens().color_text(),
            16.0,
        );
    }

    /// 快照同步：保留交互/动画运行时状态，避免更新组件时打断体验。
    pub(crate) fn sync_from(&mut self, mut next: Self) {
        // 先保存运行时状态。
        let last_frame = self.last_frame.get();
        let hovered_pos = self.hovered_pos.get();
        let tooltip_pos = self.tooltip_pos.get();
        let pan_offset = self.pan_offset.get();
        let zoom = self.zoom.get();
        // 动画配置不变则保留播放器进度，否则换用新播放器。
        let same_animation = self.animation_config == next.animation_config;
        let animation_player = if same_animation {
            self.animation_player.take()
        } else {
            next.animation_player.take()
        };
        let animation_dirty = if same_animation {
            self.animation_dirty.get()
        } else {
            next.animation_dirty.get()
        };
        // 新配置是否仍需要指针跟踪/平移/缩放/点击 tooltip。
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
        // 按新配置是否仍启用对应交互来决定保留或复位运行时状态。
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
        self.animation_player = animation_player;
        self.animation_dirty.set(animation_dirty);
    }

    // 测试目标保留组合图表类型观测入口，供图表语义测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn kind_for_test(&self) -> &'static str {
        use super::ChartKind;
        match self.kind {
            ChartKind::Generic => "generic",
            ChartKind::Bar => "bar",
            ChartKind::Line => "line",
            ChartKind::Area => "area",
            ChartKind::Scatter => "scatter",
            ChartKind::Radar => "radar",
            ChartKind::Heatmap => "heatmap",
            ChartKind::Funnel => "funnel",
            ChartKind::Waterfall => "waterfall",
            ChartKind::Combo => "combo",
            ChartKind::Treemap => "treemap",
            ChartKind::Gauge => "gauge",
        }
    }

    // 测试目标保留组合图表 tooltip 文本观测入口，供交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn tooltip_text_for_test(&self, pos: Point, frame: Rect) -> Option<String> {
        self.tooltip_text_at(pos, frame)
    }

    // 测试目标保留组合图表交互状态观测入口，供交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn interaction_state_for_test(&self) -> (f32, f32) {
        (self.zoom.get(), self.pan_offset.get())
    }

    // 测试目标保留组合图表 tooltip 位置观测入口，供交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn tooltip_position_for_test(&self) -> Option<Point> {
        self.tooltip_pos.get()
    }

    // 测试目标保留组合图表图例布局观测入口，供布局测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn legend_layout_for_test(&self, frame: Rect) -> Option<(Rect, Rect)> {
        let mut plot = Rect::new(
            frame.x + self.padding,
            frame.y + self.padding,
            (frame.w - self.padding * 2.0).max(0.0),
            (frame.h - self.padding * 2.0).max(0.0),
        );
        if !self.title.is_empty() {
            plot.y += 20.0;
            plot.h = (plot.h - 20.0).max(0.0);
        }
        if !self.subtitle.is_empty() {
            plot.y += 16.0;
            plot.h = (plot.h - 16.0).max(0.0);
        }
        let (plot, legend) = self.legend_layout(plot);
        legend.map(|legend| (plot, legend))
    }

    // 测试目标保留组合图表动画进度观测入口，供动画测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn animation_progress_for_test(&self) -> Option<f32> {
        self.animation_player
            .as_ref()
            .map(|player| player.opacity_progress)
    }
}
