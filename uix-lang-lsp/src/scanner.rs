//! 编辑器侧轻量源码扫描 Component：补全上下文、词边界与声明名扫描。
//!
//! 扫描只服务编辑体验，允许在半成品源码上给出启发式上下文；语言规则与
//! 语义结论仍以共享 compiler 的公开命令为唯一事实源。

// 归纳光标所处的补全上下文。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CompletionContext {
    // 标签名位置：补全内置组件、控制元素与自定义组件。
    TagName,
    // 开放标签内的属性名位置；携带所属标签名。
    AttributeName { component: String },
    // 事件属性名位置（@ 前缀）；携带所属标签名。
    EventName { component: String },
    // 样式属性名位置：内联 style 或顶层样式块。
    StyleProperty,
    // 样式值位置：主题 token、字面量或样式类引用。
    StyleValue,
    // `#` 引用的主题 token 位置。
    ThemeToken,
    // 样式类名值位置。
    ClassName,
    // 顶层 @ 指令位置。
    Directive,
    // 其余位置（文本、表达式、事件动作等）不提供上下文补全。
    Plain,
}

// 判断字符是否属于词内字符；UIX 标签、属性与 token 名都是 ASCII 词。
pub(crate) fn is_word_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

// 提取光标所在完整词的字节区间；词由 ASCII 字母数字下划线组成。
pub(crate) fn word_at(source: &str, offset: usize) -> Option<(usize, usize)> {
    // 先把查询点钳制回字符边界。
    let mut end = offset.min(source.len());
    while end > 0 && !source.is_char_boundary(end) {
        end -= 1;
    }
    // 向右扩展词尾。
    let word_end = source[end..]
        .find(|c: char| !is_word_char(c))
        .map_or(source.len(), |far| end + far);
    // 向左扩展词首。
    let mut word_start = end;
    for character in source[..end].chars().rev() {
        if !is_word_char(character) {
            break;
        }
        word_start -= character.len_utf8();
    }
    // 空词没有可查询对象。
    if word_start == word_end {
        return None;
    }
    Some((word_start, word_end))
}

// 提取光标前正在输入的部分词；允许 @ 前缀用于事件与指令识别。
pub(crate) fn partial_word(source: &str, offset: usize) -> String {
    // 先把查询点钳制回字符边界。
    let mut end = offset.min(source.len());
    while end > 0 && !source.is_char_boundary(end) {
        end -= 1;
    }
    // 向左收集连续词字符；@ 允许出现在任意位置以保留事件与指令前缀。
    let mut word = String::new();
    for character in source[..end].chars().rev() {
        let allow = is_word_char(character) || character == '@';
        if !allow {
            break;
        }
        word.push(character);
    }
    // 收集时逆序，需要反转恢复阅读顺序。
    word.chars().rev().collect()
}

// 找出 source 中 word 的全部全词边界出现位置，返回字节区间。
pub(crate) fn word_occurrences(source: &str, word: &str) -> Vec<(usize, usize)> {
    let mut occurrences = Vec::new();
    // 空词会导致全源码误命中，直接拒绝。
    if word.is_empty() {
        return occurrences;
    }
    let mut start = 0usize;
    while let Some(index) = source[start..].find(word) {
        let begin = start + index;
        let finish = begin + word.len();
        // 前后相邻字符都不是词字符才算独立词出现。
        let before_ok = source[..begin]
            .chars()
            .next_back()
            .is_none_or(|c| !is_word_char(c));
        let after_ok = source[finish..]
            .chars()
            .next()
            .is_none_or(|c| !is_word_char(c));
        if before_ok && after_ok {
            occurrences.push((begin, finish));
        }
        start = finish;
    }
    occurrences
}

// 区分声明扫描识别的顶层声明类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScanSymbolKind {
    // <Widget name="X"> 或 @export('X') 声明的自定义组件。
    Widget,
    // 顶层样式类块。
    StyleClass,
    // @theme 主题块。
    Theme,
    // @keyframes 关键帧块。
    Keyframes,
}

// 保存声明扫描识别出的一个顶层声明名与近似字节区间。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScanSymbol {
    pub kind: ScanSymbolKind,
    pub name: String,
    pub start: usize,
    pub end: usize,
}

