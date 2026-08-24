// RichText 布局消费的主题颜色投影。

// 引入布局最终消费的解析颜色。
use crate::draw::Color;
// 测试兼容入口读取 UI 主题的公开聚合契约。
#[cfg(test)]
use crate::ui::ThemeTokens;

// 定义无主题测量路径使用的代码背景透明度。
const ESTIMATED_CODE_BACKGROUND_ALPHA: u8 = 24;

// 保存一次布局所需的全部语义颜色，避免布局模块反向依赖主题状态。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RichTextPalette {
    // 保存普通文本的解析颜色。
    pub(crate) default_text: Color,
    // 保存代码文本的语义颜色。
    pub(crate) code_text: Color,
    // 保存代码背景的语义颜色。
    pub(crate) code_background: Color,
    // 保存链接文本的语义颜色。
    pub(crate) link: Color,
}

// 提供测量占位与真实主题两种窄构造入口。
impl RichTextPalette {
    // 为没有 PaintContext 的测量路径构造不影响几何的稳定占位色。
    pub(crate) fn estimated(default_text: Color) -> Self {
        // 所有颜色都从调用方默认色派生，不在组件内引入固定色相。
        Self {
            // 普通文本沿用调用方默认色。
            default_text,
            // 代码文本沿用调用方默认色。
            code_text: default_text,
            // 代码背景只派生透明度，布局几何不依赖该值。
            code_background: default_text.with_alpha(ESTIMATED_CODE_BACKGROUND_ALPHA),
            // 链接在测量阶段沿用调用方默认色。
            link: default_text,
        }
    }

    // 从当前主题作用域投影真实绘制颜色。
    #[cfg(test)]
    pub(crate) fn from_tokens(default_text: Color, tokens: &dyn ThemeTokens) -> Self {
        // 只读取语义 token，不缓存或拥有主题实例。
        Self {
            // 普通文本保留显式颜色优先级解析结果。
            default_text,
            // 代码文字使用主题正文色。
            code_text: tokens.color_text(),
            // 代码底色使用主题次级填充色。
            code_background: tokens.color_fill_secondary(),
            // 链接使用主题链接色。
            link: tokens.color_link(),
        }
    }
}

// 验证调色板只消费主题语义颜色。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/other/rich_text/rich_text_palette__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
