use crate::core::{Point, Rect};
use crate::ui::widget_runtime::paint_context::PaintContext;

use super::super::line_chart::LineData;
use super::{
    ChartKind, ChartPayload, ChartPlaceholder, ChartSeries, LegendPosition, MAX_HEATMAP_DIMENSION,
    TooltipDatum, calendar_day_count, finite_or_zero, january_first_weekday, normalized_grid_index,
    palette_color, scatter_tooltip_datum,
};

impl ChartPlaceholder {
    /// 判断图表是否携带可绘制的数据（按图表类型分别检查）。
    pub(crate) fn has_data(&self) -> bool {
        match self.kind {
            // 通用占位无数据。
            ChartKind::Generic => false,
            // 柱状：任意一个柱序列非空即有数据。
            ChartKind::Bar => match &self.payload {
                ChartPayload::Bars(data) => !data.is_empty(),
                ChartPayload::BarSeries(series) => {
                    series.iter().any(|series| !series.data.is_empty())
                }
                _ => self.bar_series.iter().any(|series| !series.data.is_empty()),
            },
            // 折线/面积：任意一个线序列非空即有数据。
            ChartKind::Line | ChartKind::Area => match &self.payload {
                ChartPayload::Lines(data) => !data.is_empty(),
                ChartPayload::LineSeries(series) => {
                    series.iter().any(|series| !series.data.is_empty())
                }
                _ => self
                    .line_series
                    .iter()
                    .any(|series| !series.data.is_empty()),
            },
            // 散点/气泡：任意序列非空即有数据。
            ChartKind::Scatter => match &self.payload {
                ChartPayload::Scatter(data) => !data.is_empty(),
                ChartPayload::Bubble(data) => !data.is_empty(),
                ChartPayload::ScatterSeries(series) => {
                    series.iter().any(|series| !series.data.is_empty())
                }
                _ => self
                    .scatter_series
                    .iter()
                    .any(|series| !series.data.is_empty()),
            },
            // 雷达：任意雷达序列非空即有数据。
            ChartKind::Radar => self
                .radar_series
                .iter()
                .any(|series| !series.data.is_empty()),
            // 热力图：存在有效单元格（日历模式按天数、普通模式按网格上限）。
            ChartKind::Heatmap => {
                matches!(&self.payload, ChartPayload::Heatmap(data) if data.iter().any(|cell| {
                    if self.calendar_mode {
                        cell.x < calendar_day_count(self.year)
                    } else {
                        cell.x < MAX_HEATMAP_DIMENSION && cell.y < MAX_HEATMAP_DIMENSION
                    }
                }))
            }
            // 漏斗/瀑布/矩形树：数据列表非空即有数据。
            ChartKind::Funnel => {
                matches!(&self.payload, ChartPayload::Funnel(data) if !data.is_empty())
            }
            ChartKind::Waterfall => {
                matches!(&self.payload, ChartPayload::Waterfall(data) if !data.is_empty())
            }
            // 组合：组合序列或柱/线序列任一非空即有数据。
            ChartKind::Combo => {
                !self.combo_series.is_empty()
                    || self.bar_series.iter().any(|series| !series.data.is_empty())
                    || self
                        .line_series
                        .iter()
                        .any(|series| !series.data.is_empty())
            }
            ChartKind::Treemap => {
                matches!(&self.payload, ChartPayload::Treemap(data) if !data.is_empty())
            }
            // 仪表盘恒视为有数据。
            ChartKind::Gauge => true,
        }
    }

    /// 按指针 x 坐标换算类别索引（等分映射，越界夹紧）。
    pub(crate) fn category_index(pos: Point, frame: Rect, count: usize) -> usize {
        // 无类别或空宽时返回 0。
        if count == 0 || frame.w <= 0.0 {
            return 0;
        }
        (((pos.x - frame.x) / frame.w * count as f32).floor() as usize).min(count - 1)
    }

    /// 指针处的数据标签文本（tooltip 数据的 label 部分）。
    pub(crate) fn data_label_at(&self, pos: Point, frame: Rect) -> String {
        self.tooltip_datum_at(pos, frame)
            .map(|datum| datum.label)
            .unwrap_or_default()
    }

