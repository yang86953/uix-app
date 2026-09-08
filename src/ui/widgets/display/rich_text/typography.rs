//! Resolve declarative typography once for measurement, rendering and hit testing.

use super::*;
use crate::core::EdgeInsets;
use crate::draw::resources::font::text_backend::normal_line_height;
use crate::ui::ThemeTokens;
use crate::ui::theme::style::{Style, TypographyToken};

impl RichText {
    pub(crate) fn apply_view_layout_style(
        &mut self,
        style: &Style,
        grow: Option<f32>,
        shrink: Option<f32>,
    ) {
        self.view_style = Some(style.clone());
        if let Some(grow) = grow.filter(|value| value.is_finite()) {
            self.view_flex_grow = grow.max(0.0);
        }
        if let Some(shrink) = shrink.filter(|value| value.is_finite()) {
            self.view_flex_shrink = shrink.max(0.0);
        }
        self.layout_dirty.set(true);
    }

    // Layout-only View decoration must not erase an authored builder color.
    pub(super) fn view_text_color(&self) -> Option<crate::ui::theme::style::ColorValue> {
        use crate::ui::theme::NeutralRole;
        use crate::ui::theme::style::ColorValue;
        self.view_style
            .as_ref()
            .filter(|style| {
                self.use_theme_color || style.color != ColorValue::Neutral(NeutralRole::Text)
            })
            .map(|style| style.color)
    }

    pub(crate) fn view_layout_changed(&self, next: &Self) -> bool {
        self.view_style != next.view_style
            || self.view_flex_grow != next.view_flex_grow
            || self.view_flex_shrink != next.view_flex_shrink
    }

    pub(super) fn font_size_with_tokens(&self, dpi: f32, tokens: &dyn ThemeTokens) -> f32 {
        let size = if let Some(unit) = self.default_font_size_unit {
            unit.to_dip(dpi)
        } else if let Some(style) = self.view_style.as_ref().filter(|style| {
            (!self.font_size_authored && self.default_font_size == self.visual.defaults.font_size)
                || style.font_size != TypographyToken::Body
        }) {
            style.resolve_font_size(tokens)
        } else if self.font_size_authored
            || self.default_font_size != self.visual.defaults.font_size
        {
            self.default_font_size
        } else {
            tokens.font_size()
        };
        if size.is_finite() && size > 0.0 {
            size.clamp(1.0, 512.0)
        } else {
            14.0
        }
    }

    pub(super) fn resolved_metrics(&self, font_size: f32) -> presentation::RichTextMetricsVisual {
        let mut metrics = self.visual.metrics;
        if let Some(height) = self
            .view_style
            .as_ref()
            .and_then(|style| style.resolve_line_height(font_size))
        {
            metrics.line_height_factor = height / font_size;
        }
        metrics
    }

    pub(super) fn content_padding(&self) -> EdgeInsets {
        self.view_style
            .as_ref()
            .map_or_else(EdgeInsets::zero, |style| style.padding)
    }

    pub(super) fn content_point(&self, pos: Point) -> Point {
        let padding = self.content_padding();
        Point::new(pos.x - padding.left, pos.y - padding.top)
    }

    pub(super) fn measure_content(&self, constraints: Constraints) -> Size {
        let style = self.view_style.as_ref();
        let width = style
            .and_then(|style| style.width)
            .filter(|width| width.is_finite());
        let height = style
            .and_then(|style| style.height)
            .filter(|height| height.is_finite());
        let padding = self.content_padding();
        let available = width.unwrap_or(constraints.max.w).min(constraints.max.w);
        let est_width = if available.is_finite() && available > 0.0 {
            (available - padding.horizontal()).max(1.0)
        } else if self.last_layout_width.get() > 0.0 {
            self.last_layout_width.get()
        } else {
            self.visual.defaults.unconstrained_width
        };
        let fs = self.resolved_font_size_px(self.visual.defaults.measurement_dpi);
        let metrics = self.resolved_metrics(fs);
        let changed = (self.last_layout_width.get() - est_width).abs()
            > self.visual.defaults.relayout_epsilon
            || self.last_font_size.get() != fs
            || self.last_line_height.get() != metrics.line_height_factor;
        if self.layout_dirty.get() || self.layout_height.get() <= 0.0 || changed {
            let (_, total_h, max_w) = layout_rich_text_with_images_visual(
                &self.segments,
                est_width,
                fs,
                self.visual.estimated_palette(self.default_color),
                &self.image_states.borrow(),
                metrics,
            );
            self.layout_height.set(total_h);
            self.content_width.set(max_w);
            self.layout_lines.borrow_mut().clear();
            self.code_regions.borrow_mut().clear();
            self.last_layout_width.set(est_width);
            self.last_layout_palette.set(None);
            self.last_font_size.set(fs);
            self.last_line_height.set(metrics.line_height_factor);
            self.layout_dirty.set(false);
        }
        constraints.clamp(Size::new(
            width
                .unwrap_or(self.content_width.get() + padding.horizontal())
                .max(0.0),
            height
                .unwrap_or(self.layout_height.get() + padding.vertical())
                .max(0.0),
        ))
    }

    pub(super) fn run_draw_y(
        &self,
        ctx: &mut PaintContext,
        line: &LayoutLine,
        y: f32,
        fs: f32,
    ) -> f32 {
        let largest = line
            .glyphs
            .iter()
            .filter(|glyph| glyph.kind != LayoutGlyphKind::InlineImage)
            .map(|glyph| glyph.font_size)
            .fold(fs, f32::max);
        let font = *ctx.font();
        let baseline = ctx
            .font_service()
            .horizontal_line_metrics(&font, largest)
            .map_or(line.height * 0.8, |metrics| {
                metrics.baseline_in_line_box(line.height)
            });
        let run_baseline = ctx
            .font_service()
            .horizontal_line_metrics(&font, fs)
            .map_or(normal_line_height(fs) * 0.8, |metrics| {
                metrics.baseline_in_line_box(normal_line_height(fs))
            });
        y + baseline - run_baseline
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::style::ColorValue;

    #[test]
    fn layout_only_style_preserves_authored_rich_text_color() {
        let mut rich = RichText::new().color(Color::from_rgb(255, 0, 0));
        rich.apply_view_layout_style(&Style::default(), None, None);
        assert_eq!(rich.view_text_color(), None);
        let mut explicit = Style::default();
        explicit.color = ColorValue::Custom(Color::from_rgb(0, 0, 255));
        rich.apply_view_layout_style(&explicit, None, None);
        assert_eq!(rich.view_text_color(), Some(explicit.color));
        let mut themed = RichText::new();
        themed.apply_view_layout_style(&Style::default(), None, None);
        assert_eq!(themed.view_text_color(), Some(Style::default().color));
    }
}