// 扫描文档内的顶层声明名；容许语法未完成或存在错误。
pub(crate) fn scan_declaration_spans(source: &str) -> Vec<ScanSymbol> {
    let mut symbols = Vec::new();
    let mut depth = 0usize;
    let mut i = 0usize;
    while i < source.len() {
        let rest = &source[i..];
        // 注释不参与声明识别。
        if rest.starts_with("//") {
            i += rest.find('\n').unwrap_or(rest.len());
            continue;
        }
        if rest.starts_with("/*") {
            i += rest.find("*/").map_or(rest.len(), |far| far + 2);
            continue;
        }
        let Some(character) = rest.chars().next() else {
            break;
        };
        match character {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            '"' | '\'' => {
                i = skip_quoted(source, i);
                continue;
            }
            '<' => {
                // Widget 声明形如 <Widget name="X" ...>；读取标签区间。
                let name = tag_name_at(rest);
                let tag_end = find_tag_end(source, i);
                if name.as_deref() == Some("Widget") {
                    let tag = &source[i..tag_end];
                    if let Some(value) = attribute_string(tag, "name") {
                        symbols.push(ScanSymbol {
                            kind: ScanSymbolKind::Widget,
                            name: value,
                            start: i,
                            end: tag_end,
                        });
                    }
                }
                i = tag_end.max(i + 1);
                continue;
            }
            '@' => {
                // @export 与 @theme/@keyframes 指令在同一步内消费。
                let (symbol, next) = directive_symbol(source, i);
                if let Some(symbol) = symbol {
                    symbols.push(symbol);
                }
                i = next.max(i + 1);
                continue;
            }
            c if is_word_char(c) && depth == 0 => {
                // 深度 0 的标识符后随 `{` 即样式类声明。
                let mut end = i;
                for next in source[i..].chars() {
                    if !is_word_char(next) {
                        break;
                    }
                    end += next.len_utf8();
                }
                let after = source[end..].trim_start();
                if after.starts_with('{') {
                    symbols.push(ScanSymbol {
                        kind: ScanSymbolKind::StyleClass,
                        name: source[i..end].to_string(),
                        start: i,
                        end,
                    });
                }
                i = end.max(i + 1);
                continue;
            }
            _ => {}
        }
        i += character.len_utf8();
    }
    symbols
}

// 跳过一条带转义支持的引号字符串，返回结束引号之后的偏移。
fn skip_quoted(source: &str, start: usize) -> usize {
    // 起点是引号字符本身。
    let Some(expected) = source[start..].chars().next() else {
        return start;
    };
    let mut cursor = start + expected.len_utf8();
    while cursor < source.len() {
        let Some(character) = source[cursor..].chars().next() else {
            break;
        };
        // 反斜杠转义吞掉下一个字符。
        if character == '\\' {
            cursor += character.len_utf8();
            if let Some(next) = source[cursor..].chars().next() {
                cursor += next.len_utf8();
            }
            continue;
        }
        cursor += character.len_utf8();
        // 闭合引号之后结束。
        if character == expected {
            break;
        }
    }
    cursor
}

// 读取 `<` 后的标签名；`</` 闭合标签不产生标签名。
fn tag_name_at(rest: &str) -> Option<String> {
    if rest.starts_with("</") {
        return None;
    }
    let body = rest.strip_prefix('<')?;
    let end = body
        .find(|character: char| !is_word_char(character))
        .unwrap_or(body.len());
    (end > 0).then(|| body[..end].to_string())
}

// 找到从 start 开始标签的结束 `>` 之后偏移；未闭合时返回源码末尾。
fn find_tag_end(source: &str, start: usize) -> usize {
    let mut cursor = start + 1;
    let mut quote: Option<char> = None;
    let mut braces = 0usize;
    while cursor < source.len() {
        let Some(character) = source[cursor..].chars().next() else {
            break;
        };
        if let Some(expected) = quote {
            // 反斜杠转义支持属性值中的引号。
            if character == '\\' {
                cursor += character.len_utf8();
                if let Some(next) = source[cursor..].chars().next() {
                    cursor += next.len_utf8();
                }
                continue;
            }
            if character == expected {
                quote = None;
            }
            cursor += character.len_utf8();
            continue;
        }
        match character {
            '"' | '\'' => quote = Some(character),
            '{' => braces += 1,
            '}' => braces = braces.saturating_sub(1),
            '>' if braces == 0 => return cursor + 1,
            _ => {}
        }
        cursor += character.len_utf8();
    }
    source.len()
}

