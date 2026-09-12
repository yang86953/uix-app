use std::sync::Arc;

use crate::draw::Color;
use crate::ui::theme::traits::{IBoxShadowTokens, ISpacingTokens, ITypographyTokens, ThemeTokens};
use crate::ui::theme::{IColorTokens, ShadowToken};

/// A typed, partial override for design tokens.
///
/// Unset fields delegate to the active subtree theme. The struct is pure data,
/// so patches can be cloned, compared, and installed in a `ConfigProvider`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TokenPatch {
    /// 可选的品牌主色覆盖。
    pub color_primary: Option<Color>,
    /// 可选的品牌主色悬停态覆盖。
    pub color_primary_hover: Option<Color>,
    /// 可选的品牌主色激活态覆盖。
    pub color_primary_active: Option<Color>,
    /// 可选的品牌主色弱背景覆盖。
    pub color_primary_bg: Option<Color>,
    /// 可选的品牌主色边框覆盖。
    pub color_primary_border: Option<Color>,
    /// 可选的容器背景色覆盖。
    pub color_bg_container: Option<Color>,
    /// 可选的浮层背景色覆盖。
    pub color_bg_elevated: Option<Color>,
    /// 可选的抬升表面背景色覆盖。
    pub color_bg_raised: Option<Color>,
    /// 可选的覆盖层背景色覆盖。
    pub color_bg_overlay: Option<Color>,
    /// 可选的页面布局背景色覆盖。
    pub color_bg_layout: Option<Color>,
    /// 可选的聚光提示背景色覆盖。
    pub color_bg_spotlight: Option<Color>,
    /// 可选的模态遮罩背景色覆盖。
    pub color_bg_mask: Option<Color>,
    /// 可选的默认边框色覆盖。
    pub color_border: Option<Color>,
    /// 可选的次级边框色覆盖。
    pub color_border_secondary: Option<Color>,
    /// 可选的默认填充色覆盖。
    pub color_fill: Option<Color>,
    /// 可选的次级填充色覆盖。
    pub color_fill_secondary: Option<Color>,
    /// 可选的三级填充色覆盖。
    pub color_fill_tertiary: Option<Color>,
    /// 可选的四级填充色覆盖。
    pub color_fill_quaternary: Option<Color>,
    /// 可选的默认文本色覆盖。
    pub color_text: Option<Color>,
    /// 可选的次级文本色覆盖。
    pub color_text_secondary: Option<Color>,
    /// 可选的三级文本色覆盖。
    pub color_text_tertiary: Option<Color>,
    /// 可选的四级文本色覆盖。
    pub color_text_quaternary: Option<Color>,
    /// 可选的主题白色覆盖。
    pub color_white: Option<Color>,
    /// 可选的主题黑色覆盖。
    pub color_black: Option<Color>,
    /// 可选的主阴影颜色覆盖。
    pub color_shadow: Option<Color>,
    /// 可选的次级阴影颜色覆盖。
    pub color_shadow_secondary: Option<Color>,
    /// 可选的成功状态主色覆盖。
    pub color_success: Option<Color>,
    /// 可选的成功状态背景覆盖。
    pub color_success_bg: Option<Color>,
    /// 可选的成功状态边框覆盖。
    pub color_success_border: Option<Color>,
    /// 可选的警告状态主色覆盖。
    pub color_warning: Option<Color>,
    /// 可选的警告状态背景覆盖。
    pub color_warning_bg: Option<Color>,
    /// 可选的警告状态边框覆盖。
    pub color_warning_border: Option<Color>,
    /// 可选的错误状态主色覆盖。
    pub color_error: Option<Color>,
    /// 可选的错误状态背景覆盖。
    pub color_error_bg: Option<Color>,
    /// 可选的错误状态边框覆盖。
    pub color_error_border: Option<Color>,
    /// 可选的信息状态主色覆盖。
    pub color_info: Option<Color>,
    /// 可选的信息状态背景覆盖。
    pub color_info_bg: Option<Color>,
    /// 可选的信息状态边框覆盖。
    pub color_info_border: Option<Color>,
    /// 可选的默认链接色覆盖。
    pub color_link: Option<Color>,
    /// 可选的链接悬停色覆盖。
    pub color_link_hover: Option<Color>,
    /// 可选的链接激活色覆盖。
    pub color_link_active: Option<Color>,
    /// 可选的默认字体族覆盖。
    pub font_family: Option<&'static str>,
    /// 可选的小号正文字号覆盖。
    pub font_size_sm: Option<f32>,
    /// 可选的默认正文字号覆盖。
    pub font_size: Option<f32>,
    /// 可选的大号正文字号覆盖。
    pub font_size_lg: Option<f32>,
    /// 可选的超大正文字号覆盖。
    pub font_size_xl: Option<f32>,
    /// 可选的一级标题字号覆盖。
    pub font_size_heading_1: Option<f32>,
    /// 可选的二级标题字号覆盖。
    pub font_size_heading_2: Option<f32>,
    /// 可选的三级标题字号覆盖。
    pub font_size_heading_3: Option<f32>,
    /// 可选的四级标题字号覆盖。
    pub font_size_heading_4: Option<f32>,
    /// 可选的五级标题字号覆盖。
    pub font_size_heading_5: Option<f32>,
    /// 可选的常规字重覆盖。
    pub font_weight_regular: Option<f32>,
    /// 可选的中等字重覆盖。
    pub font_weight_medium: Option<f32>,
    /// 可选的半粗字重覆盖。
    pub font_weight_semibold: Option<f32>,
    /// 可选的粗体字重覆盖。
    pub font_weight_bold: Option<f32>,
    /// 可选的默认行高覆盖。
    pub line_height: Option<f32>,
    /// 可选的极小内边距覆盖。
    pub padding_xss: Option<f32>,
    /// 可选的超小内边距覆盖。
    pub padding_xs: Option<f32>,
    /// 可选的小内边距覆盖。
    pub padding_sm: Option<f32>,
    /// 可选的默认内边距覆盖。
    pub padding: Option<f32>,
    /// 可选的中等内边距覆盖。
    pub padding_md: Option<f32>,
    /// 可选的大内边距覆盖。
    pub padding_lg: Option<f32>,
    /// 可选的超大内边距覆盖。
    pub padding_xl: Option<f32>,
    /// 可选的默认圆角半径覆盖。
    pub border_radius: Option<f32>,
    /// 可选的小圆角半径覆盖。
    pub border_radius_sm: Option<f32>,
    /// 可选的大圆角半径覆盖。
    pub border_radius_lg: Option<f32>,
    /// 可选的超大圆角半径覆盖。
    pub border_radius_xl: Option<f32>,
    /// 可选的胶囊形圆角半径覆盖。
    pub border_radius_round: Option<f32>,
    /// 可选的小控件高度覆盖。
    pub control_height_sm: Option<f32>,
    /// 可选的默认控件高度覆盖。
    pub control_height: Option<f32>,
    /// 可选的大控件高度覆盖。
    pub control_height_lg: Option<f32>,
    /// 可选的覆盖层背景模糊半径覆盖。
    pub backdrop_blur_radius: Option<f32>,
    /// 可选的默认盒阴影覆盖。
    pub box_shadow: Option<ShadowToken>,
    /// 可选的次级盒阴影覆盖。
    pub box_shadow_secondary: Option<ShadowToken>,
    /// 可选的快速动效时长覆盖。
    pub motion_duration_fast: Option<f32>,
    /// 可选的中速动效时长覆盖。
    pub motion_duration_mid: Option<f32>,
    /// 可选的慢速动效时长覆盖。
    pub motion_duration_slow: Option<f32>,
    /// 可选的默认缓动表达式覆盖。
    pub motion_easing_default: Option<&'static str>,
    /// 可选的进入缓动表达式覆盖。
    pub motion_easing_in: Option<&'static str>,
    /// 可选的离开缓动表达式覆盖。
    pub motion_easing_out: Option<&'static str>,
    /// 可选的双向缓动表达式覆盖。
    pub motion_easing_in_out: Option<&'static str>,
    /// 可选的超小响应式断点覆盖。
    pub screen_xs: Option<f32>,
    /// 可选的小响应式断点覆盖。
    pub screen_sm: Option<f32>,
    /// 可选的中等响应式断点覆盖。
    pub screen_md: Option<f32>,
    /// 可选的大响应式断点覆盖。
    pub screen_lg: Option<f32>,
    /// 可选的超大响应式断点覆盖。
    pub screen_xl: Option<f32>,
    /// 可选的双倍超大响应式断点覆盖。
    pub screen_xxl: Option<f32>,
    /// 可选的深色主题标志覆盖。
    pub is_dark: Option<bool>,
}

