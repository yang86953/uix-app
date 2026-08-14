//! DesignTokens — concrete Ant Design 5 token preset with light & dark modes.
//!
//! Implements all token sub-traits (`IColorTokens`, `ITypographyTokens`,
//! `ISpacingTokens`, `IBoxShadowTokens`, `TokenProvider`) by delegating to
//! public fields. Factory functions `antd_light()` and `antd_dark()` provide
//! complete Ant Design 5 presets (defined in the `presets` submodule).

pub(crate) mod presets;
pub(crate) mod primitives;

use super::color_tokens::{IColorTokens, NeutralRole, ShadowToken};
use crate::draw::Color;
use crate::ui::theme::traits::{
    IBoxShadowTokens, ISpacingTokens, ITypographyTokens, ThemeTokens, TokenProvider,
};

#[derive(Debug, Clone)]
/// UIX 对外公开的完整设计令牌快照。
pub struct DesignTokens {
    /// 品牌主色。
    pub color_primary: Color,
    /// 品牌主色悬停态。
    pub color_primary_hover: Color,
    /// 品牌主色激活态。
    pub color_primary_active: Color,
    /// 品牌主色弱背景。
    pub color_primary_bg: Color,
    /// 品牌主色边框。
    pub color_primary_border: Color,
    /// 默认容器背景色。
    pub color_bg_container: Color,
    /// 浮层背景色。
    pub color_bg_elevated: Color,
    /// 抬升表面背景色。
    pub color_bg_raised: Color,
    /// 覆盖层背景色。
    pub color_bg_overlay: Color,
    /// 页面布局背景色。
    pub color_bg_layout: Color,
    /// 聚光提示背景色。
    pub color_bg_spotlight: Color,
    /// 模态遮罩背景色。
    pub color_bg_mask: Color,
    /// 默认边框色。
    pub color_border: Color,
    /// 次级边框色。
    pub color_border_secondary: Color,
    /// 默认填充色。
    pub color_fill: Color,
    /// 次级填充色。
    pub color_fill_secondary: Color,
    /// 三级填充色。
    pub color_fill_tertiary: Color,
    /// 四级填充色。
    pub color_fill_quaternary: Color,
    /// 默认文本色。
    pub color_text: Color,
    /// 次级文本色。
    pub color_text_secondary: Color,
    /// 三级文本色。
    pub color_text_tertiary: Color,
    /// 四级文本色。
    pub color_text_quaternary: Color,
    /// 主题白色。
    pub color_white: Color,
    /// 主题黑色。
    pub color_black: Color,
    /// 主阴影颜色。
    pub color_shadow: Color,
    /// 次级阴影颜色。
    pub color_shadow_secondary: Color,
    /// 成功状态主色。
    pub color_success: Color,
    /// 成功状态背景色。
    pub color_success_bg: Color,
    /// 成功状态边框色。
    pub color_success_border: Color,
    /// 警告状态主色。
    pub color_warning: Color,
    /// 警告状态背景色。
    pub color_warning_bg: Color,
    /// 警告状态边框色。
    pub color_warning_border: Color,
    /// 错误状态主色。
    pub color_error: Color,
    /// 错误状态背景色。
    pub color_error_bg: Color,
    /// 错误状态边框色。
    pub color_error_border: Color,
    /// 信息状态主色。
    pub color_info: Color,
    /// 信息状态背景色。
    pub color_info_bg: Color,
    /// 信息状态边框色。
    pub color_info_border: Color,
    /// 默认链接色。
    pub color_link: Color,
    /// 链接悬停色。
    pub color_link_hover: Color,
    /// 链接激活色。
    pub color_link_active: Color,
    /// 默认字体族。
    pub font_family: &'static str,
    /// 小号正文字号。
    pub font_size_sm: f32,
    /// 默认正文字号。
    pub font_size: f32,
    /// 大号正文字号。
    pub font_size_lg: f32,
    /// 超大正文字号。
    pub font_size_xl: f32,
    /// 一级标题字号。
    pub font_size_heading_1: f32,
    /// 二级标题字号。
    pub font_size_heading_2: f32,
    /// 三级标题字号。
    pub font_size_heading_3: f32,
    /// 四级标题字号。
    pub font_size_heading_4: f32,
    /// 五级标题字号。
    pub font_size_heading_5: f32,
    /// 常规字重。
    pub font_weight_regular: f32,
    /// 中等字重。
    pub font_weight_medium: f32,
    /// 半粗字重。
    pub font_weight_semibold: f32,
    /// 粗体字重。
    pub font_weight_bold: f32,
    /// 默认文本行高倍率。
    pub line_height: f32,
    /// 极小内边距。
    pub padding_xss: f32,
    /// 超小内边距。
    pub padding_xs: f32,
    /// 小内边距。
    pub padding_sm: f32,
    /// 默认内边距。
    pub padding: f32,
    /// 中等内边距。
    pub padding_md: f32,
    /// 大内边距。
    pub padding_lg: f32,
    /// 超大内边距。
    pub padding_xl: f32,
    /// 默认圆角半径。
    pub border_radius: f32,
    /// 小圆角半径。
    pub border_radius_sm: f32,
    /// 大圆角半径。
    pub border_radius_lg: f32,
    /// 超大圆角半径。
    pub border_radius_xl: f32,
    /// 胶囊形圆角半径。
    pub border_radius_round: f32,
    /// 小控件高度。
    pub control_height_sm: f32,
    /// 默认控件高度。
    pub control_height: f32,
    /// 大控件高度。
    pub control_height_lg: f32,
    /// overlay backdrop blur 的默认逻辑像素半径。
    pub backdrop_blur_radius: f32,
    /// 默认盒阴影。
    pub box_shadow: ShadowToken,
    /// 次级盒阴影。
    pub box_shadow_secondary: ShadowToken,
    /// 快速动效持续时间秒数。
    pub motion_duration_fast: f32,
    /// 中速动效持续时间秒数。
    pub motion_duration_mid: f32,
    /// 慢速动效持续时间秒数。
    pub motion_duration_slow: f32,
    /// 默认动效缓动表达式。
    pub motion_easing_default: &'static str,
    /// 进入动效缓动表达式。
    pub motion_easing_in: &'static str,
    /// 离开动效缓动表达式。
    pub motion_easing_out: &'static str,
    /// 双向动效缓动表达式。
    pub motion_easing_in_out: &'static str,
    /// 超小响应式断点。
    pub screen_xs: f32,
    /// 小响应式断点。
    pub screen_sm: f32,
    /// 中等响应式断点。
    pub screen_md: f32,
    /// 大响应式断点。
    pub screen_lg: f32,
    /// 超大响应式断点。
    pub screen_xl: f32,
    /// 双倍超大响应式断点。
    pub screen_xxl: f32,
    /// 是否为深色主题。
    pub is_dark: bool,
}

