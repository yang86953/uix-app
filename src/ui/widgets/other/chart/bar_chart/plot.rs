//! 柱状图几何计算与绘制辅助。

use crate::core::{Point, Rect, Size};
use crate::ui::component::paint_context::PaintContext;

use super::super::advanced::{LegendPosition, TooltipDatum, normalized_ratio};
use super::{BarChart, BarData};

pub(super) fn map_x(value: f32, x: f32, width: f32, min: f32, max: f32) -> f32 {
    x + normalized_ratio(value, min, max) * width
}

#[derive(Debug)]
pub(crate) struct BarPlot {
    pub(crate) min: f32,
    pub(crate) max: f32,
    pub(crate) chart_x: f32,
    pub(crate) chart_y: f32,
    pub(crate) chart_w: f32,
    pub(crate) chart_h: f32,
    pub(crate) y_label_w: f32,
    pub(crate) baseline: f32,
    // 测试目标保留柱形几何列表，供图表布局观测入口按需读取。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) bars: Vec<Rect>,
    /// `(series_index, item_index, rect)` for grouped/stacked rendering.
    pub(crate) items: Vec<(usize, usize, Rect)>,
}

impl BarChart {
    pub(super) fn intrinsic_size(&self) -> Size {
        let width = if self.fixed_width > 0.0 {
            self.fixed_width
        } else {
            Self::DEFAULT_WIDTH
        };
        let height = if self.fixed_height > 0.0 {
            self.fixed_height
        } else {
            Self::DEFAULT_HEIGHT
        };
        Size::new(width, height)
    }

    pub(super) fn optional_dimension(value: f32) -> f32 {
        if value.is_finite() && value > 0.0 {
            value
        } else {
            0.0
        }
    }

    pub(super) fn finite_value(value: f32) -> f32 {
        if value.is_finite() { value } else { 0.0 }
    }

