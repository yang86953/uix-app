//! Color token trait — brand, neutral, semantic, shadow, and link colors.

use crate::draw::Color;

/// 盒阴影令牌（Ant Design 5 多层阴影）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowToken {
    /// 最接近表面的阴影层，依次保存水平偏移、垂直偏移、模糊半径和颜色。
    pub layer_1: (f32, f32, f32, Color),
    /// 中间阴影层，依次保存水平偏移、垂直偏移、模糊半径和颜色。
    pub layer_2: (f32, f32, f32, Color),
    /// 最外侧阴影层，依次保存水平偏移、垂直偏移、模糊半径和颜色。
    pub layer_3: (f32, f32, f32, Color),
}

impl ShadowToken {
    /// 返回三个层级均完全透明的阴影令牌。
    pub const fn none() -> Self {
        Self {
            layer_1: (0.0, 0.0, 0.0, Color::transparent()),
            layer_2: (0.0, 0.0, 0.0, Color::transparent()),
            layer_3: (0.0, 0.0, 0.0, Color::transparent()),
        }
    }
}

/// 颜色设计令牌。
pub trait IColorTokens: Send + Sync {
    /// 返回品牌主色。
    fn color_primary(&self) -> Color;
    /// 返回品牌主色悬停态。
    fn color_primary_hover(&self) -> Color;
    /// 返回品牌主色激活态。
    fn color_primary_active(&self) -> Color;
    /// 返回品牌主色弱背景。
    fn color_primary_bg(&self) -> Color;
    /// 返回品牌主色边框。
    fn color_primary_border(&self) -> Color;
    /// 返回默认容器背景色。
    fn color_bg_container(&self) -> Color;
    /// 返回浮层背景色。
    fn color_bg_elevated(&self) -> Color;
    /// 返回抬升表面背景色。
    fn color_bg_raised(&self) -> Color;
    /// 返回覆盖层背景色。
    fn color_bg_overlay(&self) -> Color;
    /// 返回页面布局背景色。
    fn color_bg_layout(&self) -> Color;
    /// 返回聚光提示背景色。
    fn color_bg_spotlight(&self) -> Color;
    /// 返回模态遮罩背景色。
    fn color_bg_mask(&self) -> Color;
    /// 返回默认边框色。
    fn color_border(&self) -> Color;
    /// 返回次级边框色。
    fn color_border_secondary(&self) -> Color;
    /// 返回默认填充色。
    fn color_fill(&self) -> Color;
    /// 返回次级填充色。
    fn color_fill_secondary(&self) -> Color;
    /// 返回三级填充色。
    fn color_fill_tertiary(&self) -> Color;
    /// 返回四级填充色。
    fn color_fill_quaternary(&self) -> Color;
    /// 返回默认文本色。
    fn color_text(&self) -> Color;
    /// 返回次级文本色。
    fn color_text_secondary(&self) -> Color;
    /// 返回三级文本色。
    fn color_text_tertiary(&self) -> Color;
    /// 返回四级文本色。
    fn color_text_quaternary(&self) -> Color;
    /// 返回主题白色。
    fn color_white(&self) -> Color;
    /// 返回主题黑色。
    fn color_black(&self) -> Color;
    /// 返回主阴影颜色。
    fn color_shadow(&self) -> Color;
    /// 返回次级阴影颜色。
    fn color_shadow_secondary(&self) -> Color;
    /// 返回成功状态主色。
    fn color_success(&self) -> Color;
    /// 返回成功状态背景色。
    fn color_success_bg(&self) -> Color;
    /// 返回成功状态边框色。
    fn color_success_border(&self) -> Color;
    /// 返回警告状态主色。
    fn color_warning(&self) -> Color;
    /// 返回警告状态背景色。
    fn color_warning_bg(&self) -> Color;
    /// 返回警告状态边框色。
    fn color_warning_border(&self) -> Color;
    /// 返回错误状态主色。
    fn color_error(&self) -> Color;
    /// 返回错误状态背景色。
    fn color_error_bg(&self) -> Color;
    /// 返回错误状态边框色。
    fn color_error_border(&self) -> Color;
    /// 返回信息状态主色。
    fn color_info(&self) -> Color;
    /// 返回信息状态背景色。
    fn color_info_bg(&self) -> Color;
    /// 返回信息状态边框色。
    fn color_info_border(&self) -> Color;
    /// 返回默认链接色。
    fn color_link(&self) -> Color;
    /// 返回链接悬停色。
    fn color_link_hover(&self) -> Color;
    /// 返回链接激活色。
    fn color_link_active(&self) -> Color;
}

/// Neutral semantic roles mapped onto the concrete token fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NeutralRole {
    /// 默认文本角色。
    Text,
    /// 次级文本角色。
    TextSecondary,
    /// 三级文本角色。
    TextTertiary,
    /// 四级文本角色。
    TextQuaternary,
    /// 反色文本角色。
    TextInverse,
    /// 默认边框角色。
    Border,
    /// 次级边框角色。
    BorderSecondary,
    /// 默认填充角色。
    Fill,
    /// 次级填充角色。
    FillSecondary,
    /// 三级填充角色。
    FillTertiary,
    /// 四级填充角色。
    FillQuaternary,
    /// 默认容器背景角色。
    BgContainer,
    /// 浮层背景角色。
    BgElevated,
    /// 页面布局背景角色。
    BgLayout,
    /// 模态遮罩背景角色。
    BgMask,
}