// ── Trait implementations ──

impl IColorTokens for DesignTokens {
    fn color_primary(&self) -> Color {
        self.color_primary
    }
    fn color_primary_hover(&self) -> Color {
        self.color_primary_hover
    }
    fn color_primary_active(&self) -> Color {
        self.color_primary_active
    }
    fn color_primary_bg(&self) -> Color {
        self.color_primary_bg
    }
    fn color_primary_border(&self) -> Color {
        self.color_primary_border
    }
    fn color_bg_container(&self) -> Color {
        self.color_bg_container
    }
    fn color_bg_elevated(&self) -> Color {
        self.color_bg_elevated
    }
    fn color_bg_raised(&self) -> Color {
        self.color_bg_raised
    }
    fn color_bg_overlay(&self) -> Color {
        self.color_bg_overlay
    }
    fn color_bg_layout(&self) -> Color {
        self.color_bg_layout
    }
    fn color_bg_spotlight(&self) -> Color {
        self.color_bg_spotlight
    }
    fn color_bg_mask(&self) -> Color {
        self.color_bg_mask
    }
    fn color_border(&self) -> Color {
        self.color_border
    }
    fn color_border_secondary(&self) -> Color {
        self.color_border_secondary
    }
    fn color_fill(&self) -> Color {
        self.color_fill
    }
    fn color_fill_secondary(&self) -> Color {
        self.color_fill_secondary
    }
    fn color_fill_tertiary(&self) -> Color {
        self.color_fill_tertiary
    }
    fn color_fill_quaternary(&self) -> Color {
        self.color_fill_quaternary
    }
    fn color_text(&self) -> Color {
        self.color_text
    }
    fn color_text_secondary(&self) -> Color {
        self.color_text_secondary
    }
    fn color_text_tertiary(&self) -> Color {
        self.color_text_tertiary
    }
    fn color_text_quaternary(&self) -> Color {
        self.color_text_quaternary
    }
    fn color_white(&self) -> Color {
        self.color_white
    }
    fn color_black(&self) -> Color {
        self.color_black
    }
    fn color_shadow(&self) -> Color {
        self.color_shadow
    }
    fn color_shadow_secondary(&self) -> Color {
        self.color_shadow_secondary
    }
    fn color_success(&self) -> Color {
        self.color_success
    }
    fn color_success_bg(&self) -> Color {
        self.color_success_bg
    }
    fn color_success_border(&self) -> Color {
        self.color_success_border
    }
    fn color_warning(&self) -> Color {
        self.color_warning
    }
    fn color_warning_bg(&self) -> Color {
        self.color_warning_bg
    }
    fn color_warning_border(&self) -> Color {
        self.color_warning_border
    }
    fn color_error(&self) -> Color {
        self.color_error
    }
    fn color_error_bg(&self) -> Color {
        self.color_error_bg
    }
    fn color_error_border(&self) -> Color {
        self.color_error_border
    }
    fn color_info(&self) -> Color {
        self.color_info
    }
    fn color_info_bg(&self) -> Color {
        self.color_info_bg
    }
    fn color_info_border(&self) -> Color {
        self.color_info_border
    }
    fn color_link(&self) -> Color {
        self.color_link
    }
    fn color_link_hover(&self) -> Color {
        self.color_link_hover
    }
    fn color_link_active(&self) -> Color {
        self.color_link_active
    }
}

