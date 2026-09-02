//! MenuBar 的 UIX 静态视觉契约与主题解析。

use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::style::ColorValue;
use crate::ui::theme::{NeutralRole, ShadowToken};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct MenuBarLayoutVisual {
    /// 顶级入口的可点击行高。
    pub(super) entry_height: f32,
    /// 入口文字左右内边距。
    pub(super) entry_padding_x: f32,
    /// 相邻入口之间的水平间距。
    pub(super) entry_gap: f32,
    /// 入口最小宽度，短标签也保持可点击面积。
    pub(super) entry_min_width: f32,
    /// 下拉弹层的固定内容宽度。
    pub(super) menu_width: f32,
    /// 弹层普通条目行高。
    pub(super) row_height: f32,
    /// 弹层分隔线条目行高。
    pub(super) divider_row_height: f32,
    /// 弹层条目内容起始缩进。
    pub(super) content_start: f32,
    /// 条目图标槽宽。
    pub(super) icon_slot_width: f32,
    /// 图标之后文字推进距离。
    pub(super) icon_advance: f32,
    /// 条目标签尾部留白。
    pub(super) label_end_padding: f32,
    /// 分隔线左右缩进。
    pub(super) divider_inset: f32,
    /// 分隔线粗细。
    pub(super) divider_thickness: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct MenuBarTypographyVisual {
    /// 顶级入口文字字号。
    pub(super) entry_label: f32,
    /// 弹层条目文字字号。
    pub(super) label: f32,
    /// 条目图标字号。
    pub(super) icon: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct MenuBarChromeVisual {
    /// 弹层边框粗细。
    pub(super) border_width: f32,
    /// 键盘焦点环粗细。
    pub(super) focus_width: f32,
    /// 脏区外扩距离，覆盖弹层阴影。
    pub(super) shadow_expand: f32,
    /// 弹层叠放层级，与 Dropdown 弹层一致。
    pub(super) overlay_z: i32,
    radius: MenuBarRadiusRole,
    shadow: MenuBarShadowRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MenuBarPaletteVisual {
    text: ColorValue,
    text_quaternary: ColorValue,
    entry_hover: ColorValue,
    entry_open: ColorValue,
    elevated_background: ColorValue,
    border: ColorValue,
    fill_tertiary: ColorValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MenuBarRadiusRole {
    Small,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MenuBarShadowRole {
    Secondary,
}

impl MenuBarRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

impl MenuBarShadowRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> ShadowToken {
        match self {
            Self::Secondary => tokens.box_shadow_secondary(),
        }
    }
}

/// 解析后的弹层绘制色与圆角。
pub(super) struct MenuBarChromeResolved {
    pub(super) radius: f32,
    pub(super) shadow: ShadowToken,
}

/// 解析后的入口调色。
pub(super) struct MenuBarEntryPalette {
    pub(super) text: Color,
    pub(super) text_quaternary: Color,
    pub(super) hover: Color,
    pub(super) open: Color,
}

/// 解析后的弹层调色。
pub(super) struct MenuBarMenuPalette {
    pub(super) background: Color,
    pub(super) border: Color,
    pub(super) text: Color,
    pub(super) text_quaternary: Color,
    pub(super) highlight: Color,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct MenuBarVisual {
    pub(super) layout: MenuBarLayoutVisual,
    pub(super) typography: MenuBarTypographyVisual,
    pub(super) chrome: MenuBarChromeVisual,
    pub(super) palette: MenuBarPaletteVisual,
}

impl MenuBarVisual {
    pub(super) fn resolve(&self, tokens: &dyn ThemeTokens) -> MenuBarVisualResolved {
        MenuBarVisualResolved {
            chrome: MenuBarChromeResolved {
                radius: self.chrome.radius.resolve(tokens),
                shadow: self.chrome.shadow.resolve(tokens),
            },
            entry: MenuBarEntryPalette {
                text: self.palette.text.resolve(tokens),
                text_quaternary: self.palette.text_quaternary.resolve(tokens),
                hover: self.palette.entry_hover.resolve(tokens),
                open: self.palette.entry_open.resolve(tokens),
            },
            menu: MenuBarMenuPalette {
                background: self.palette.elevated_background.resolve(tokens),
                border: self.palette.border.resolve(tokens),
                text: self.palette.text.resolve(tokens),
                text_quaternary: self.palette.text_quaternary.resolve(tokens),
                highlight: self.palette.fill_tertiary.resolve(tokens),
            },
        }
    }
}

pub(super) struct MenuBarVisualResolved {
    pub(super) chrome: MenuBarChromeResolved,
    pub(super) entry: MenuBarEntryPalette,
    pub(super) menu: MenuBarMenuPalette,
}

pub(super) const fn menu_bar_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}

pub(super) const fn menu_bar_text_quaternary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextQuaternary)
}

pub(super) const fn menu_bar_entry_hover() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}

pub(super) const fn menu_bar_entry_open() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillSecondary)
}

pub(super) const fn menu_bar_elevated_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}

pub(super) const fn menu_bar_border() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}

pub(super) const fn menu_bar_fill_tertiary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}

pub(super) const fn menu_bar_radius_small() -> MenuBarRadiusRole {
    MenuBarRadiusRole::Small
}

pub(super) const fn menu_bar_shadow_secondary() -> MenuBarShadowRole {
    MenuBarShadowRole::Secondary
}

// 同一份 UIX 在模块级生成静态视觉常量，供 Rust 生命周期引用。
crate::uix_items!("src/ui/widgets/navigation/menu_bar/menu_bar.uix");
