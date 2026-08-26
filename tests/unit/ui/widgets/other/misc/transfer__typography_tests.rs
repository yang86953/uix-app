// 引入被测私有排版值契约。
use super::TransferTypography;
// 引入可定制的主题 token 实现。
use crate::ui::theme::DesignTokens;

// 自定义排版 token 必须驱动标题与条目字号。
#[test]
fn transfer_typography_resolves_custom_theme_tokens() {
    // 从完整亮色主题建立测试 token。
    let mut tokens = DesignTokens::antd_light();
    // 覆写小号正文以证明组件没有保留固定 12px。
    tokens.font_size_sm = 10.0;
    // 覆写正文以证明紧凑条目字号由主题相邻 token 派生。
    tokens.font_size = 18.0;
    // 解析当前测试主题的组件排版。
    let typography = TransferTypography::resolve(&tokens);
    // 标题与搜索说明必须直接使用小号正文 token。
    assert_eq!(typography.caption, 10.0);
    // 条目字号必须是两个相邻 token 的中点。
    assert_eq!(typography.item, 14.0);
}
