// 引入共享诊断与样式 AST。
use super::{Diagnostic, SourceSpan, StyleHash, StyleHashKind, StyleProperty, StyleValue};
// 引入名称去重集合。
use std::collections::HashSet;

// 解析样式块或内联样式中的属性列表。
pub(crate) fn parse_style_properties(
    // 接收不含外围花括号或双引号的源码。
    source: &str,
    // 接收源码首字符的绝对位置。
    origin: SourceSpan,
    // 控制是否允许样式类 extends。
    allow_extends: bool,
) -> Result<(Option<String>, Vec<StyleProperty>), Diagnostic> {
    // 创建局部样式游标。
    let mut cursor = StyleCursor::new(source, origin);
    // 保存可选继承目标。
    let mut extends = None;
    // 保存属性源码顺序。
    let mut properties = Vec::new();
    // 保存已出现属性名。
    let mut names = HashSet::new();
    // 解析到输入末尾。
    loop {
        // 跳过属性间空白与注释。
        cursor.skip_trivia()?;
        // 输入结束完成样式解析。
        if cursor.is_eof() {
            // 退出属性循环。
            break;
        }
        // 保存属性起点。
        let start = cursor.offset;
        // 解析属性名及可选状态后缀。
        let name = cursor.property_name()?;
        // 同一块内属性名必须唯一。
        if !names.insert(name.clone()) {
            // 返回重复属性诊断。
            return Err(Diagnostic::new(
                // 覆盖重复名称。
                cursor.span(start, cursor.offset),
                // 陈述失败原因。
                format!("样式属性 {name} 重复声明"),
                // 给出修复建议。
                "删除重复项或合并为一个属性值",
            ));
        }
        // 跳过冒号后的空白。
        cursor.skip_whitespace();
        // 解析以分号结束的样式值。
        let value = cursor.style_value()?;
        // extends 使用独立继承槽位。
        if name == "extends" {
            // 内联样式和主题不允许继承声明。
            if !allow_extends {
                // 返回继承位置诊断。
                return Err(Diagnostic::new(
                    // 覆盖完整 extends 属性。
                    cursor.span(start, cursor.offset),
                    // 陈述失败原因。
                    "extends 只允许出现在顶层样式类中",
                    // 给出修复建议。
                    "把继承声明移动到具名样式类",
                ));
            }
            // 继承目标必须是单个规范标识符。
            if !is_identifier(&value.source) {
                // 返回非法继承目标诊断。
                return Err(Diagnostic::new(
                    // 指向继承值。
                    value.span,
                    // 陈述失败原因。
                    "extends 必须引用一个样式类名",
                    // 给出合法示例。
                    "使用 extends: baseCard;",
                ));
            }
            // 保存继承目标。
            extends = Some(value.source);
            // 不把 extends 混入普通属性。
            continue;
        }
        // 保存普通样式属性。
        properties.push(StyleProperty {
            // 保存属性名。
            name,
            // 保存解析后的值。
            value,
            // 保存完整属性跨度。
            span: cursor.span(start, cursor.offset),
            // 样式块内的属性默认无条件；@media 块由声明解析器另行标注。
            media: None,
        });
    }
    // 返回继承目标和有序属性。
    Ok((extends, properties))
}

// 提供带绝对位置映射的样式局部游标。
struct StyleCursor<'a> {
    // 保存样式源码。
    source: &'a str,
    // 保存样式首字符绝对位置。
    origin: SourceSpan,
    // 保存当前局部 UTF-8 字节偏移。
    offset: usize,
}

// 实现样式属性与值扫描。
impl<'a> StyleCursor<'a> {
    // 创建样式游标。
    fn new(source: &'a str, origin: SourceSpan) -> Self {
        // 初始化零局部偏移。
        Self {
            // 保存源码。
            source,
            // 保存绝对位置基准。
            origin,
            // 从首字符开始。
            offset: 0,
        }
    }

    // 判断是否到达输入末尾。
    fn is_eof(&self) -> bool {
        // 比较局部偏移与字节长度。
        self.offset >= self.source.len()
    }

