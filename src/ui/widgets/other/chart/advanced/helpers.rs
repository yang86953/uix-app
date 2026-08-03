use crate::core::{Point, Rect};
use crate::ui::component::paint_context::PaintContext;

use super::super::line_chart::LineData;
use super::{
    ChartKind, ChartPayload, ChartPlaceholder, ChartSeries, LegendPosition,
    MAX_HEATMAP_DIMENSION, TooltipDatum, calendar_day_count, finite_or_zero,
    january_first_weekday, normalized_grid_index, palette_color, scatter_tooltip_datum,
};

impl ChartPlaceholder {

    pub(crate) fn has_data(&self) -> bool {
        match self.kind {
            ChartKind::Generic => false,
            ChartKind::Bar => match &self.payload {
                ChartPayload::Bars(data) => !data.is_empty(),
                ChartPayload::BarSeries(series) => {
                    series.iter().any(|series| !series.data.is_empty())
                }
                _ => self.bar_series.iter().any(|series| !series.data.is_empty()),
            },
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
            ChartKind::Radar => self
                .radar_series
                .iter()
                .any(|series| !series.data.is_empty()),
            ChartKind::Heatmap => {
                matches!(&self.payload, ChartPayload::Heatmap(data) if data.iter().any(|cell| {
                    if self.calendar_mode {
                        cell.x < calendar_day_count(self.year)
                    } else {
                        cell.x < MAX_HEATMAP_DIMENSION && cell.y < MAX_HEATMAP_DIMENSION
                    }
                }))
            }
            ChartKind::Funnel => {
                matches!(&self.payload, ChartPayload::Funnel(data) if !data.is_empty())
            }
            ChartKind::Waterfall => {
                matches!(&self.payload, ChartPayload::Waterfall(data) if !data.is_empty())
            }
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
            ChartKind::Gauge => true,
        }
    }

    pub(crate) fn category_index(pos: Point, frame: Rect, count: usize) -> usize {
        if count == 0 || frame.w <= 0.0 {
            return 0;
        }
        (((pos.x - frame.x) / frame.w * count as f32).floor() as usize).min(count - 1)
    }

    pub(crate) fn data_label_at(&self, pos: Point, frame: Rect) -> String {
        self.tooltip_datum_at(pos, frame)
            .map(|datum| datum.label)
            .unwrap_or_default()
    }

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
            ChartPayload::Heatmap(cells) => {
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
        if payload_datum.is_some() {
            return payload_datum;
        }
        match self.kind {
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
            ChartKind::Gauge => Some(TooltipDatum {
                label: self.title.clone(),
                value: Some(finite_or_zero(self.gauge_value)),
                ..TooltipDatum::default()
            }),
            _ => None,
        }
    }

    pub(crate) fn tooltip_text_at(&self, pos: Point, frame: Rect) -> Option<String> {
        let config = self.tooltip_config.as_ref()?;
        let datum = self.tooltip_datum_at(pos, frame)?;
        Some(config.format(&datum))
    }

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
            ChartPayload::Empty if self.kind == ChartKind::Gauge => {
                vec![if self.title.is_empty() {
                    "值".to_owned()
                } else {
                    self.title.clone()
                }]
            }
            ChartPayload::Empty => Vec::new(),
            _ => vec!["数据".to_owned()],
        }
    }

    pub(crate) fn legend_layout(&self, plot: Rect) -> (Rect, Option<Rect>) {
        if self.legend == LegendPosition::None || self.legend_labels().is_empty() {
            return (plot, None);
        }
        const ROW: f32 = 18.0;
        match self.legend {
            LegendPosition::Top => (
                Rect::new(plot.x, plot.y + ROW, plot.w, (plot.h - ROW).max(0.0)),
                Some(Rect::new(plot.x, plot.y, plot.w, ROW)),
            ),
            LegendPosition::Bottom => (
                Rect::new(plot.x, plot.y, plot.w, (plot.h - ROW).max(0.0)),
                Some(Rect::new(
                    plot.x,
                    plot.y + (plot.h - ROW).max(0.0),
                    plot.w,
                    ROW.min(plot.h),
                )),
            ),
            LegendPosition::Left | LegendPosition::Right => {
                let width = (plot.w * 0.24).clamp(64.0, 120.0).min(plot.w);
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

    pub(crate) fn paint_legend(&self, ctx: &mut PaintContext, rect: Rect) {
        let labels = self.legend_labels();
        if labels.is_empty() || rect.w <= 0.0 || rect.h <= 0.0 {
            return;
        }
        let color = ctx.tokens().color_text_secondary();
        if matches!(self.legend, LegendPosition::Top | LegendPosition::Bottom) {
            ctx.text_center(&labels.join("  "), rect, color, 10.0);
            return;
        }
        for (index, label) in labels.iter().enumerate() {
            let y = rect.y + index as f32 * 18.0;
            if y + 18.0 > rect.y + rect.h {
                break;
            }
            ctx.fill_rect(
                Rect::new(rect.x + 4.0, y + 5.0, 8.0, 8.0),
                palette_color(index),
                None,
            );
            ctx.draw_text(label, Point::new(rect.x + 16.0, y + 3.0), color, 10.0);
        }
    }


}