    /// 构造指针处的 tooltip 数据：按载荷类型分别取对应索引项。
    pub(crate) fn tooltip_datum_at(&self, pos: Point, frame: Rect) -> Option<TooltipDatum> {
        let index = |count| Self::category_index(pos, frame, count);
        let payload_datum = match &self.payload {
            ChartPayload::Bars(data) => data.get(index(data.len())).map(|item| TooltipDatum {
                label: item.label.clone(),
                value: Some(finite_or_zero(item.value)),
                ..TooltipDatum::default()
            }),
            ChartPayload::BarSeries(series) => series.first().and_then(|series| {
                series
                    .data
                    .get(index(series.data.len()))
                    .map(|item| TooltipDatum {
                        label: item.label.clone(),
                        series: Some(series.name.clone()),
                        value: Some(finite_or_zero(item.value)),
                        ..TooltipDatum::default()
                    })
            }),
            ChartPayload::Lines(data) => data.get(index(data.len())).map(|item| TooltipDatum {
                label: item.label.clone(),
                value: Some(finite_or_zero(item.value)),
                ..TooltipDatum::default()
            }),
            ChartPayload::LineSeries(series) => series.first().and_then(|series| {
                series
                    .data
                    .get(index(series.data.len()))
                    .map(|item| TooltipDatum {
                        label: item.label.clone(),
                        series: Some(series.name.clone()),
                        value: Some(finite_or_zero(item.value)),
                        ..TooltipDatum::default()
                    })
            }),
            ChartPayload::Scatter(data) => data.get(index(data.len())).map(scatter_tooltip_datum),
            ChartPayload::ScatterSeries(series) => series.first().and_then(|series| {
                series.data.get(index(series.data.len())).map(|item| {
                    let mut datum = scatter_tooltip_datum(item);
                    datum.series = Some(series.name.clone());
                    datum
                })
            }),
            ChartPayload::Bubble(data) => data.get(index(data.len())).map(|item| TooltipDatum {
                label: item.label.clone(),
                x: Some(finite_or_zero(item.x)),
                y: Some(finite_or_zero(item.y)),
                ..TooltipDatum::default()
            }),
            // 热力图：按网格坐标反查单元格（日历模式按周/星期换算）。
            ChartPayload::Heatmap(cells) => {
                // 列数：日历模式按周数，普通模式按最大 x。
                let columns = if self.calendar_mode {
                    let offset = january_first_weekday(self.year);
                    (offset + calendar_day_count(self.year) - 1) / 7 + 1
                } else {
                    cells
                        .iter()
                        .map(|cell| cell.x)
                        .max()
                        .unwrap_or(0)
                        .saturating_add(1)
                        .min(MAX_HEATMAP_DIMENSION)
                };
                // 行数：日历固定 7 行（星期），普通模式按最大 y。
                let rows = if self.calendar_mode {
                    7
                } else {
                    cells
                        .iter()
                        .map(|cell| cell.y)
                        .max()
                        .unwrap_or(0)
                        .saturating_add(1)
                        .min(MAX_HEATMAP_DIMENSION)
                };
                let x = normalized_grid_index(pos.x, frame.x, frame.w, columns);
                let y = normalized_grid_index(pos.y, frame.y, frame.h, rows);
                // 命中与指针同格的数据项。
                cells
                    .iter()
                    .find(|cell| {
                        if self.calendar_mode {
                            let ordinal = january_first_weekday(self.year) + cell.x;
                            cell.x < calendar_day_count(self.year)
                                && ordinal / 7 == x
                                && ordinal % 7 == y
                        } else {
                            cell.x == x && cell.y == y
                        }
                    })
                    .map(|cell| {
                        // 标签优先用配置的坐标标签，缺省回退为数字。
                        let x_label = self
                            .x_labels
                            .get(cell.x)
                            .cloned()
                            .unwrap_or_else(|| cell.x.to_string());
                        let y_label = self
                            .y_labels
                            .get(cell.y)
                            .cloned()
                            .unwrap_or_else(|| cell.y.to_string());
                        TooltipDatum {
                            label: format!("{y_label} {x_label}"),
                            value: Some(finite_or_zero(cell.value)),
                            ..TooltipDatum::default()
                        }
                    })
            }
            ChartPayload::Funnel(data) => data.get(index(data.len())).map(|item| TooltipDatum {
                label: item.label.clone(),
                value: Some(finite_or_zero(item.value)),
                ..TooltipDatum::default()
            }),
            ChartPayload::Waterfall(data) => data.get(index(data.len())).map(|item| TooltipDatum {
                label: item.label.clone(),
                value: Some(finite_or_zero(item.value)),
                ..TooltipDatum::default()
            }),
            // 矩形树：附带占比信息。
            ChartPayload::Treemap(data) => {
                let total = data
                    .iter()
                    .map(|node| finite_or_zero(node.value).max(0.0))
                    .sum::<f32>();
                data.get(index(data.len())).map(|item| {
                    let value = finite_or_zero(item.value);
                    TooltipDatum {
                        label: item.label.clone(),
                        value: Some(value),
                        percentage: (total > 0.0).then_some(value.max(0.0) / total),
                        ..TooltipDatum::default()
                    }
                })
            }
            ChartPayload::Empty => None,
        };
        // 载荷无命中时，回退到非载荷型图表（雷达/组合/仪表盘）。
        if payload_datum.is_some() {
            return payload_datum;
        }
        match self.kind {
            // 雷达：按轴标签与数值构造。
            ChartKind::Radar => {
                let series = self.radar_series.first()?;
                let item_index = index(series.data.len());
                let value = series.data.get(item_index)?;
                Some(TooltipDatum {
                    label: self
                        .radar_axes
                        .get(item_index)
                        .map(|axis| axis.label.clone())
                        .unwrap_or_else(|| item_index.to_string()),
                    series: Some(series.name.clone()),
                    value: Some(finite_or_zero(value.value)),
                    ..TooltipDatum::default()
                })
            }
            // 组合：依次尝试组合序列、柱序列、线序列。
            ChartKind::Combo => {
                if let Some(series) = self.combo_series.first() {
                    let item = series.data.get(index(series.data.len()))?;
                    return Some(TooltipDatum {
                        label: item.label.clone(),
                        series: Some(series.name.clone()),
                        value: Some(finite_or_zero(item.value)),
                        ..TooltipDatum::default()
                    });
                }
                if let Some(series) = self.bar_series.first() {
                    let item = series.data.get(index(series.data.len()))?;
                    return Some(TooltipDatum {
                        label: item.label.clone(),
                        series: Some(series.name.clone()),
                        value: Some(finite_or_zero(item.value)),
                        ..TooltipDatum::default()
                    });
                }
                let series = self.line_series.first()?;
                let item = series.data.get(index(series.data.len()))?;
                Some(TooltipDatum {
                    label: item.label.clone(),
                    series: Some(series.name.clone()),
                    value: Some(finite_or_zero(item.value)),
                    ..TooltipDatum::default()
                })
            }
            // 仪表盘：显示标题与当前值。
            ChartKind::Gauge => Some(TooltipDatum {
                label: self.title.clone(),
                value: Some(finite_or_zero(self.gauge_value)),
                ..TooltipDatum::default()
            }),
            _ => None,
        }
    }