    // 查看当前字符。
    fn peek(&self) -> Option<char> {
        // 从字符边界读取 Unicode 字符。
        self.source[self.offset..].chars().next()
    }

    // 消费当前字符。
    fn bump(&mut self) -> Option<char> {
        // 读取当前字符。
        let value = self.peek()?;
        // 按 UTF-8 长度推进。
        self.offset += value.len_utf8();
        // 返回已消费字符。
        Some(value)
    }

    // 判断剩余源码前缀。
    fn starts_with(&self, value: &str) -> bool {
        // 执行字符边界安全的前缀判断。
        self.source[self.offset..].starts_with(value)
    }

    // 消费精确前缀。
    fn consume(&mut self, value: &str) -> bool {
        // 前缀不匹配时保持偏移。
        if !self.starts_with(value) {
            // 报告未消费。
            return false;
        }
        // 按文本字节数推进。
        self.offset += value.len();
        // 报告消费成功。
        true
    }

    // 跳过 Unicode 空白。
    fn skip_whitespace(&mut self) {
        // 连续消费空白字符。
        while self.peek().is_some_and(char::is_whitespace) {
            // 推进一个空白。
            self.bump();
        }
    }

    // 跳过空白及两类规范注释。
    fn skip_trivia(&mut self) -> Result<(), Diagnostic> {
        // 持续消费相邻 trivia。
        loop {
            // 保存本轮起点。
            let before = self.offset;
            // 先跳过空白。
            self.skip_whitespace();
            // 单行注释消费到换行。
            if self.consume("//") {
                // 推进到行尾。
                while self.peek().is_some_and(|value| value != '\n') {
                    // 消费注释字符。
                    self.bump();
                }
            // 块注释消费到结束标记。
            } else if self.consume("/*") {
                // 保存注释起点。
                let start = before;
                // 搜索结束标记。
                while !self.starts_with("*/") {
                    // 输入结束表示注释未闭合。
                    if self.is_eof() {
                        // 返回结构化诊断。
                        return Err(Diagnostic::new(
                            // 覆盖未闭合注释。
                            self.span(start, self.offset),
                            // 陈述失败原因。
                            "样式注释缺少结束标记 */",
                            // 给出修复建议。
                            "在注释末尾添加 */",
                        ));
                    }
                    // 消费注释内容。
                    self.bump();
                }
                // 消费结束标记。
                self.consume("*/");
            }
            // 没有推进时结束。
            if self.offset == before {
                // 退出 trivia 循环。
                break;
            }
        }
        // 报告成功。
        Ok(())
    }

    // 解析属性名和可选交互状态后缀。
    fn property_name(&mut self) -> Result<String, Diagnostic> {
        // 保存名称起点。
        let start = self.offset;
        // 首字符必须是 ASCII 字母或下划线。
        if !self
            // 查看首字符。
            .peek()
            // 验证属性首字符。
            .is_some_and(|value| value.is_ascii_alphabetic() || value == '_')
        {
            // 返回非法名称诊断。
            return Err(Diagnostic::new(
                // 指向当前位置。
                self.span(start, self.offset),
                // 陈述失败原因。
                "样式属性缺少合法名称",
                // 给出命名规则。
                "使用字母开头的属性名，例如 backgroundColor",
            ));
        }
        // 消费首字符。
        self.bump();
        // 消费字母数字、下划线或连字符。
        while self.peek().is_some_and(|value| {
            // 返回后续字符是否合法。
            value.is_ascii_alphanumeric() || matches!(value, '_' | '-')
        }) {
            // 推进一个名称字符。
            self.bump();
        }
        // 保存基础属性名。
        let mut name = self.source[start..self.offset].to_string();
        // 第一个冒号必须存在。
        if !self.consume(":") {
            // 返回缺少分隔符诊断。
            return Err(Diagnostic::new(
                // 覆盖属性名。
                self.span(start, self.offset),
                // 陈述失败原因。
                format!("样式属性 {name} 缺少 :"),
                // 给出修复建议。
                format!("使用 {name}: value;"),
            ));
        }
        // 识别规范状态后缀后的第二个冒号。
        for state in ["hover", "focus", "active"] {
            // 状态文本和分隔符必须连续出现。
            let suffix = format!("{state}:");
            // 匹配时把状态并入属性名。
            if self.consume(&suffix) {
                // 保存状态后缀。
                name.push(':');
                // 保存状态名称。
                name.push_str(state);
                // 状态已经确定。
                break;
            }
        }
        // 返回规范属性名。
        Ok(name)
    }