// 在标签片段中读取 name 属性的字符串字面量值。
fn attribute_string(tag: &str, name: &str) -> Option<String> {
    let mut search = 0usize;
    while let Some(position) = tag[search..].find(name) {
        let after = &tag[search + position + name.len()..];
        // 属性名后必须紧跟 = 与引号才算该属性。
        let trimmed = after.trim_start();
        if let Some(value) = trimmed.strip_prefix('=') {
            let value = value.trim_start();
            let quote = value.chars().next()?;
            if quote == '"' || quote == '\'' {
                let rest = &value[quote.len_utf8()..];
                let end = rest.find(quote).unwrap_or(rest.len());
                return Some(rest[..end].to_string());
            }
        }
        search += position + name.len();
    }
    None
}

// 读取 @ 指令可形成的声明；返回声明与消费后的偏移。
fn directive_symbol(source: &str, start: usize) -> (Option<ScanSymbol>, usize) {
    // 读取完整指令词。
    let mut end = start + 1;
    for character in source[end..].chars() {
        if !character.is_ascii_alphabetic() {
            break;
        }
        end += character.len_utf8();
    }
    match &source[start..end] {
        "@export" => {
            // @export('Name')：取括号内首个字符串字面量。
            let tail = &source[end..];
            let close = tail.find(')').unwrap_or(tail.len());
            let args = &tail[..close];
            let quote = args.chars().find(|c| *c == '\'' || *c == '"');
            if let Some(quote) = quote
                && let Some(position) = args.find(quote)
            {
                let rest = &args[position + quote.len_utf8()..];
                let finish = rest.find(quote).unwrap_or(rest.len());
                return (
                    Some(ScanSymbol {
                        kind: ScanSymbolKind::Widget,
                        name: rest[..finish].to_string(),
                        start,
                        end: end + close,
                    }),
                    end + close,
                );
            }
            (None, end)
        }
        "@theme" | "@keyframes" => {
            // 指令名后的下一个词是块名。
            let rest = source[end..].trim_start();
            let offset = source.len() - rest.len();
            let mut name_end = offset;
            for character in rest.chars() {
                if !is_word_char(character) {
                    break;
                }
                name_end += character.len_utf8();
            }
            let kind = if source[start..end] == *"@theme" {
                ScanSymbolKind::Theme
            } else {
                ScanSymbolKind::Keyframes
            };
            let symbol = (name_end > offset).then(|| ScanSymbol {
                kind,
                name: source[offset..name_end].to_string(),
                start: offset,
                end: name_end,
            });
            (symbol, name_end.max(end))
        }
        _ => (None, end),
    }
}