    /// 指针处的 tooltip 文本（配置了 tooltip 且命中数据时返回格式化文本）。
    pub(crate) fn tooltip_text_at(&self, pos: Point, frame: Rect) -> Option<String> {
        let config = self.tooltip_config.as_ref()?;
        let datum = self.tooltip_datum_at(pos, frame)?;
        Some(config.format(&datum))
    }

    /// 图例标签列表：按载荷/图表类型取系列名或条目标签。
    pub(crate) fn legend_labels(&self) -> Vec<String> {
        let series_names = |series: &[ChartSeries<Vec<LineData>>]| {
            series
                .iter()
                .map(|series| series.name.clone())
                .collect::<Vec<_>>()
        };
        match &self.payload {
            ChartPayload::BarSeries(series) => {
                series.iter().map(|series| series.name.clone()).collect()
            }
            ChartPayload::LineSeries(series) => series_names(series),
            ChartPayload::ScatterSeries(series) => {
                series.iter().map(|series| series.name.clone()).collect()
            }
            // 漏斗/瀑布/矩形树：取前 8 个条目标签。
            ChartPayload::Funnel(data) => {
                data.iter().take(8).map(|item| item.label.clone()).collect()
            }
            ChartPayload::Waterfall(data) => {
                data.iter().take(8).map(|item| item.label.clone()).collect()
            }
            ChartPayload::Treemap(nodes) => nodes
                .iter()
                .take(8)
                .map(|node| node.label.clone())
                .collect(),
            // 载荷为空时按后备系列取标签。
            ChartPayload::Empty if !self.combo_series.is_empty() => self
                .combo_series
                .iter()
                .map(|series| series.name.clone())
                .collect(),
            ChartPayload::Empty if !self.radar_series.is_empty() => self
                .radar_series
                .iter()
                .map(|series| series.name.clone())
                .collect(),
            ChartPayload::Empty if !self.bar_series.is_empty() || !self.line_series.is_empty() => {
                self.bar_series
                    .iter()
                    .map(|series| series.name.clone())
                    .chain(self.line_series.iter().map(|series| series.name.clone()))
                    .collect()
            }
            // 仪表盘：显示标题或默认「值」。
            ChartPayload::Empty if self.kind == ChartKind::Gauge => {
                vec![if self.title.is_empty() {
                    "值".to_owned()
                } else {
                    self.title.clone()
                }]
            }
            ChartPayload::Empty => Vec::new(),
            // 其余类型缺省显示「数据」。
            _ => vec!["数据".to_owned()],
        }
    }