    // 解析到顶层分号的样式值。
    fn style_value(&mut self) -> Result<StyleValue, Diagnostic> {
        // 保存包含前导空白后的值起点。
        let raw_start = self.offset;
        // 跟踪圆括号深度。
        let mut depth = 0usize;
        // 跟踪单引号字符串状态。
        let mut quoted = false;
        // 跟踪字符串转义状态。
        let mut escaped = false;
        // 扫描到顶层分号。
        loop {
            // 输入末尾表示缺少分号或未闭合结构。
            let Some(next) = self.peek() else {
                // 选择更具体的失败原因。
                let message = if quoted {
                    // 字符串未闭合。
                    "样式值中的字符串缺少结束单引号"
                } else if depth > 0 {
                    // 函数括号未闭合。
                    "样式值中的函数调用缺少 )"
                } else {
                    // 普通值缺少分号。
                    "样式属性缺少结束分号"
                };
                // 返回未闭合诊断。
                return Err(Diagnostic::new(
                    // 覆盖当前值。
                    self.span(raw_start, self.offset),
                    // 陈述失败原因。
                    message,
                    // 给出统一修复动作。
                    "闭合字符串或函数，并在属性末尾添加 ;",
                ));
            };
            // 字符串转义只影响下一字符。
            if escaped {
                // 清除转义状态。
                escaped = false;
                // 消费被转义字符。
                self.bump();
                // 继续扫描。
                continue;
            }
            // 字符串内反斜杠开始转义。
            if quoted && next == '\\' {
                // 标记下一字符转义。
                escaped = true;
                // 消费反斜杠。
                self.bump();
                // 继续扫描。
                continue;
            }
            // 单引号切换字符串状态。
            if next == '\'' {
                // 切换状态。
                quoted = !quoted;
                // 消费单引号。
                self.bump();
                // 继续扫描。
                continue;
            }
            // 字符串内字符没有结构语义。
            if quoted {
                // 消费普通字符串字符。
                self.bump();
                // 继续扫描。
                continue;
            }
            // 左圆括号增加深度。
            if next == '(' {
                // 增加括号深度。
                depth += 1;
                // 消费左圆括号。
                self.bump();
                // 继续扫描。
                continue;
            }
            // 右圆括号必须匹配已有左括号。
            if next == ')' {
                // 零深度右括号非法。
                if depth == 0 {
                    // 返回多余括号诊断。
                    return Err(Diagnostic::new(
                        // 指向右括号。
                        self.span(self.offset, self.offset + 1),
                        // 陈述失败原因。
                        "样式值包含多余的 )",
                        // 给出修复建议。
                        "删除多余右括号或补充匹配的 (",
                    ));
                }
                // 降低括号深度。
                depth -= 1;
                // 消费右圆括号。
                self.bump();
                // 继续扫描。
                continue;
            }
            // 顶层分号结束值。
            if next == ';' && depth == 0 {
                // 保存分号前终点。
                let raw_end = self.offset;
                // 消费分号。
                self.bump();
                // 返回规范化样式值。
                return build_style_value(
                    // 借用原始值片段。
                    &self.source[raw_start..raw_end],
                    // 传入原始值绝对位置。
                    self.span(raw_start, raw_end),
                );
            }
            // 右花括号表明属性遗漏分号。
            if next == '}' && depth == 0 {
                // 返回分号诊断。
                return Err(Diagnostic::new(
                    // 覆盖当前值。
                    self.span(raw_start, self.offset),
                    // 陈述失败原因。
                    "样式属性缺少结束分号",
                    // 给出修复建议。
                    "在属性值末尾添加 ;",
                ));
            }
            // 普通字符继续扫描。
            self.bump();
        }
    }