// 判定光标所处的补全上下文；允许在半成品源码上给出启发式结论。
pub(crate) fn completion_context(source: &str, offset: usize) -> CompletionContext {
    // 先把扫描终点钳制回字符边界。
    let mut end = offset.min(source.len());
    while end > 0 && !source.is_char_boundary(end) {
        end -= 1;
    }
    // 标签内状态。
    let mut in_tag = false;
    let mut tag_name = String::new();
    let mut tag_name_start = 0usize;
    let mut attr_name = String::new();
    let mut quote: Option<char> = None;
    let mut quote_start = 0usize;
    // 标签内表达式花括号深度。
    let mut expr_depth = 0usize;
    // 顶层块与文本插值的深度互不干扰。
    let mut depth = 0usize;
    let mut interp_depth = 0usize;
    let mut block_open = 0usize;
    let mut directive_block: Option<&'static str> = None;
    // 正在累积与上一个完整词。
    let mut pending = String::new();
    let mut last_word = String::new();
    // 上一非空白字符，用于区分文本插值 `{` 与样式块 `{`。
    let mut last_significant = '\0';

    let mut i = 0usize;
    while i < end {
        let rest = &source[i..end];
        // 注释整体跳过。
        if rest.starts_with("//") {
            i += rest.find('\n').unwrap_or(rest.len());
            continue;
        }
        if rest.starts_with("/*") {
            i += rest.find("*/").map_or(rest.len(), |far| far + 2);
            continue;
        }
        let Some(character) = rest.chars().next() else {
            break;
        };
        if in_tag {
            // 引号属性值：整体跳过并记录内容起点。
            if let Some(expected) = quote {
                if character == '\\' {
                    i += character.len_utf8();
                    if let Some(next) = source[i..].chars().next() {
                        i += next.len_utf8();
                    }
                    continue;
                }
                if character == expected {
                    quote = None;
                }
                i += character.len_utf8();
                continue;
            }
            // 表达式值内部：花括号配对与字符串嵌套。
            if expr_depth > 0 {
                match character {
                    '{' => expr_depth += 1,
                    '}' => expr_depth = expr_depth.saturating_sub(1),
                    '"' | '\'' => quote = Some(character),
                    _ => {}
                }
                i += character.len_utf8();
                continue;
            }
            match character {
                '"' | '\'' => {
                    quote = Some(character);
                    quote_start = i + 1;
                }
                '{' => expr_depth += 1,
                '>' => {
                    in_tag = false;
                    tag_name.clear();
                    attr_name.clear();
                    last_significant = '>';
                }
                c if is_word_char(c) => pending.push(c),
                _ => {
                    // 词终止：记录上一个词；等号把最近词固定为属性名。
                    if !pending.is_empty() {
                        last_word = std::mem::take(&mut pending);
                    }
                    if character == '=' {
                        attr_name = last_word.clone();
                    }
                    if !character.is_whitespace() {
                        last_significant = character;
                    }
                }
            }
            i += character.len_utf8();
            continue;
        }
        match character {
            '<' => {
                // 闭合标签直接跳到 >，不属于任何补全上下文。
                if rest.starts_with("</") {
                    i += rest.find('>').map_or(rest.len(), |far| far + 1);
                    continue;
                }
                // 开始标签：读取可能为空的标签名。
                let mut name = String::new();
                let mut cursor = i + 1;
                for next in source[cursor..end].chars() {
                    if !is_word_char(next) {
                        break;
                    }
                    name.push(next);
                    cursor += next.len_utf8();
                }
                tag_name = name;
                tag_name_start = i + 1;
                in_tag = true;
                attr_name.clear();
                expr_depth = 0;
                pending.clear();
                last_word.clear();
                i = cursor;
                continue;
            }
            '"' | '\'' => {
                i = skip_quoted(&source[..end], i).min(end);
                last_significant = character;
                continue;
            }
            '{' => {
                if interp_depth > 0 || last_significant == '>' {
                    // 紧随 `>` 的花括号是文本插值，不是顶层样式块。
                    interp_depth += 1;
                } else {
                    // 先冲刷未结束的词，让 @theme/@keyframes 直接贴 `{` 也成立。
                    if pending.starts_with('@') {
                        directive_block = match pending.as_str() {
                            "@theme" => Some("theme"),
                            "@keyframes" => Some("keyframes"),
                            _ => None,
                        };
                    }
                    depth += 1;
                    if depth == 1 {
                        block_open = i + 1;
                        // 消费指令块标记；其余顶层 `{` 视为样式类块。
                        let _ = directive_block.take();
                    }
                }
                pending.clear();
                last_word.clear();
                last_significant = '{';
            }
            '}' => {
                if interp_depth > 0 {
                    interp_depth = interp_depth.saturating_sub(1);
                } else {
                    depth = depth.saturating_sub(1);
                    directive_block = None;
                }
                pending.clear();
                last_word.clear();
                last_significant = '}';
            }
            c if is_word_char(c) || c == '@' => pending.push(c),
            _ => {
                if !pending.is_empty() {
                    let word = std::mem::take(&mut pending);
                    if word.starts_with('@') {
                        // 识别会开启顶层块的指令。
                        directive_block = match word.as_str() {
                            "@theme" => Some("theme"),
                            "@keyframes" => Some("keyframes"),
                            _ => None,
                        };
                    }
                    last_word = word;
                }
                if !character.is_whitespace() {
                    last_significant = character;
                }
            }
        }
        i += character.len_utf8();
    }

    // 光标前的部分词与紧邻字符直接来自源码，避免状态机残留污染分类。
    let partial = partial_word(source, end);
    let previous = source[..end].chars().last();
    // 词首紧邻 `#` 表示主题 token 引用。
    let word_start = end - partial.chars().map(char::len_utf8).sum::<usize>();
    let hash_before = previous == Some('#')
        || (!partial.is_empty()
            && !partial.starts_with('@')
            && source[..word_start].ends_with('#'));
    if in_tag {
        // 引号属性值按属性名分流。
        if quote.is_some() {
            if attr_name == "style" {
                if hash_before {
                    return CompletionContext::ThemeToken;
                }
                // 按 `;` 与 `:` 的最后出现区分属性名位与值位。
                return segment_context(&source[quote_start..end]);
            }
            if attr_name == "class" {
                return CompletionContext::ClassName;
            }
            return CompletionContext::Plain;
        }
        if expr_depth > 0 {
            return CompletionContext::Plain;
        }
        // 事件属性以 @ 开头。
        if partial.starts_with('@') || previous == Some('@') {
            return CompletionContext::EventName {
                component: tag_name.clone(),
            };
        }
        // 标签名后尚未出现空白与等号时仍处于标签名位。
        let after_name = &source[tag_name_start..end];
        if after_name.chars().all(is_word_char) {
            return CompletionContext::TagName;
        }
        return CompletionContext::AttributeName {
            component: tag_name.clone(),
        };
    }
    // 表达式与文本内部不提供上下文补全。
    if interp_depth > 0 {
        return CompletionContext::Plain;
    }
    if partial.starts_with('@') || previous == Some('@') {
        return CompletionContext::Directive;
    }
    if hash_before {
        return CompletionContext::ThemeToken;
    }
    if depth > 0 {
        // 顶层样式/主题块内部按 `;` 与 `:` 区分属性名位与值位。
        return segment_context(&source[block_open..end]);
    }
    CompletionContext::Plain
}

