use crate::draw::Color;
use crate::ui::{IColorTokens, ShadowToken, IBoxShadowTokens, ISpacingTokens, ITypographyTokens, ThemeTokens, TokenProvider};
pub(super) struct NeutralTokens(pub bool);
impl NeutralTokens {
    fn foreground(&self) -> Color { if self.0 { Color::white() } else { Color::black() } }
    fn background(&self) -> Color { if self.0 { Color::black() } else { Color::white() } }
}
impl IColorTokens for NeutralTokens {
    fn color_primary(&self) -> Color { self.foreground() }
    fn color_primary_hover(&self) -> Color { self.foreground() }
    fn color_primary_active(&self) -> Color { self.foreground() }
    fn color_primary_bg(&self) -> Color { self.background() }
    fn color_primary_border(&self) -> Color { self.foreground() }
    fn color_bg_container(&self) -> Color { self.background() }
    fn color_bg_elevated(&self) -> Color { self.background() }
    fn color_bg_raised(&self) -> Color { self.background() }
    fn color_bg_overlay(&self) -> Color { self.background() }
    fn color_bg_layout(&self) -> Color { self.background() }
    fn color_bg_spotlight(&self) -> Color { self.background() }
    fn color_bg_mask(&self) -> Color { self.background() }
    fn color_border(&self) -> Color { self.foreground() }
    fn color_border_secondary(&self) -> Color { self.foreground() }
    fn color_fill(&self) -> Color { self.foreground() }
    fn color_fill_secondary(&self) -> Color { self.foreground() }
    fn color_fill_tertiary(&self) -> Color { self.foreground() }
    fn color_fill_quaternary(&self) -> Color { self.foreground() }
    fn color_text(&self) -> Color { self.foreground() }
    fn color_text_secondary(&self) -> Color { self.foreground() }
    fn color_text_tertiary(&self) -> Color { self.foreground() }
    fn color_text_quaternary(&self) -> Color { self.foreground() }
    fn color_white(&self) -> Color { Color::white() }
    fn color_black(&self) -> Color { Color::black() }
    fn color_shadow(&self) -> Color { Color::transparent() }
    fn color_shadow_secondary(&self) -> Color { Color::transparent() }
    fn color_success(&self) -> Color { self.foreground() }
    fn color_success_bg(&self) -> Color { self.background() }
    fn color_success_border(&self) -> Color { self.foreground() }
    fn color_warning(&self) -> Color { self.foreground() }
    fn color_warning_bg(&self) -> Color { self.background() }
    fn color_warning_border(&self) -> Color { self.foreground() }
    fn color_error(&self) -> Color { self.foreground() }
    fn color_error_bg(&self) -> Color { self.background() }
    fn color_error_border(&self) -> Color { self.foreground() }
    fn color_info(&self) -> Color { self.foreground() }
    fn color_info_bg(&self) -> Color { self.background() }
    fn color_info_border(&self) -> Color { self.foreground() }
    fn color_link(&self) -> Color { self.foreground() }
    fn color_link_hover(&self) -> Color { self.foreground() }
    fn color_link_active(&self) -> Color { self.foreground() }
}
impl ITypographyTokens for NeutralTokens { fn font_family(&self) -> &str { "sans-serif" } }
impl ISpacingTokens for NeutralTokens {}
impl IBoxShadowTokens for NeutralTokens {
    fn box_shadow(&self) -> ShadowToken { ShadowToken::none() }
    fn box_shadow_secondary(&self) -> ShadowToken { ShadowToken::none() }
}
impl ThemeTokens for NeutralTokens { fn is_dark(&self) -> bool { self.0 } }
impl TokenProvider for NeutralTokens {}
