// 引入被测排版解析与文件大小格式化入口。
use super::{UploadTypography, format_file_size};
// 引入可定制的主题 token 实现。
use crate::ui::theme::DesignTokens;

// 自定义排版 token 必须驱动 Upload 的三层文字层级。
#[test]
fn upload_presentation_resolves_theme_typography() {
    // 从完整亮色主题建立测试 token。
    let mut tokens = DesignTokens::antd_light();
    // 覆写小号正文以证明组件没有保留固定 10/12px。
    tokens.font_size_sm = 12.0;
    // 覆写正文以证明主说明由相邻 token 派生。
    tokens.font_size = 16.0;
    // 解析当前测试主题的组件排版。
    let typography = UploadTypography::resolve(&tokens);
    // 主说明必须位于相邻 token 中点。
    assert_eq!(typography.prompt, 14.0);
    // 辅助说明必须按命名比例从小号正文派生。
    assert_eq!(typography.supporting, 10.0);
    // 文件名必须直接采用小号正文 token。
    assert_eq!(typography.file_name, 12.0);
    // presentation 拆分后仍必须保持既有文件大小文案。
    assert_eq!(format_file_size(1024), "1.0 KiB");
}