impl ITypographyTokens for DesignTokens {
    fn font_family(&self) -> &str {
        self.font_family
    }
    fn font_size_sm(&self) -> f32 {
        self.font_size_sm
    }
    fn font_size(&self) -> f32 {
        self.font_size
    }
    fn font_size_lg(&self) -> f32 {
        self.font_size_lg
    }
    fn font_size_xl(&self) -> f32 {
        self.font_size_xl
    }
    fn font_size_heading_1(&self) -> f32 {
        self.font_size_heading_1
    }
    fn font_size_heading_2(&self) -> f32 {
        self.font_size_heading_2
    }
    fn font_size_heading_3(&self) -> f32 {
        self.font_size_heading_3
    }
    fn font_size_heading_4(&self) -> f32 {
        self.font_size_heading_4
    }
    fn font_size_heading_5(&self) -> f32 {
        self.font_size_heading_5
    }
    fn font_weight_regular(&self) -> f32 {
        self.font_weight_regular
    }
    fn font_weight_medium(&self) -> f32 {
        self.font_weight_medium
    }
    fn font_weight_semibold(&self) -> f32 {
        self.font_weight_semibold
    }
    fn font_weight_bold(&self) -> f32 {
        self.font_weight_bold
    }
    fn line_height(&self) -> f32 {
        self.line_height
    }
}

