// 富文本尖括号 HTTP(S) 自动链接校验辅助。

// 解析尖括号包裹的 HTTP(S) 自动链接，并返回不含尖括号的原始 URL。
pub(super) fn parse_http_autolink(text: &str) -> Option<&str> {
    // 自动链接必须从左尖括号开始。
    let remaining = text.strip_prefix('<')?;
    // 第一个右尖括号结束自动链接，未闭合输入保持字面文本。
    let close = remaining.find('>')?;
    // 提取尖括号内部的候选 URL。
    let url = &remaining[..close];
    // HTTP 与 HTTPS scheme 按 ASCII 大小写不敏感匹配，同时保留原文。
    let scheme_len = if url
        // 安全读取 HTTPS scheme 的 ASCII 前缀。
        .get(..8)
        // 仅在前缀完整匹配时接受 HTTPS。
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https://"))
    {
        // HTTPS scheme 固定占八个 ASCII 字节。
        8
    } else if url
        // 安全读取 HTTP scheme 的 ASCII 前缀。
        .get(..7)
        // 仅在前缀完整匹配时接受 HTTP。
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("http://"))
    {
        // HTTP scheme 固定占七个 ASCII 字节。
        7
    } else {
        // 其他 scheme 继续作为普通文本处理。
        return None;
    };
    // scheme 后必须存在目标内容，避免创建空交互链接。
    if url.len() == scheme_len {
        // 空目标保持作者输入的字面形式。
        return None;
    }
    // 自动链接内部禁止空白、控制字符和嵌套左尖括号。
    if url
        // 逐 Unicode 字符检查确定性边界。
        .chars()
        // 任一非法字符都会让整个候选保持字面文本。
        .any(|ch| ch.is_whitespace() || ch.is_control() || ch == '<')
    {
        // 不完整或歧义目标不进入链接交互链。
        return None;
    }
    // 返回已验证且保持原始拼写的 URL。
    Some(url)
}
