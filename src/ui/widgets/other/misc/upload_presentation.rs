// 引入主题排版 token 契约。
use crate::ui::ThemeTokens;

// 列表区顶部相对控件的偏移（像素）。
pub(super) const LIST_TOP: f32 = 104.0;
// 文件行高（像素）。
pub(super) const FILE_ROW_H: f32 = 32.0;
// 列表右侧留白（像素），为状态图标区保留空间。
pub(super) const LIST_RIGHT_PAD: f32 = 56.0;
// 主说明字号位于小号正文与正文 token 的中点。
const PROMPT_FONT_MIDPOINT_WEIGHT: f32 = 0.5;
// 辅助说明字号相对小号正文的比例，默认主题下保持原 10px。
const SUPPORTING_FONT_SCALE: f32 = 5.0 / 6.0;

// 保存一次 Upload 绘制内解析出的排版值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct UploadTypography {
    // 拖放主说明使用紧凑正文。
    pub(super) prompt: f32,
    // 过滤条件与文件大小使用辅助说明字号。
    pub(super) supporting: f32,
    // 文件名使用主题小号正文。
    pub(super) file_name: f32,
}

// 将主题排版 token 转换为 Upload 私有绘制值。
impl UploadTypography {
    // 从当前组件主题作用域解析排版。
    pub(super) fn resolve(tokens: &dyn ThemeTokens) -> Self {
        // 读取主题拥有的小号正文字号。
        let small = tokens.font_size_sm();
        // 读取主题拥有的正文字号。
        let body = tokens.font_size();
        // 返回供当前绘制批次复用的稳定值。
        Self {
            // 默认主题下保持原 13px，同时跟随两个相邻 token 变化。
            prompt: small + (body - small) * PROMPT_FONT_MIDPOINT_WEIGHT,
            // 从小号正文按命名比例派生辅助说明字号。
            supporting: small * SUPPORTING_FONT_SCALE,
            // 文件名直接使用小号正文 token。
            file_name: small,
        }
    }
}

// 将字节数格式化为 Upload 文件列表的紧凑文案。
pub(super) fn format_file_size(bytes: u64) -> String {
    // 定义二进制 KiB 单位。
    const KIB: f64 = 1024.0;
    // 定义二进制 MiB 单位。
    const MIB: f64 = KIB * 1024.0;
    // MiB 及以上使用一位小数。
    if bytes >= MIB as u64 {
        // 返回 MiB 展示文案。
        format!("{:.1} MiB", bytes as f64 / MIB)
    // KiB 及以上使用一位小数。
    } else if bytes >= KIB as u64 {
        // 返回 KiB 展示文案。
        format!("{:.1} KiB", bytes as f64 / KIB)
    // 更小值直接显示字节数。
    } else {
        // 返回字节展示文案。
        format!("{bytes} B")
    }
}

// 验证 Upload presentation 值契约。
#[cfg(test)]
mod tests {
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
}