impl ISpacingTokens for DesignTokens {
    fn padding_xss(&self) -> f32 {
        self.padding_xss
    }
    fn padding_xs(&self) -> f32 {
        self.padding_xs
    }
    fn padding_sm(&self) -> f32 {
        self.padding_sm
    }
    fn padding(&self) -> f32 {
        self.padding
    }
    fn padding_md(&self) -> f32 {
        self.padding_md
    }
    fn padding_lg(&self) -> f32 {
        self.padding_lg
    }
    fn padding_xl(&self) -> f32 {
        self.padding_xl
    }
    fn border_radius(&self) -> f32 {
        self.border_radius
    }
    fn border_radius_sm(&self) -> f32 {
        self.border_radius_sm
    }
    fn border_radius_lg(&self) -> f32 {
        self.border_radius_lg
    }
    fn border_radius_xl(&self) -> f32 {
        self.border_radius_xl
    }
    fn border_radius_round(&self) -> f32 {
        self.border_radius_round
    }
    fn control_height_sm(&self) -> f32 {
        self.control_height_sm
    }
    fn control_height(&self) -> f32 {
        self.control_height
    }
    fn control_height_lg(&self) -> f32 {
        self.control_height_lg
    }
    // 将可定制的 DesignTokens 字段投影到主题契约。
    fn backdrop_blur_radius(&self) -> f32 {
        // 返回当前主题的 overlay blur 默认值。
        self.backdrop_blur_radius
    }
    fn motion_duration_fast(&self) -> f32 {
        self.motion_duration_fast
    }
    fn motion_duration_mid(&self) -> f32 {
        self.motion_duration_mid
    }
    fn motion_duration_slow(&self) -> f32 {
        self.motion_duration_slow
    }
    fn motion_easing_default(&self) -> &str {
        self.motion_easing_default
    }
    fn motion_easing_in(&self) -> &str {
        self.motion_easing_in
    }
    fn motion_easing_out(&self) -> &str {
        self.motion_easing_out
    }
    fn motion_easing_in_out(&self) -> &str {
        self.motion_easing_in_out
    }
    fn screen_xs(&self) -> f32 {
        self.screen_xs
    }
    fn screen_sm(&self) -> f32 {
        self.screen_sm
    }
    fn screen_md(&self) -> f32 {
        self.screen_md
    }
    fn screen_lg(&self) -> f32 {
        self.screen_lg
    }
    fn screen_xl(&self) -> f32 {
        self.screen_xl
    }
    fn screen_xxl(&self) -> f32 {
        self.screen_xxl
    }
}

impl IBoxShadowTokens for DesignTokens {
    fn box_shadow(&self) -> ShadowToken {
        self.box_shadow
    }
    fn box_shadow_secondary(&self) -> ShadowToken {
        self.box_shadow_secondary
    }
}

impl ThemeTokens for DesignTokens {
    fn is_dark(&self) -> bool {
        self.is_dark
    }
}

impl TokenProvider for DesignTokens {}

impl DesignTokens {
    /// 解析指定中性色角色的最终颜色。
    pub fn neutral(&self, role: NeutralRole) -> Color {
        match role {
            NeutralRole::Text => self.color_text,
            NeutralRole::TextSecondary => self.color_text_secondary,
            NeutralRole::TextTertiary => self.color_text_tertiary,
            NeutralRole::TextQuaternary => self.color_text_quaternary,
            NeutralRole::TextInverse => {
                if self.is_dark {
                    self.color_black
                } else {
                    self.color_white
                }
            }
            NeutralRole::Border => self.color_border,
            NeutralRole::BorderSecondary => self.color_border_secondary,
            NeutralRole::Fill => self.color_fill,
            NeutralRole::FillSecondary => self.color_fill_secondary,
            NeutralRole::FillTertiary => self.color_fill_tertiary,
            NeutralRole::FillQuaternary => self.color_fill_quaternary,
            NeutralRole::BgContainer => self.color_bg_container,
            NeutralRole::BgElevated => self.color_bg_elevated,
            NeutralRole::BgLayout => self.color_bg_layout,
            NeutralRole::BgMask => self.color_bg_mask,
        }
    }
}