    pub(super) fn reserve_legend(&self, content: &mut Rect) -> Option<Rect> {
        if self.legend == LegendPosition::None {
            return None;
        }
        const ROW: f32 = 18.0;
        match self.legend {
            LegendPosition::Top => {
                let rect = Rect::new(content.x, content.y, content.w, ROW.min(content.h));
                content.y += ROW.min(content.h);
                content.h = (content.h - ROW).max(0.0);
                Some(rect)
            }
            LegendPosition::Bottom => {
                let height = ROW.min(content.h);
                let rect = Rect::new(content.x, content.y + content.h - height, content.w, height);
                content.h = (content.h - ROW).max(0.0);
                Some(rect)
            }
            LegendPosition::Left | LegendPosition::Right => {
                let width = (content.w * 0.24).clamp(64.0, 120.0).min(content.w);
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

    pub(super) fn data_index(&self, pos: Point, frame: Rect) -> Option<usize> {
        let count = if self.series.is_empty() {
            self.data.len()
        } else {
            self.series.first().map_or(0, |series| series.data.len())
        };
        let (offset, extent) = if self.horizontal {
            (pos.y - frame.y, frame.h)
        } else {
            (pos.x - frame.x, frame.w)
        };
        (count > 0 && extent > 0.0)
            .then(|| (offset / extent * count as f32).floor() as usize)
            .map(|index| index.min(count - 1))
    }

    pub(super) fn data_label(&self, index: usize) -> &str {
        if self.series.is_empty() {
            self.data.get(index).map_or("", |item| item.label.as_str())
        } else {
            self.series
                .first()
                .and_then(|series| series.data.get(index))
                .map_or("", |item| item.label.as_str())
        }
    }

    pub(super) fn format_value(value: f32) -> String {
        if value == value.trunc() {
            format!("{value:.0}")
        } else {
            format!("{value:.1}")
        }
    }

    pub(super) fn paint_tooltip(&self, ctx: &mut PaintContext, frame: Rect, pos: Point) {
        let Some(config) = &self.tooltip_config else {
            return;
        };
        let Some(datum) = self.tooltip_datum_at(pos, frame) else {
            return;
        };
        let text = config.format(&datum);
        let size = ctx.measure_text(&text, 10.0);
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

    pub(super) fn tooltip_datum_at(&self, pos: Point, frame: Rect) -> Option<TooltipDatum> {
        let index = self.data_index(pos, frame)?;
        let (series, item) = if self.series.is_empty() {
            (None, self.data.get(index)?)
        } else {
            let series = self.series.first()?;
            (Some(series.name.clone()), series.data.get(index)?)
        };
        Some(TooltipDatum {
            label: item.label.clone(),
            series,
            value: Some(Self::finite_value(item.value)),
            ..TooltipDatum::default()
        })
    }

    fn value_range(&self) -> Option<(f32, f32)> {
        let series = self.series_data();
        if series.is_empty() || series.iter().all(|items| items.is_empty()) {
            return None;
        }
        let mut min = 0.0_f32;
        let mut observed_max = 0.0_f32;
        if self.stacked && series.len() > 1 {
            let category_count = series.iter().map(|items| items.len()).max().unwrap_or(0);
            for index in 0..category_count {
                let (positive, negative) = series.iter().fold((0.0, 0.0), |(pos, neg), items| {
                    let value = items
                        .get(index)
                        .map_or(0.0, |item| Self::finite_value(item.value));
                    if value >= 0.0 {
                        (pos + value, neg)
                    } else {
                        (pos, neg + value)
                    }
                });
                min = min.min(negative);
                observed_max = observed_max.max(positive);
            }
        } else {
            for items in &series {
                for item in items.iter() {
                    let value = Self::finite_value(item.value);
                    min = min.min(value);
                    observed_max = observed_max.max(value);
                }
            }
        }
        let max = if self.max_value > 0.0 {
            self.max_value
        } else {
            observed_max
        };
        (max > min).then_some((min, max))
    }

    fn series_data(&self) -> Vec<&[BarData]> {
        if self.series.is_empty() {
            vec![self.data.as_slice()]
        } else {
            self.series
                .iter()
                .map(|series| series.data.as_slice())
                .collect()
        }
    }

    pub(super) fn plot_geometry(&self, frame: Rect) -> Option<BarPlot> {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return None;
        }
        let (min, max) = self.value_range()?;
        let y_label_w = 36.0_f32.min(frame.w * 0.35);
        let chart_x = frame.x + y_label_w;
        let chart_w = frame.w - y_label_w;
        let label_h = 14.0;
        let value_h = if self.show_value { 14.0 } else { 0.0 };
        let chart_h = frame.h - label_h - value_h - 4.0;
        if chart_w <= 0.0 || chart_h <= 0.0 {
            return None;
        }
        let map_y = |value: f32| frame.y + chart_h - normalized_ratio(value, min, max) * chart_h;
        let baseline = map_y(0.0);
        let series = self.series_data();
        let category_count = series.iter().map(|items| items.len()).max().unwrap_or(0);
        if category_count == 0 {
            return None;
        }
        let category_extent = if self.horizontal { chart_h } else { chart_w };
        let category_slot = category_extent / category_count as f32;
        let series_count = series.len().max(1);
        let category_gap = category_slot * self.category_gap;
        let available = (category_slot - category_gap).max(0.0);
        #[cfg(test)]
        let mut bars = Vec::new();
        let mut items = Vec::new();
        for category in 0..category_count {
            if self.stacked && series_count > 1 {
                let mut positive = 0.0;
                let mut negative = 0.0;
                for (series_index, data) in series.iter().enumerate() {
                    let Some(bar) = data.get(category) else {
                        continue;
                    };
                    let value = Self::finite_value(bar.value);
                    let (start, end) = if value >= 0.0 {
                        let start = positive;
                        positive += value;
                        (start, positive)
                    } else {
                        let start = negative;
                        negative += value;
                        (start, negative)
                    };
                    let rect = if self.horizontal {
                        let x1 = map_x(start, chart_x, chart_w, min, max);
                        let x2 = map_x(end, chart_x, chart_w, min, max);
                        Rect::new(
                            x1.min(x2),
                            frame.y + category as f32 * category_slot + category_gap * 0.5,
                            (x2 - x1).abs(),
                            available,
                        )
                    } else {
                        let y1 = map_y(start);
                        let y2 = map_y(end);
                        Rect::new(
                            chart_x + category as f32 * category_slot + category_gap * 0.5,
                            y1.min(y2),
                            available,
                            (y2 - y1).abs(),
                        )
                    };
                    items.push((series_index, category, rect));
                    #[cfg(test)]
                    if series_index == 0 {
                        bars.push(rect);
                    }
                }
            } else {
                let visible_series = if self.grouped { series_count } else { 1 };
                let series_slot = available / visible_series as f32;
                let bar_extent = if visible_series > 1 {
                    series_slot * (1.0 - self.bar_gap)
                } else {
                    series_slot
                };
                let bar_inset = (series_slot - bar_extent) * 0.5;
                for series_index in 0..visible_series {
                    let Some(data) = series.get(series_index) else {
                        continue;
                    };
                    let Some(bar) = data.get(category) else {
                        continue;
                    };
                    let value = Self::finite_value(bar.value);
                    let rect = if self.horizontal {
                        let x1 = map_x(0.0, chart_x, chart_w, min, max);
                        let x2 = map_x(value, chart_x, chart_w, min, max);
                        Rect::new(
                            x1.min(x2),
                            frame.y
                                + category as f32 * category_slot
                                + category_gap * 0.5
                                + series_index as f32 * series_slot
                                + bar_inset,
                            (x2 - x1).abs(),
                            bar_extent,
                        )
                    } else {
                        let value_y = map_y(value);
                        let x = chart_x
                            + category as f32 * category_slot
                            + category_gap * 0.5
                            + series_index as f32 * series_slot
                            + bar_inset;
                        Rect::new(
                            x,
                            value_y.min(baseline),
                            bar_extent,
                            (value_y - baseline).abs(),
                        )
                    };
                    items.push((series_index, category, rect));
                    #[cfg(test)]
                    if series_index == 0 {
                        bars.push(rect);
                    }
                    if !self.grouped {
                        break;
                    }
                }
            }
        }
        Some(BarPlot {
            min,
            max,
            chart_x,
            chart_y: frame.y,
            chart_w,
            chart_h,
            y_label_w,
            baseline,
            #[cfg(test)]
            bars,
            items,
        })
    }
}
