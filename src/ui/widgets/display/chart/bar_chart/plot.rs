//! 柱状图几何计算与绘制辅助。

use std::fmt::{self, Write as _};

use crate::core::{Point, Rect, Size};
use crate::ui::widget_runtime::paint_context::PaintContext;

use super::super::advanced::{LegendPosition, TooltipDatum, normalized_ratio};
use super::presentation::ResolvedBarChartVisual;
use super::{BarChart, BarData};

// 保存无需堆分配的有限 f32 标签；最大 f32 的定点一位文本小于此容量。
pub(super) struct BarValueLabel {
    bytes: [u8; 48],
    len: u8,
}

impl BarValueLabel {
    fn new(value: f32) -> Self {
        let mut label = Self {
            bytes: [0; 48],
            len: 0,
        };
        let result = if value == value.trunc() {
            write!(&mut label, "{value:.0}")
        } else {
            write!(&mut label, "{value:.1}")
        };
        debug_assert!(result.is_ok(), "f32 标签必须适配固定栈缓冲");
        label
    }

    pub(super) fn as_str(&self) -> &str {
        // 写入来源是 Rust 格式化器，始终产生合法 UTF-8。
        std::str::from_utf8(&self.bytes[..usize::from(self.len)]).unwrap_or("")
    }
}

impl fmt::Write for BarValueLabel {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let start = usize::from(self.len);
        let end = start.checked_add(value.len()).ok_or(fmt::Error)?;
        let target = self.bytes.get_mut(start..end).ok_or(fmt::Error)?;
        target.copy_from_slice(value.as_bytes());
        self.len = end as u8;
        Ok(())
    }
}

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
            self.visual.defaults.width
        };
        let height = if self.fixed_height > 0.0 {
            self.fixed_height
        } else {
            self.visual.defaults.height
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
        let layout = self.visual.layout;
        match self.legend {
            LegendPosition::Top => {
                let height = layout.legend_row_height.min(content.h);
                let rect = Rect::new(content.x, content.y, content.w, height);
                content.y += height;
                content.h = (content.h - layout.legend_row_height).max(0.0);
                Some(rect)
            }
            LegendPosition::Bottom => {
                let height = layout.legend_row_height.min(content.h);
                let rect = Rect::new(content.x, content.y + content.h - height, content.w, height);
                content.h = (content.h - layout.legend_row_height).max(0.0);
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

    pub(super) fn format_value(value: f32) -> BarValueLabel {
        BarValueLabel::new(value)
    }

    pub(super) fn paint_tooltip(
        &self,
        ctx: &mut PaintContext,
        frame: Rect,
        pos: Point,
        resolved: ResolvedBarChartVisual,
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

    // 不拼接临时字符串，直接按测量结果居中绘制每个系列名称。
    pub(super) fn paint_legend(
        &self,
        ctx: &mut PaintContext,
        frame: Rect,
        resolved: ResolvedBarChartVisual,
    ) {
        let typography = self.visual.typography;
        if self.series.is_empty() {
            ctx.text_center(
                typography.single_series_legend,
                frame,
                resolved.text_secondary,
                typography.legend,
            );
            return;
        }
        let gap = ctx
            .measure_text(typography.series_separator, typography.legend)
            .w;
        let text_width = self
            .series
            .iter()
            .map(|series| ctx.measure_text(&series.name, typography.legend).w)
            .sum::<f32>();
        let gaps = gap * self.series.len().saturating_sub(1) as f32;
        let mut x = frame.x + (frame.w - text_width - gaps).max(0.0) * 0.5;
        let y = ctx.visual_center_y(frame, typography.legend);
        for series in &self.series {
            let width = ctx.measure_text(&series.name, typography.legend).w;
            ctx.draw_text(
                &series.name,
                Point::new(x, y),
                resolved.text_secondary,
                typography.legend,
            );
            x += width + gap;
        }
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
        let series_count = self.series_count();
        if (0..series_count)
            .all(|index| self.series_at(index).map_or(true, |items| items.is_empty()))
        {
            return None;
        }
        let mut min = 0.0_f32;
        let mut observed_max = 0.0_f32;
        if self.stacked && series_count > 1 {
            let category_count = (0..series_count)
                .filter_map(|index| self.series_at(index))
                .map(<[BarData]>::len)
                .max()
                .unwrap_or(0);
            for index in 0..category_count {
                let mut positive = 0.0;
                let mut negative = 0.0;
                for series_index in 0..series_count {
                    let value = self
                        .series_at(series_index)
                        .and_then(|items| items.get(index))
                        .map_or(0.0, |item| Self::finite_value(item.value));
                    if value >= 0.0 {
                        positive += value;
                    } else {
                        negative += value;
                    }
                }
                min = min.min(negative);
                observed_max = observed_max.max(positive);
            }
        } else {
            for series_index in 0..series_count {
                for item in self.series_at(series_index).unwrap_or_default() {
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

    fn series_count(&self) -> usize {
        if self.series.is_empty() {
            1
        } else {
            self.series.len()
        }
    }

    fn series_at(&self, index: usize) -> Option<&[BarData]> {
        if self.series.is_empty() {
            (index == 0).then_some(self.data.as_slice())
        } else {
            self.series.get(index).map(|series| series.data.as_slice())
        }
    }

    pub(super) fn plot_geometry(&self, frame: Rect) -> Option<BarPlot> {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return None;
        }
        let (min, max) = self.value_range()?;
        let layout = self.visual.layout;
        let y_label_w = layout
            .y_label_width
            .min(frame.w * layout.y_label_width_ratio);
        let chart_x = frame.x + y_label_w;
        let chart_w = frame.w - y_label_w;
        let label_h = layout.category_label_height;
        let value_h = if self.show_value {
            layout.value_label_height
        } else {
            0.0
        };
        let chart_h = frame.h - label_h - value_h - layout.plot_bottom_gap;
        if chart_w <= 0.0 || chart_h <= 0.0 {
            return None;
        }
        let map_y = |value: f32| frame.y + chart_h - normalized_ratio(value, min, max) * chart_h;
        let baseline = map_y(0.0);
        let series_count = self.series_count();
        let category_count = (0..series_count)
            .filter_map(|index| self.series_at(index))
            .map(<[BarData]>::len)
            .max()
            .unwrap_or(0);
        if category_count == 0 {
            return None;
        }
        let category_extent = if self.horizontal { chart_h } else { chart_w };
        let category_slot = category_extent / category_count as f32;
        let category_gap = category_slot * self.category_gap;
        let available = (category_slot - category_gap).max(0.0);
        #[cfg(test)]
        let mut bars = Vec::with_capacity(category_count);
        let visible_series_count = if self.stacked || self.grouped {
            series_count
        } else {
            1
        };
        let mut items = Vec::with_capacity(category_count.saturating_mul(visible_series_count));
        for category in 0..category_count {
            if self.stacked && series_count > 1 {
                let mut positive = 0.0;
                let mut negative = 0.0;
                for series_index in 0..series_count {
                    let Some(data) = self.series_at(series_index) else {
                        continue;
                    };
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
                    let Some(data) = self.series_at(series_index) else {
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
