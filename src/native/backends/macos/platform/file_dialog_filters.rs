// 引入集合以按首次出现顺序去除重复扩展名。
use std::collections::HashSet;

// 将 UIX 现有分号、空白、竖线或 Win32 NUL 分隔过滤器转换为 AppKit 扩展名列表。
pub(crate) fn parse_allowed_file_types(filters: &str) -> Vec<String> {
    // 保存已经按大小写折叠观察到的扩展名。
    let mut seen = HashSet::new();
    // 保存稳定顺序的 AppKit allowedFileTypes 结果。
    let mut allowed = Vec::new();
    // 按现有平台使用的分隔符扫描各过滤器片段。
    for token in filters.split(|character: char| {
        // 分号、竖线、NUL 与空白都只承担片段分隔职责。
        character == ';' || character == '|' || character == '\0' || character.is_whitespace()
    }) {
        // 只保留可确定为扩展名或 UTI 的片段。
        let Some(file_type) = normalize_file_type(token) else {
            // 描述文字与全通配符不限制 AppKit 面板。
            continue;
        };
        // 用小写规范值同时执行大小写不敏感去重。
        if seen.insert(file_type.clone()) {
            // 首次出现的类型按调用方声明顺序传给 AppKit。
            allowed.push(file_type);
        }
    }
    // 返回不含点号与星号的 AppKit 文件类型列表。
    allowed
}

// 把单个过滤器片段规范化为 AppKit 接受的扩展名或 UTI。
fn normalize_file_type(token: &str) -> Option<String> {
    // 去除描述语法中常见的括号、引号与逗号。
    let trimmed = token.trim_matches(|character: char| {
        // 这些标点只包围过滤模式，不属于文件类型。
        matches!(
            character,
            '(' | ')' | '[' | ']' | '{' | '}' | ',' | '\'' | '"'
        )
    });
    // 路径模式不能降格为扩展名，否则会意外扩大调用方声明的输入语义。
    if trimmed.contains('/') || trimmed.contains('\\') {
        // 目录限定模式不形成 AppKit allowedFileTypes 条目。
        return None;
    }
    // 优先截取最后一个星号点号后的真实扩展名。
    let candidate = if let Some(index) = trimmed.rfind("*.") {
        // 跳过星号点号前缀，仅保留扩展名或复合扩展名。
        &trimmed[index + 2..]
    // 兼容以点号开头的简写扩展名。
    } else if let Some(extension) = trimmed.strip_prefix('.') {
        // 去除扩展名前导点号。
        extension
    // 兼容仅由小写 ASCII 字母、数字、横线、下划线和点组成的裸类型。
    } else if !trimmed.is_empty()
        // 描述文字通常包含大写字母，不能误当扩展名。
        && trimmed
            .chars()
            .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit() || matches!(character, '-' | '_' | '.'))
    {
        // 裸类型无需删除前缀。
        trimmed
    // 其余片段是描述文字或不受支持的路径模式。
    } else {
        // 返回空值表示该片段不形成 AppKit 限制。
        return None;
    };
    // 清理模式尾部残留的标点。
    let candidate = candidate.trim_matches(|character: char| {
        // 只移除过滤器描述边界，不移除复合扩展名中的点号。
        matches!(
            character,
            '(' | ')' | '[' | ']' | '{' | '}' | ',' | '\'' | '"'
        )
    });
    // 全通配符、空值和路径模式都表示不限制文件类型。
    if candidate.is_empty()
        // 单星号不得传给 allowedFileTypes。
        || candidate == "*"
        // 目录分隔符说明调用方传入的不是扩展名。
        || candidate.contains('/')
        // Windows 目录分隔符同样拒绝。
        || candidate.contains('\\')
    {
        // 不产生 AppKit 文件类型。
        return None;
    }
    // 验证扩展名只包含 AppKit 文件类型可稳定解释的字符。
    if !candidate.chars().all(|character| {
        // 支持扩展名、复合扩展名和常见 UTI 标识字符。
        character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
    }) {
        // 非法模式不收窄用户可选文件范围。
        return None;
    }
    // 统一为小写，保证跨平台过滤结果稳定且便于去重。
    Some(candidate.to_ascii_lowercase())
}