    // 映射局部范围到文档绝对跨度。
    fn span(&self, start: usize, end: usize) -> SourceSpan {
        // 计算局部起点相对行列。
        absolute_span(self.source, self.origin, start, end)
    }
}

// 去除外围空白并分析井号语义。
fn build_style_value(raw: &str, raw_span: SourceSpan) -> Result<StyleValue, Diagnostic> {
    // 计算前导空白字节数。
    let leading = raw.len() - raw.trim_start().len();
    // 保存去空白源码。
    let source = raw.trim().to_string();
    // 空值没有样式语义。
    if source.is_empty() {
        // 返回空值诊断。
        return Err(Diagnostic::new(
            // 指向原始值位置。
            raw_span,
            // 陈述失败原因。
            "样式属性值不能为空",
            // 给出修复建议。
            "在冒号与分号之间填写样式值",
        ));
    }
    // 构造规范化值起点。
    let origin = advance_origin(raw, raw_span, leading);
    // 分析颜色与主题引用。
    let hashes = analyze_hashes(&source, origin)?;
    // 返回完整样式值。
    Ok(StyleValue {
        // 保存原始值。
        source: source.clone(),
        // 保存哈希语义。
        hashes,
        // 保存规范化跨度。
        span: absolute_span(&source, origin, 0, source.len()),
    })
}

// 分析样式值中未被字符串包裹的井号片段。
fn analyze_hashes(source: &str, origin: SourceSpan) -> Result<Vec<StyleHash>, Diagnostic> {
    // 保存源码顺序中的哈希片段。
    let mut hashes = Vec::new();
    // 保存局部扫描偏移。
    let mut offset = 0usize;
    // 跟踪单引号字符串状态。
    let mut quoted = false;
    // 跟踪字符串转义状态。
    let mut escaped = false;
    // 扫描完整值。
    while offset < source.len() {
        // 读取当前位置字符。
        let next = source[offset..].chars().next().expect("偏移位于字符边界");
        // 字符串转义只影响下一字符。
        if escaped {
            // 清除转义状态。
            escaped = false;
            // 推进被转义字符。
            offset += next.len_utf8();
            // 继续扫描。
            continue;
        }
        // 字符串内反斜杠开始转义。
        if quoted && next == '\\' {
            // 标记下一字符转义。
            escaped = true;
            // 推进反斜杠。
            offset += next.len_utf8();
            // 继续扫描。
            continue;
        }
        // 单引号切换字符串状态。
        if next == '\'' {
            // 切换字符串状态。
            quoted = !quoted;
            // 推进单引号。
            offset += next.len_utf8();
            // 继续扫描。
            continue;
        }
        // 字符串外井号开始语义片段。
        if !quoted && next == '#' {
            // 保存井号起点。
            let start = offset;
            // 跳过井号。
            offset += 1;
            // 保存名称起点。
            let value_start = offset;
            // 消费字母数字、下划线或美元符号。
            while offset < source.len() {
                // 读取候选字符。
                let value = source[offset..].chars().next().expect("偏移位于字符边界");
                // 非名称字符结束片段。
                if !(value.is_ascii_alphanumeric() || matches!(value, '_' | '$')) {
                    // 退出名称扫描。
                    break;
                }
                // 推进候选字符。
                offset += value.len_utf8();
            }
            // 借用井号后的内容。
            let value = &source[value_start..offset];
            // 空井号没有合法语义。
            if value.is_empty() {
                // 返回缺失内容诊断。
                return Err(Diagnostic::new(
                    // 指向井号。
                    absolute_span(source, origin, start, offset),
                    // 陈述失败原因。
                    "样式值中的 # 后缺少颜色或主题属性",
                    // 给出修复建议。
                    "使用 #FF5722 或 #primaryColor",
                ));
            }
            // 全十六进制数字按颜色处理。
            let kind = if value.chars().all(|item| item.is_ascii_hexdigit()) {
                // 颜色只允许标准位数。
                if !matches!(value.len(), 3 | 4 | 6 | 8) {
                    // 返回颜色位数诊断。
                    return Err(Diagnostic::new(
                        // 指向完整颜色。
                        absolute_span(source, origin, start, offset),
                        // 陈述失败原因。
                        "十六进制颜色必须是 3、4、6 或 8 位",
                        // 给出修复示例。
                        "使用 #RGB、#RGBA、#RRGGBB 或 #RRGGBBAA",
                    ));
                }
                // 保存颜色数字。
                StyleHashKind::HexColor(value.to_string())
            // 规范标识符按主题属性引用处理。
            } else if is_identifier(value) {
                // 保存主题属性名。
                StyleHashKind::ThemeReference(value.to_string())
            // 其他内容既不是颜色也不是主题名。
            } else {
                // 返回非法哈希片段诊断。
                return Err(Diagnostic::new(
                    // 指向完整片段。
                    absolute_span(source, origin, start, offset),
                    // 陈述失败原因。
                    "# 后的内容既不是十六进制颜色也不是主题属性名",
                    // 给出修复示例。
                    "使用 #FF5722 或 #primaryColor",
                ));
            };
            // 保存哈希事实。
            hashes.push(StyleHash {
                // 保存分类结果。
                kind,
                // 保存完整跨度。
                span: absolute_span(source, origin, start, offset),
            });
            // 继续扫描后续内容。
            continue;
        }
        // 普通字符推进。
        offset += next.len_utf8();
    }
    // 返回全部哈希片段。
    Ok(hashes)
}