pub(crate) struct ScopedThemeTokens {
    root: Arc<dyn ThemeTokens>,
    theme: Option<Arc<dyn ThemeTokens>>,
    patch: Option<Arc<TokenPatch>>,
}

pub(crate) struct TokenScope {
    pub theme: Option<Arc<dyn ThemeTokens>>,
    pub patch: Option<Arc<TokenPatch>>,
}

impl ScopedThemeTokens {
    pub(crate) fn new(root: Arc<dyn ThemeTokens>) -> Self {
        Self {
            root,
            theme: None,
            patch: None,
        }
    }

    pub(crate) fn replace_scope(
        &mut self,
        theme: Option<Arc<dyn ThemeTokens>>,
        patch: Option<Arc<TokenPatch>>,
    ) -> TokenScope {
        TokenScope {
            theme: std::mem::replace(&mut self.theme, theme),
            patch: std::mem::replace(&mut self.patch, patch),
        }
    }

    pub(crate) fn restore_scope(&mut self, scope: TokenScope) {
        self.theme = scope.theme;
        self.patch = scope.patch;
    }

    fn base(&self) -> &dyn ThemeTokens {
        self.theme.as_deref().unwrap_or(self.root.as_ref())
    }
}

macro_rules! patched_copy {
    ($self:ident, $field:ident) => {
        $self
            .patch
            .as_ref()
            .and_then(|patch| patch.$field)
            .unwrap_or_else(|| $self.base().$field())
    };
}

