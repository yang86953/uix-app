//! NavigationShell 的 UIX 静态视觉契约与主题解析。

use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct NavigationShellLayoutVisual {
    pub(super) expanded_width: f32,
    pub(super) collapsed_width: f32,
    pub(super) height: f32,
    pub(super) expanded_header_height: f32,
    pub(super) collapsed_header_height: f32,
    pub(super) version_height: f32,
    pub(super) toggle_size: f32,
    pub(super) title_start: f32,
    pub(super) title_end_reserve: f32,
    pub(super) version_start: f32,
    pub(super) version_end_reserve: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct NavigationShellTypographyVisual {
    pub(super) title: f32,
    pub(super) version: f32,
    pub(super) toggle_icon: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct NavigationShellChromeVisual {
    pub(super) focus_width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct NavigationShellIconsVisual {
    pub(super) expand: &'static str,
    pub(super) collapse: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct NavigationShellPaletteVisual {
    container_background: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    text_quaternary: ColorValue,
    primary: ColorValue,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct NavigationShellVisual {
    pub(super) layout: NavigationShellLayoutVisual,
    pub(super) typography: NavigationShellTypographyVisual,
    pub(super) chrome: NavigationShellChromeVisual,
    pub(super) icons: NavigationShellIconsVisual,
    palette: NavigationShellPaletteVisual,
}

crate::uix_items!("src/ui/widgets/navigation/navigation_shell/navigation_shell.uix");

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ResolvedNavigationShellVisual {
    pub(super) container_background: Color,
    pub(super) text: Color,
    pub(super) text_secondary: Color,
    pub(super) text_quaternary: Color,
    pub(super) primary: Color,
}

impl NavigationShellVisual {
    pub(super) fn resolve(&self, tokens: &dyn ThemeTokens) -> ResolvedNavigationShellVisual {
        ResolvedNavigationShellVisual {
            container_background: self.palette.container_background.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            text_quaternary: self.palette.text_quaternary.resolve(tokens),
            primary: self.palette.primary.resolve(tokens),
        }
    }
}

pub(super) const fn navigation_shell_container_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
pub(super) const fn navigation_shell_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(super) const fn navigation_shell_text_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
pub(super) const fn navigation_shell_text_quaternary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextQuaternary)
}
pub(super) const fn navigation_shell_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
