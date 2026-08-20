//! Markdown 内联代码正文规范化。

/// 按 Markdown code span 规则处理一对外围 ASCII 空格。
pub(super) fn normalize_code_span(content: &str) -> String {
    // 记录正文是否包含至少一个非空格字符，避免清空纯空格代码。
    let has_non_space = content.bytes().any(|byte| byte != b' ');
    // 只有首尾都有空格且正文并非纯空格时才移除外围空格。
    if content.starts_with(' ') && content.ends_with(' ') && has_non_space {
        // 首尾 ASCII 空格各占一个字节，可以安全截取中间的 UTF-8 正文。
        return content[1..content.len() - 1].to_string();
    }
    // 其他正文保持原样，保留单侧空格和纯空格内容。
    content.to_string()
}
