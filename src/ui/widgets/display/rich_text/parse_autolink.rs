// 富文本尖括号 HTTP(S) 与邮件自动链接校验辅助。

// 解析受支持的尖括号自动链接，并返回显示文本与提交目标。
pub(super) fn parse_angle_autolink(text: &str) -> Option<(&str, String)> {
    // HTTP(S) 地址显示并提交同一个原始 URL。
    if let Some(url) = parse_http_autolink(text) {
        // 保留作者输入的 scheme 大小写与完整目标。
        return Some((url, url.to_owned()));
    }
    // 其余候选按 Markdown 邮件自动链接规则校验。
    let email = parse_email_autolink(text)?;
    // 邮箱显示原始地址，提交目标增加标准 mailto scheme。
    Some((email, format!("mailto:{email}")))
}

// 解析尖括号包裹的 HTTP(S) 自动链接，并返回不含尖括号的原始 URL。
fn parse_http_autolink(text: &str) -> Option<&str> {
    // 提取第一个完整尖括号中的候选 URL。
    let url = angle_candidate(text)?;
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

// 解析尖括号包裹的 Markdown 邮件自动链接。
fn parse_email_autolink(text: &str) -> Option<&str> {
    // 提取第一个完整尖括号中的候选邮箱。
    let email = angle_candidate(text)?;
    // 邮箱只能包含 ASCII 字符，避免未经定义的国际化地址语义。
    if !email.is_ascii() {
        // Unicode 地址保持普通文本。
        return None;
    }
    // 邮箱必须且只能包含一个分隔 local-part 与 domain 的 @。
    let (local, domain) = email.split_once('@')?;
    // domain 中再次出现 @ 表示候选不唯一。
    if domain.contains('@') {
        // 多个 @ 的输入保持普通文本。
        return None;
    }
    // local-part 与 domain 必须分别满足确定性边界。
    if !valid_email_local(local) || !valid_email_domain(domain) {
        // 任一部分无效时不创建可交互链接。
        return None;
    }
    // 返回保持作者原始大小写和字符的邮箱地址。
    Some(email)
}

// 校验邮件 local-part 的受支持 ASCII 字符与点号边界。
fn valid_email_local(local: &str) -> bool {
    // local-part 长度遵守常用邮箱上限，并且不能为空。
    if local.is_empty() || local.len() > 64 {
        // 空值或超长 local-part 不进入链接交互链。
        return false;
    }
    // 点号不能位于首尾，也不能连续出现。
    if local.starts_with('.') || local.ends_with('.') || local.contains("..") {
        // 歧义点号边界保持普通文本。
        return false;
    }
    // 只接受 Markdown 邮件自动链接允许的常见 ASCII atom 字符。
    local.chars().all(|ch| {
        // 字母数字以及受支持的可打印特殊字符均可进入 local-part。
        ch.is_ascii_alphanumeric()
            // 使用显式字符集合避免空白、控制符和结构分隔符混入。
            || matches!(
                // 检查当前 ASCII 字符。
                ch,
                // 点号与 RFC atom 常见特殊字符。
                '.' | '!' | '#' | '$' | '%' | '&' | '\'' | '*' | '+' | '-' | '/' | '=' | '?'
                    | '^' | '_' | '`' | '{' | '|' | '}' | '~'
            )
    })
}

// 校验邮件 domain 的点分 label 结构。
fn valid_email_domain(domain: &str) -> bool {
    // domain 必须非空且不超过 DNS 文本常用上限。
    if domain.is_empty() || domain.len() > 255 {
        // 空值或超长 domain 保持普通文本。
        return false;
    }
    // 每个点分 label 都必须独立满足 DNS 名称边界。
    domain.split('.').all(|label| {
        // label 必须包含一到六十三个 ASCII 字节。
        if label.is_empty() || label.len() > 63 {
            // 空 label、连续点号或超长 label 无效。
            return false;
        }
        // label 首尾必须是 ASCII 字母或数字。
        if !label
            // 读取首字符并验证起始边界。
            .chars()
            // 首字符不存在或不是字母数字时拒绝。
            .next()
            // 使用 ASCII 规则避免隐式国际化域名。
            .is_some_and(|ch| ch.is_ascii_alphanumeric())
            // 同时验证末字符边界。
            || !label
                // 读取末字符。
                .chars()
                // 从尾部取得最后一个 Unicode 标量。
                .next_back()
                // 末字符必须是 ASCII 字母或数字。
                .is_some_and(|ch| ch.is_ascii_alphanumeric())
        {
            // 首尾连字符等输入保持普通文本。
            return false;
        }
        // label 内部只允许 ASCII 字母数字和连字符。
        label
            // 遍历 label 的全部字符。
            .chars()
            // 任一其他字符都会让完整邮箱候选失效。
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    })
}

// 提取第一个完整尖括号内部的候选文本。
fn angle_candidate(text: &str) -> Option<&str> {
    // 自动链接必须从左尖括号开始。
    let remaining = text.strip_prefix('<')?;
    // 第一个右尖括号结束自动链接，未闭合输入保持字面文本。
    let close = remaining.find('>')?;
    // 返回不含尖括号的候选文本。
    Some(&remaining[..close])
}
