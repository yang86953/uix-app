//! Tooltip 的 UIX 静态视觉契约与主题解析。

use std::sync::OnceLock;

use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::widgets::overlay_types::TooltipPlacement;
use crate::ui::widgets::tooltip_primitives::TooltipBubbleVisual;
pub(crate) use crate::ui::widgets::tooltip_primitives::tooltip_bubble_visual;

// 保存 UIX 声明的固有尺寸与可由 Rust 调用方覆盖的默认值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TooltipDefaultsVisual {
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) placement: TooltipPlacement,
    pub(crate) arrow: bool,
}

// 保存 Tooltip 进入与退出的静态时序。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TooltipMotionVisual {
    pub(crate) enter_duration: f64,
    pub(crate) exit_duration: f64,
}

// 保存 Tooltip 的主题角色与背景透明度。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TooltipPaletteVisual {
    background: ColorValue,
    text: ColorValue,
    background_alpha: u8,
}

// 全部 Tooltip 实例共享的完整静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TooltipVisual {
    pub(crate) defaults: TooltipDefaultsVisual,
    pub(crate) bubble: TooltipBubbleVisual,
    pub(crate) motion: TooltipMotionVisual,
    pub(crate) overlay_z: i32,
    palette: TooltipPaletteVisual,
}

// 保存 Tooltip 每帧一次解析得到的主题颜色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResolvedTooltipVisual {
    pub(crate) background: Color,
    pub(crate) text: Color,
}

impl TooltipVisual {
    pub(crate) fn resolve(self, tokens: &dyn ThemeTokens) -> ResolvedTooltipVisual {
        ResolvedTooltipVisual {
            background: self
                .palette
                .background
                .resolve(tokens)
                .with_alpha(self.palette.background_alpha),
            text: self.palette.text.resolve(tokens),
        }
    }
}

pub(crate) const fn tooltip_defaults_visual(
    width: f32,
    height: f32,
    placement: TooltipPlacement,
    arrow: bool,
) -> TooltipDefaultsVisual {
    TooltipDefaultsVisual {
        width,
        height,
        placement,
        arrow,
    }
}

pub(crate) const fn tooltip_motion_visual(
    enter_duration: f64,
    exit_duration: f64,
) -> TooltipMotionVisual {
    TooltipMotionVisual {
        enter_duration,
        exit_duration,
    }
}

pub(crate) const fn tooltip_palette_visual(
    background: ColorValue,
    text: ColorValue,
    background_alpha: f32,
) -> TooltipPaletteVisual {
    TooltipPaletteVisual {
        background,
        text,
        background_alpha: background_alpha as u8,
    }
}

pub(crate) const fn tooltip_visual(
    defaults: TooltipDefaultsVisual,
    bubble: TooltipBubbleVisual,
    motion: TooltipMotionVisual,
    overlay_z: f32,
    palette: TooltipPaletteVisual,
) -> TooltipVisual {
    TooltipVisual {
        defaults,
        bubble,
        motion,
        overlay_z: overlay_z as i32,
        palette,
    }
}

// 向 UIX 静态模板提供零分配默认方向。
pub(crate) const fn tooltip_placement_top() -> TooltipPlacement {
    TooltipPlacement::Top
}

// 向 UIX 静态模板提供主题黑色角色。
pub(crate) const fn tooltip_black() -> ColorValue {
    ColorValue::Palette(PaletteColor::Black)
}

// 向 UIX 静态模板提供主题白色角色。
pub(crate) const fn tooltip_white() -> ColorValue {
    ColorValue::Palette(PaletteColor::White)
}

pub(crate) static DEFAULT_TOOLTIP_VISUAL: TooltipVisual = tooltip_visual(
    tooltip_defaults_visual(80.0, 28.0, TooltipPlacement::Top, true),
    tooltip_bubble_visual(12.0, 16.0, 26.0, 6.0, 2.0, 4.0, 4.0, 1.0, 0.5, 2.0, 5.0),
    tooltip_motion_visual(0.15, 0.1),
    1100.0,
    tooltip_palette_visual(
        ColorValue::Palette(PaletteColor::Black),
        ColorValue::Palette(PaletteColor::White),
        230.0,
    ),
);

// 首次 UIX 构建固化声明值，全部 Tooltip 实例共享一份视觉表。
pub(crate) static UIX_TOOLTIP_VISUAL: OnceLock<TooltipVisual> = OnceLock::new();