// 按片段内最后出现的 `;` 与 `:` 判定样式属性名位或值位。
fn segment_context(segment: &str) -> CompletionContext {
    let last_semicolon = segment.rfind(';');
    let last_colon = segment.rfind(':');
    match (last_colon, last_semicolon) {
        // 冒号在最后一个分号之后说明处于值位。
        (Some(colon), Some(semicolon)) if colon > semicolon => CompletionContext::StyleValue,
        (Some(_), None) => CompletionContext::StyleValue,
        // 伪类选择器中的冒号出现在属性之前，仍属属性名位。
        _ => CompletionContext::StyleProperty,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CompletionContext, ScanSymbolKind, completion_context, partial_word,
        scan_declaration_spans, word_at, word_occurrences,
    };

    // 在 fixture 中定位标记所在的字节偏移，用竖线表示光标位置。
    fn context_at(source: &str, marker: usize) -> CompletionContext {
        completion_context(source, marker)
    }

    #[test]
    fn tag_name_context_detected_after_angle_bracket() {
        let source = "<App><Co";
        assert_eq!(context_at(source, source.len()), CompletionContext::TagName);
        // 光标紧随 < 时同样成立。
        let source = "<App><";
        assert_eq!(context_at(source, source.len()), CompletionContext::TagName);
    }

    #[test]
    fn attribute_and_event_contexts_detected_inside_open_tag() {
        let source = "<Column ";
        assert_eq!(
            context_at(source, source.len()),
            CompletionContext::AttributeName {
                component: "Column".into()
            }
        );
        let source = "<Button @cl";
        assert_eq!(
            context_at(source, source.len()),
            CompletionContext::EventName {
                component: "Button".into()
            }
        );
        // 部分属性词不改变上下文。
        let source = "<Column ga";
        assert_eq!(
            context_at(source, source.len()),
            CompletionContext::AttributeName {
                component: "Column".into()
            }
        );
    }

    #[test]
    fn inline_style_property_and_value_contexts_split() {
        // 属性名位：分号或引号开头之后。
        let source = "<Text style=\"";
        assert_eq!(
            context_at(source, source.len()),
            CompletionContext::StyleProperty
        );
        let source = "<Text style=\"color: #FFF; padding";
        assert_eq!(
            context_at(source, source.len()),
            CompletionContext::StyleProperty
        );
        // 值位：冒号之后。
        let source = "<Text style=\"padding: ";
        assert_eq!(
            context_at(source, source.len()),
            CompletionContext::StyleValue
        );
        // # 引用进入主题 token 上下文。
        let source = "<Text style=\"color: #back";
        assert_eq!(
            context_at(source, source.len()),
            CompletionContext::ThemeToken
        );
    }

    #[test]
    fn style_block_contexts_detected_at_top_level() {
        let source = "pagePanel {\n  padding: 16px;\n  border";
        assert_eq!(
            context_at(source, source.len()),
            CompletionContext::StyleProperty
        );
        let source = "pagePanel {\n  borderColor: #colorB";
        assert_eq!(
            context_at(source, source.len()),
            CompletionContext::ThemeToken
        );
        // @theme 块同样按属性名/值位分流。
        let source = "@theme light {\n  fontSize: 1";
        assert_eq!(
            context_at(source, source.len()),
            CompletionContext::StyleValue
        );
    }

    #[test]
    fn directive_and_class_contexts_detected() {
        let source = "@th";
        assert_eq!(
            context_at(source, source.len()),
            CompletionContext::Directive
        );
        let source = "<Button class=\"pseudo";
        assert_eq!(
            context_at(source, source.len()),
            CompletionContext::ClassName
        );
    }

    #[test]
    fn comments_and_interpolations_are_plain() {
        // 注释内不提供上下文。
        let source = "// <Text style=\"";
        assert_eq!(context_at(source, source.len()), CompletionContext::Plain);
        // 文本插值表达式内部按 Plain 处理。
        let source = "<Text>{sta";
        assert_eq!(context_at(source, source.len()), CompletionContext::Plain);
        // 事件动作字符串同样 Plain。
        let source = "<Button @click=\"setSta";
        assert_eq!(context_at(source, source.len()), CompletionContext::Plain);
    }

    #[test]
    fn word_at_reports_byte_ranges_across_multibyte_lines() {
        // 中文与 ASCII 混排时词必须按字节区间定位。
        let source = "中文 stateValue";
        let start = source.find("stateValue").expect("fixture 必须包含词");
        assert_eq!(
            word_at(source, start + 5),
            Some((start, start + "stateValue".len()))
        );
        // 多字节字符内部不属于任何词。
        assert_eq!(word_at(source, 1), None);
    }

    #[test]
    fn partial_word_keeps_at_prefix_only() {
        let source = "<Button @cli";
        assert_eq!(partial_word(source, source.len()), "@cli");
        let source = "<Button disabled";
        assert_eq!(partial_word(source, source.len()), "disabled");
    }

    #[test]
    fn word_occurrences_respect_word_boundaries() {
        let source = "<Text /><Texture /><Text>值</Text>";
        let hits = word_occurrences(source, "Text");
        // Texture 内的 Text 子串不计入；开标签与闭标签各计一次。
        assert_eq!(hits.len(), 3);
        let closed = source.find("</Text>").expect("fixture 必须包含闭标签");
        // 闭标签中的词从 `/` 之后的 T 开始。
        assert!(hits.contains(&(closed + 2, closed + 2 + "Text".len())));
    }

    #[test]
    fn declaration_scan_finds_widgets_classes_themes_and_exports() {
        let source = "@theme light {\n  primaryColor: #FFF;\n}\npagePanel {\n  padding: 16px;\n}\n@export('Helper')\n<Widget name=\"Helper\">\n  <Text>共享</Text>\n</Widget>";
        let symbols = scan_declaration_spans(source);
        let names = |kind| {
            symbols
                .iter()
                .filter(|symbol| symbol.kind == kind)
                .map(|symbol| symbol.name.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(ScanSymbolKind::Theme), vec!["light"]);
        assert_eq!(names(ScanSymbolKind::StyleClass), vec!["pagePanel"]);
        assert_eq!(names(ScanSymbolKind::Widget), vec!["Helper", "Helper"]);
        // 注释中的示例不得被识别。
        let commented = "// <Widget name=\"Fake\" />";
        assert!(scan_declaration_spans(commented).is_empty());
    }
}