// 验证规范 ASCII 标识符。
fn is_identifier(value: &str) -> bool {
    // 读取首字符。
    let mut chars = value.chars();
    // 首字符必须是字母或下划线。
    let Some(first) = chars.next() else {
        // 空文本不是标识符。
        return false;
    };
    // 首字符非法时拒绝。
    if !(first.is_ascii_alphabetic() || first == '_') {
        // 报告非法。
        return false;
    }
    // 后续允许字母数字、下划线或美元符号。
    chars.all(|item| item.is_ascii_alphanumeric() || matches!(item, '_' | '$'))
}

// 把原始起点推进指定局部字节数。
fn advance_origin(source: &str, origin: SourceSpan, offset: usize) -> SourceSpan {
    // 复用绝对跨度计算取得新起点。
    let point = absolute_span(source, origin, offset, offset);
    // 返回零宽新起点。
    point
}

// 把局部字节范围映射为文档绝对跨度。
fn absolute_span(source: &str, origin: SourceSpan, start: usize, end: usize) -> SourceSpan {
    // 计算起点前缀的相对行列。
    let (line_delta, local_column) = local_line_column(&source[..start]);
    // 首行叠加外部列，后续行使用局部列。
    let column = if line_delta == 0 {
        // 叠加首行列偏移。
        origin.column + local_column - 1
    } else {
        // 换行后从局部首列计算。
        local_column
    };
    // 返回绝对跨度。
    SourceSpan {
        // 叠加绝对字节起点。
        start: origin.start + start,
        // 叠加绝对字节终点。
        end: origin.start + end,
        // 叠加绝对行号。
        line: origin.line + line_delta,
        // 保存计算列号。
        column,
    }
}

// 计算局部前缀末端的一基行列。
fn local_line_column(source: &str) -> (usize, usize) {
    // 初始化零行增量。
    let mut line_delta = 0usize;
    // 初始化一基列。
    let mut column = 1usize;
    // 遍历局部前缀字符。
    for value in source.chars() {
        // 换行增加行并重置列。
        if value == '\n' {
            // 增加行增量。
            line_delta += 1;
            // 重置首列。
            column = 1;
        } else {
            // 普通字符增加列。
            column += 1;
        }
    }
    // 返回相对行列。
    (line_delta, column)
}
