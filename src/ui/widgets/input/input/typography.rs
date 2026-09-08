//! Keep Input measurement, paint and editing geometry on the same typography.

use super::*;
use crate::core::EdgeInsets;
use crate::draw::resources::font::text_backend::normal_line_height;
use crate::ui::ThemeTokens;

impl Input {
    pub(super) fn resolved_typography(&self, tokens: &dyn ThemeTokens) -> InputTypographyVisual {
        let mut typography = self.visual.typography;
        typography.font_size = self.view_style.as_ref().map_or_else(
            || tokens.font_size(),
            |style| style.resolve_font_size(tokens),
        );
        typography.font_size =
            crate::draw::resources::font::text_backend::bounded_font_size(typography.font_size);
        typography.line_height = self
            .view_style
            .as_ref()
            .and_then(|style| style.resolve_line_height(typography.font_size))
            .unwrap_or_else(|| normal_line_height(typography.font_size));
        typography.addon_font_size = typography.font_size;
        typography.status_font_size = tokens.font_size_sm();
        typography
    }

    pub(super) fn content_padding(&self) -> EdgeInsets {
        self.view_style
            .as_ref()
            .map(|style| style.padding)
            .filter(|padding| *padding != EdgeInsets::default())
            .unwrap_or_else(|| {
                EdgeInsets::new(
                    self.visual.layout.horizontal_padding,
                    self.visual.layout.textarea_content_vertical_inset,
                    self.visual.layout.horizontal_padding,
                    self.visual.layout.textarea_content_vertical_inset,
                )
            })
    }

    pub(super) fn resolved_visual(&self, tokens: &dyn ThemeTokens) -> ResolvedInputVisual {
        let mut visual = self.visual.resolve(tokens);
        if let Some(style) = self.view_style.as_ref() {
            visual.text = style.color.resolve(tokens);
            if let Some(background) = style.background {
                visual.background = background.resolve(tokens);
                visual.background_elevated = visual.background;
            }
            if let Some(border) = style.border_color {
                visual.border = border.resolve(tokens);
            }
        }
        visual
    }

    pub(super) fn text_weight(&self) -> crate::ui::theme::style::FontWeight {
        self.view_style
            .as_ref()
            .map_or_else(Default::default, |style| style.effective_font_weight())
    }
}
