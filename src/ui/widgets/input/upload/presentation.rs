//! Upload 的 UIX 静态视觉契约与主题解析。

use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};

// 保存上传区、文件列表、缩略图、图标与进度条的静态几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct UploadLayoutVisual {
    pub(super) intrinsic_width: f32,
    pub(super) dropzone_height: f32,
    pub(super) list_top: f32,
    pub(super) file_row_height: f32,
    pub(super) list_right_padding: f32,
    pub(super) hover_inset: f32,
    pub(super) hover_width_reduction: f32,
    pub(super) hover_height: f32,
    pub(super) upload_icon_half_offset: f32,
    pub(super) upload_icon_y: f32,
    pub(super) upload_icon_width: f32,
    pub(super) upload_icon_height: f32,
    pub(super) prompt_half_offset: f32,
    pub(super) prompt_y: f32,
    pub(super) supporting_half_offset: f32,
    pub(super) supporting_y: f32,
    pub(super) thumbnail_inset: f32,
    pub(super) thumbnail_size: f32,
    pub(super) preview_text_inset: f32,
    pub(super) fallback_text_inset: f32,
    pub(super) file_icon_x: f32,
    pub(super) file_icon_width: f32,
    pub(super) file_icon_height: f32,
    pub(super) file_name_y: f32,
    pub(super) file_size_y: f32,
    pub(super) progress_y: f32,
    pub(super) progress_height: f32,
    pub(super) status_icon_right: f32,
    pub(super) remove_icon_right: f32,
    pub(super) row_icon_y: f32,
    pub(super) row_icon_width: f32,
    pub(super) row_icon_height: f32,
    pub(super) preview_min_device_side: f32,
    pub(super) preview_max_device_side: f32,
    pub(super) preview_radius: f32,
}

// 保存字号派生参数与图标字号。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct UploadTypographyVisual {
    pub(super) prompt_midpoint_weight: f32,
    pub(super) supporting_scale: f32,
    pub(super) upload_icon_size: f32,
    pub(super) file_icon_size: f32,
    pub(super) row_icon_size: f32,
}

// 保存上传区各状态描边和圆角角色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct UploadChromeVisual {
    pub(super) border_width: f32,
    pub(super) drag_border_width: f32,
    pub(super) focus_border_width: f32,
    pub(super) hover_inner_border_width: f32,
    root_radius: UploadRadiusRole,
    hover_radius: UploadRadiusRole,
}

// 保存 Upload 使用的图标名称。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct UploadIconsVisual {
    pub(super) upload: &'static str,
    pub(super) file: &'static str,
    pub(super) done: &'static str,
    pub(super) error: &'static str,
    pub(super) pending: &'static str,
    pub(super) uploading: &'static str,
    pub(super) remove: &'static str,
}

// 保存 Upload 使用的全部主题颜色角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct UploadPaletteVisual {
    background: ColorValue,
    border: ColorValue,
    text: ColorValue,
    text_quaternary: ColorValue,
    primary: ColorValue,
    error: ColorValue,
    success: ColorValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UploadRadiusRole {
    Default,
    Small,
}

impl UploadRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Default => tokens.border_radius(),
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

// 保存一次 Upload 绘制内解析出的排版值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct UploadTypography {
    pub(super) prompt: f32,
    pub(super) supporting: f32,
    pub(super) file_name: f32,
}

impl UploadTypography {
    // 兼容既有排版测试入口，派生比例仍由 UIX 唯一声明。
    pub(super) fn resolve(tokens: &dyn ThemeTokens) -> Self {
        Self::resolve_with_visual(tokens, UPLOAD_VISUAL_REF.typography)
    }

    fn resolve_with_visual(tokens: &dyn ThemeTokens, visual: UploadTypographyVisual) -> Self {
        let small = tokens.font_size_sm();
        let body = tokens.font_size();
        Self {
            prompt: small + (body - small) * visual.prompt_midpoint_weight,
            supporting: small * visual.supporting_scale,
            file_name: small,
        }
    }
}

// 全部 Upload 实例共享的完整 UIX 静态视觉表。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct UploadVisual {
    pub(super) layout: UploadLayoutVisual,
    typography: UploadTypographyVisual,
    pub(super) chrome: UploadChromeVisual,
    pub(super) icons: UploadIconsVisual,
    palette: UploadPaletteVisual,
}

crate::uix_items!("src/ui/widgets/input/upload/upload.uix");

// 保存上传区和全部文件行一次解析共享的主题值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ResolvedUploadVisual {
    pub(super) background: Color,
    pub(super) border: Color,
    pub(super) text: Color,
    pub(super) text_quaternary: Color,
    pub(super) primary: Color,
    pub(super) error: Color,
    pub(super) success: Color,
    pub(super) root_radius: f32,
    pub(super) hover_radius: f32,
    pub(super) typography: UploadTypography,
    pub(super) upload_icon_size: f32,
    pub(super) file_icon_size: f32,
    pub(super) row_icon_size: f32,
}

impl UploadVisual {
    pub(super) fn resolve(&self, tokens: &dyn ThemeTokens) -> ResolvedUploadVisual {
        ResolvedUploadVisual {
            background: self.palette.background.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            text_quaternary: self.palette.text_quaternary.resolve(tokens),
            primary: self.palette.primary.resolve(tokens),
            error: self.palette.error.resolve(tokens),
            success: self.palette.success.resolve(tokens),
            root_radius: self.chrome.root_radius.resolve(tokens),
            hover_radius: self.chrome.hover_radius.resolve(tokens),
            typography: UploadTypography::resolve_with_visual(tokens, self.typography),
            upload_icon_size: self.typography.upload_icon_size,
            file_icon_size: self.typography.file_icon_size,
            row_icon_size: self.typography.row_icon_size,
        }
    }
}

pub(super) const fn upload_radius_default() -> UploadRadiusRole {
    UploadRadiusRole::Default
}
pub(super) const fn upload_radius_small() -> UploadRadiusRole {
    UploadRadiusRole::Small
}
pub(super) const fn upload_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
pub(super) const fn upload_border() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}
pub(super) const fn upload_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(super) const fn upload_text_quaternary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextQuaternary)
}
pub(super) const fn upload_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(super) const fn upload_error() -> ColorValue {
    ColorValue::Palette(PaletteColor::Error)
}
pub(super) const fn upload_success() -> ColorValue {
    ColorValue::Palette(PaletteColor::Success)
}

// 将字节数格式化为 Upload 文件列表的紧凑文案。
pub(super) fn format_file_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    if bytes >= MIB as u64 {
        format!("{:.1} MiB", bytes as f64 / MIB)
    } else if bytes >= KIB as u64 {
        format!("{:.1} KiB", bytes as f64 / KIB)
    } else {
        format!("{bytes} B")
    }
}