macro_rules! copy_methods {
    ($($field:ident -> $ty:ty;)*) => {
        $(
            fn $field(&self) -> $ty {
                patched_copy!(self, $field)
            }
        )*
    };
}

impl IColorTokens for ScopedThemeTokens {
    copy_methods! {
        color_primary -> Color;
        color_primary_hover -> Color;
        color_primary_active -> Color;
        color_primary_bg -> Color;
        color_primary_border -> Color;
        color_bg_container -> Color;
        color_bg_elevated -> Color;
        color_bg_raised -> Color;
        color_bg_overlay -> Color;
        color_bg_layout -> Color;
        color_bg_spotlight -> Color;
        color_bg_mask -> Color;
        color_border -> Color;
        color_border_secondary -> Color;
        color_fill -> Color;
        color_fill_secondary -> Color;
        color_fill_tertiary -> Color;
        color_fill_quaternary -> Color;
        color_text -> Color;
        color_text_secondary -> Color;
        color_text_tertiary -> Color;
        color_text_quaternary -> Color;
        color_white -> Color;
        color_black -> Color;
        color_shadow -> Color;
        color_shadow_secondary -> Color;
        color_success -> Color;
        color_success_bg -> Color;
        color_success_border -> Color;
        color_warning -> Color;
        color_warning_bg -> Color;
        color_warning_border -> Color;
        color_error -> Color;
        color_error_bg -> Color;
        color_error_border -> Color;
        color_info -> Color;
        color_info_bg -> Color;
        color_info_border -> Color;
        color_link -> Color;
        color_link_hover -> Color;
        color_link_active -> Color;
    }
}

impl ITypographyTokens for ScopedThemeTokens {
    fn font_family(&self) -> &str {
        self.patch
            .as_ref()
            .and_then(|patch| patch.font_family)
            .unwrap_or_else(|| self.base().font_family())
    }

    copy_methods! {
        font_size_sm -> f32;
        font_size -> f32;
        font_size_lg -> f32;
        font_size_xl -> f32;
        font_size_heading_1 -> f32;
        font_size_heading_2 -> f32;
        font_size_heading_3 -> f32;
        font_size_heading_4 -> f32;
        font_size_heading_5 -> f32;
        font_weight_regular -> f32;
        font_weight_medium -> f32;
        font_weight_semibold -> f32;
        font_weight_bold -> f32;
        line_height -> f32;
    }
}

impl ISpacingTokens for ScopedThemeTokens {
    copy_methods! {
        padding_xss -> f32;
        padding_xs -> f32;
        padding_sm -> f32;
        padding -> f32;
        padding_md -> f32;
        padding_lg -> f32;
        padding_xl -> f32;
        border_radius -> f32;
        border_radius_sm -> f32;
        border_radius_lg -> f32;
        border_radius_xl -> f32;
        border_radius_round -> f32;
        control_height_sm -> f32;
        control_height -> f32;
        control_height_lg -> f32;
        // 将组件级背景模糊覆盖投影到主题间距契约。
        backdrop_blur_radius -> f32;
        motion_duration_fast -> f32;
        motion_duration_mid -> f32;
        motion_duration_slow -> f32;
        screen_xs -> f32;
        screen_sm -> f32;
        screen_md -> f32;
        screen_lg -> f32;
        screen_xl -> f32;
        screen_xxl -> f32;
    }

    fn motion_easing_default(&self) -> &str {
        self.patch
            .as_ref()
            .and_then(|patch| patch.motion_easing_default)
            .unwrap_or_else(|| self.base().motion_easing_default())
    }

    fn motion_easing_in(&self) -> &str {
        self.patch
            .as_ref()
            .and_then(|patch| patch.motion_easing_in)
            .unwrap_or_else(|| self.base().motion_easing_in())
    }

    fn motion_easing_out(&self) -> &str {
        self.patch
            .as_ref()
            .and_then(|patch| patch.motion_easing_out)
            .unwrap_or_else(|| self.base().motion_easing_out())
    }

    fn motion_easing_in_out(&self) -> &str {
        self.patch
            .as_ref()
            .and_then(|patch| patch.motion_easing_in_out)
            .unwrap_or_else(|| self.base().motion_easing_in_out())
    }
}

impl IBoxShadowTokens for ScopedThemeTokens {
    copy_methods! {
        box_shadow -> ShadowToken;
        box_shadow_secondary -> ShadowToken;
    }
}

impl ThemeTokens for ScopedThemeTokens {
    fn is_dark(&self) -> bool {
        patched_copy!(self, is_dark)
    }
}

impl crate::ui::theme::traits::TokenProvider for ScopedThemeTokens {}