    /// 图例布局：返回扣除图例后的绘图区与图例矩形（无图例时原样返回）。
    pub(crate) fn legend_layout(&self, plot: Rect) -> (Rect, Option<Rect>) {
        // 不显示图例或无标签时不做切分。
        if self.legend == LegendPosition::None || self.legend_labels().is_empty() {
            return (plot, None);
        }
        let row = self.visual.layout.legend_row_height;
        match self.legend {
            // 顶部：图例占最上方一行。
            LegendPosition::Top => (
                Rect::new(plot.x, plot.y + row, plot.w, (plot.h - row).max(0.0)),
                Some(Rect::new(plot.x, plot.y, plot.w, row)),
            ),
            // 底部：图例占最下方一行。
            LegendPosition::Bottom => (
                Rect::new(plot.x, plot.y, plot.w, (plot.h - row).max(0.0)),
                Some(Rect::new(
                    plot.x,
                    plot.y + (plot.h - row).max(0.0),
                    plot.w,
                    row.min(plot.h),
                )),
            ),
            // 左/右：图例占约 1/4 宽度（64~120px）。
            LegendPosition::Left | LegendPosition::Right => {
                let width = (plot.w * self.visual.layout.legend_side_ratio)
                    .clamp(
                        self.visual.layout.legend_side_min_width,
                        self.visual.layout.legend_side_max_width,
                    )
                    .min(plot.w);
                if self.legend == LegendPosition::Left {
                    (
                        Rect::new(plot.x + width, plot.y, (plot.w - width).max(0.0), plot.h),
                        Some(Rect::new(plot.x, plot.y, width, plot.h)),
                    )
                } else {
                    (
                        Rect::new(plot.x, plot.y, (plot.w - width).max(0.0), plot.h),
                        Some(Rect::new(
                            plot.x + (plot.w - width).max(0.0),
                            plot.y,
                            width,
                            plot.h,
                        )),
                    )
                }
            }
            LegendPosition::None => (plot, None),
        }
    }

    /// 绘制图例：顶部/底部为一行串联文本，左/右为色块 + 标签列表。
    pub(crate) fn paint_legend(
        &self,
        ctx: &mut PaintContext,
        rect: Rect,
        visual: &super::ResolvedAdvancedChartVisual,
    ) {
        let labels = self.legend_labels();
        // 无标签或空矩形时跳过。
        if labels.is_empty() || rect.w <= 0.0 || rect.h <= 0.0 {
            return;
        }
        let color = visual.text_secondary;
        // 顶部/底部：单行居中显示全部标签。
        if matches!(self.legend, LegendPosition::Top | LegendPosition::Bottom) {
            ctx.text_center(&labels.join("  "), rect, color, self.visual.typography.body);
            return;
        }
        // 左/右：逐行绘制色块与标签，越界截断。
        for (index, label) in labels.iter().enumerate() {
            let y = rect.y + index as f32 * self.visual.layout.legend_row_height;
            if y + self.visual.layout.legend_row_height > rect.y + rect.h {
                break;
            }
            ctx.fill_rect(
                Rect::new(
                    rect.x + self.visual.layout.legend_swatch_x,
                    y + self.visual.layout.legend_swatch_y,
                    self.visual.layout.legend_swatch_size,
                    self.visual.layout.legend_swatch_size,
                ),
                palette_color(index),
                None,
            );
            ctx.draw_text(
                label,
                Point::new(
                    rect.x + self.visual.layout.legend_text_x,
                    y + self.visual.layout.legend_text_y,
                ),
                color,
                self.visual.typography.body,
            );
        }
    }
}
