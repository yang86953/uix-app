// 引入共享诊断和跨度类型。
use super::{Diagnostic, SourceSpan};

// 提供按 UTF-8 字符边界推进的源码词法游标。
pub(crate) struct Cursor<'a> {
    // 保存完整输入源码。
    source: &'a str,
    // 保存当前 UTF-8 字节偏移。
    offset: usize,
}

// 实现解析器需要的最小词法操作。
impl<'a> Cursor<'a> {
    // 从文档起点创建词法游标。
    pub(crate) fn new(source: &'a str) -> Self {
        // 初始化零偏移游标。
        Self { source, offset: 0 }
    }

    // 返回完整输入源码。
    pub(crate) fn source(&self) -> &'a str {
        // 借用保存的源码。
        self.source
    }

    // 返回当前字节偏移。
    pub(crate) fn offset(&self) -> usize {
        // 返回内部偏移。
        self.offset
    }

    // 判断游标是否到达输入末尾。
    pub(crate) fn is_eof(&self) -> bool {
        // 比较当前偏移与源码长度。
        self.offset >= self.source.len()
    }

    // 判断剩余源码是否以给定文本开头。
    pub(crate) fn starts_with(&self, value: &str) -> bool {
        // 在当前字符边界执行前缀判断。
        self.source[self.offset..].starts_with(value)
    }

    // 查看但不消费下一个 Unicode 标量值。
    pub(crate) fn peek(&self) -> Option<char> {
        // 从剩余源码读取首字符。
        self.source[self.offset..].chars().next()
    }

    // 消费并返回下一个 Unicode 标量值。
    pub(crate) fn bump(&mut self) -> Option<char> {
        // 读取当前字符。
        let value = self.peek()?;
        // 按 UTF-8 编码长度推进偏移。
        self.offset += value.len_utf8();
        // 返回已消费字符。
        Some(value)
    }

    // 在当前位置消费精确文本。
    pub(crate) fn consume(&mut self, value: &str) -> bool {
        // 前缀不匹配时保持游标不变。
        if !self.starts_with(value) {
            // 报告未消费。
            return false;
        }
        // 按 ASCII 或 UTF-8 文本字节长度推进。
        self.offset += value.len();
        // 报告消费成功。
        true
    }

    // 构造从给定起点到当前偏移的源码跨度。
    pub(crate) fn span_from(&self, start: usize) -> SourceSpan {
        // 复用任意半开区间跨度构造。
        self.span_between(start, self.offset)
    }

    // 构造输入内任意半开字节区间的源码跨度。
    pub(crate) fn span_between(&self, start: usize, end: usize) -> SourceSpan {
        // 计算起点的一基行列。
        let (line, column) = self.line_column(start);
        // 返回半开区间跨度。
        SourceSpan {
            // 保存起始偏移。
            start,
            // 保存当前结束偏移。
            end,
            // 保存一基行号。
            line,
            // 保存一基字符列号。
            column,
        }
    }

    // 构造当前位置的零宽跨度。
    pub(crate) fn point_span(&self) -> SourceSpan {
        // 复用统一跨度构造。
        self.span_from(self.offset)
    }

    // 跳过空白与两类规范注释。
    pub(crate) fn skip_trivia(&mut self) -> Result<(), Diagnostic> {
        // 持续消费相邻 trivia。
        loop {
            // 记录本轮起点以判断是否推进。
            let before = self.offset;
            // 消费所有 Unicode 空白。
            while self.peek().is_some_and(char::is_whitespace) {
                // 推进一个空白字符。
                self.bump();
            }
            // 消费单行注释。
            if self.consume("//") {
                // 推进到换行或输入末尾。
                while self.peek().is_some_and(|value| value != '\n') {
                    // 消费注释字符。
                    self.bump();
                }
            // 消费不可嵌套的块注释。
            } else if self.consume("/*") {
                // 保存注释起点用于错误定位。
                let start = before;
                // 搜索第一个结束标记。
                while !self.starts_with("*/") {
                    // 输入结束表示注释未闭合。
                    if self.is_eof() {
                        // 返回带修复建议的诊断。
                        return Err(Diagnostic::new(
                            // 覆盖未闭合注释。
                            self.span_from(start),
                            // 陈述失败原因。
                            "多行注释缺少结束标记 */",
                            // 给出确定修复动作。
                            "在注释末尾添加 */",
                        ));
                    }
                    // 消费注释内容字符。
                    self.bump();
                }
                // 消费结束标记。
                self.consume("*/");
            }
            // 没有推进表示 trivia 已耗尽。
            if self.offset == before {
                // 结束循环。
                break;
            }
        }
        // 报告 trivia 消费成功。
        Ok(())
    }

    // 解析规范标识符。
    pub(crate) fn identifier(&mut self) -> Option<(String, SourceSpan)> {
        // 保存标识符起点。
        let start = self.offset;
        // 首字符必须是 ASCII 字母或下划线。
        let first = self.peek()?;
        // 拒绝非法首字符。
        if !(first.is_ascii_alphabetic() || first == '_') {
            // 报告没有标识符。
            return None;
        }
        // 消费合法首字符。
        self.bump();
        // 消费后续字母、数字、下划线或美元符号。
        while self.peek().is_some_and(|value| {
            // 返回后续字符是否合法。
            value.is_ascii_alphanumeric() || value == '_' || value == '$'
        }) {
            // 推进一个标识符字符。
            self.bump();
        }
        // 复制标识符源码。
        let value = self.source[start..self.offset].to_string();
        // 返回值与跨度。
        Some((value, self.span_from(start)))
    }

    // 解析双引号包裹的属性字面量。
    pub(crate) fn quoted_literal(&mut self) -> Result<String, Diagnostic> {
        // 保存引号起点。
        let start = self.offset;
        // 要求起始双引号。
        if !self.consume("\"") {
            // 返回缺少属性值的诊断。
            return Err(Diagnostic::new(
                // 指向当前输入位置。
                self.point_span(),
                // 陈述失败原因。
                "属性值必须使用双引号或花括号",
                // 给出两种合法写法。
                "使用 name=\"value\" 或 name={expression}",
            ));
        }
        // 保存去引号后的值。
        let mut value = String::new();
        // 扫描到闭合双引号。
        loop {
            // 读取下一个字符或报告未闭合。
            let Some(next) = self.bump() else {
                // 返回覆盖整个字面量的诊断。
                return Err(Diagnostic::new(
                    // 覆盖未闭合内容。
                    self.span_from(start),
                    // 陈述失败原因。
                    "属性字符串缺少结束双引号",
                    // 给出确定修复动作。
                    "在属性值末尾添加双引号",
                ));
            };
            // 双引号结束字面量。
            if next == '"' {
                // 返回已解析内容。
                return Ok(value);
            }
            // 反斜杠保留下一字符的字面含义。
            if next == '\\' {
                // 读取被转义字符。
                let Some(escaped) = self.bump() else {
                    // 返回末尾悬空转义诊断。
                    return Err(Diagnostic::new(
                        // 覆盖未闭合内容。
                        self.span_from(start),
                        // 陈述失败原因。
                        "属性字符串以未完成的转义结尾",
                        // 给出修复建议。
                        "删除末尾反斜杠或补充被转义字符",
                    ));
                };
                // 保存被转义字符。
                value.push(escaped);
            // 普通字符原样保存。
            } else {
                // 追加普通字符。
                value.push(next);
            }
        }
    }

    // 读取双引号包裹但不解码的事件表达式源码。
    pub(crate) fn quoted_expression_source(&mut self) -> Result<(String, SourceSpan), Diagnostic> {
        // 保存外层引号起点。
        let start = self.offset;
        // 要求起始双引号。
        if !self.consume("\"") {
            // 返回缺少事件表达式诊断。
            return Err(Diagnostic::new(
                // 指向当前位置。
                self.point_span(),
                // 陈述失败原因。
                "事件处理器必须使用双引号包裹",
                // 给出合法示例。
                "使用 @click=\"onConfirm()\"",
            ));
        }
        // 保存内容起点。
        let content_start = self.offset;
        // 跟踪反斜杠是否转义下一字符。
        let mut escaped = false;
        // 扫描到未转义的结束双引号。
        loop {
            // 读取下一字符或报告未闭合。
            let Some(next) = self.bump() else {
                // 返回未闭合处理器诊断。
                return Err(Diagnostic::new(
                    // 覆盖整个事件属性值。
                    self.span_from(start),
                    // 陈述失败原因。
                    "事件处理器缺少结束双引号",
                    // 给出确定修复动作。
                    "在事件处理器末尾添加双引号",
                ));
            };
            // 已转义字符不会结束外层字符串。
            if escaped {
                // 清除转义状态。
                escaped = false;
                // 继续扫描。
                continue;
            }
            // 反斜杠转义下一字符。
            if next == '\\' {
                // 标记下一字符被转义。
                escaped = true;
                // 继续扫描。
                continue;
            }
            // 未转义双引号结束内容。
            if next == '"' {
                // 计算不含结束引号的内容终点。
                let content_end = self.offset - next.len_utf8();
                // 复制原始表达式源码。
                let source = self.source[content_start..content_end].to_string();
                // 返回源码与内容跨度。
                return Ok((source, self.span_between(content_start, content_end)));
            }
        }
    }

    // 解析并返回去除外层花括号的表达式源码。
    pub(crate) fn braced_expression(
        &mut self,
    ) -> Result<(String, SourceSpan, SourceSpan), Diagnostic> {
        // 保存表达式起点。
        let start = self.offset;
        // 消费起始花括号。
        self.consume("{");
        // 保存表达式内容起点。
        let content_start = self.offset;
        // 跟踪单引号字符串状态。
        let mut quoted = false;
        // 跟踪字符串内转义状态。
        let mut escaped = false;
        // 扫描到未被字符串包裹的结束花括号。
        loop {
            // 读取下一字符或报告未闭合。
            let Some(next) = self.bump() else {
                // 返回完整未闭合跨度。
                return Err(Diagnostic::new(
                    // 覆盖表达式起点到文件末尾。
                    self.span_from(start),
                    // 陈述失败原因。
                    "表达式缺少结束花括号",
                    // 给出确定修复动作。
                    "在表达式末尾添加 }",
                ));
            };
            // 字符串内的转义只影响下一字符。
            if escaped {
                // 清除转义状态。
                escaped = false;
                // 继续扫描。
                continue;
            }
            // 字符串内反斜杠开启转义。
            if quoted && next == '\\' {
                // 标记下一字符被转义。
                escaped = true;
                // 继续扫描。
                continue;
            }
            // 单引号切换字符串状态。
            if next == '\'' {
                // 翻转字符串状态。
                quoted = !quoted;
                // 继续扫描。
                continue;
            }
            // 非字符串内的花括号结束表达式。
            if !quoted && next == '}' {
                // 结束偏移排除闭合花括号。
                let content_end = self.offset - next.len_utf8();
                // 借用未去空白的表达式内容。
                let raw = &self.source[content_start..content_end];
                // 计算首个非空白字节相对位置。
                let leading = raw.len() - raw.trim_start().len();
                // 去除外围空白但保留内部源码。
                let source = raw.trim().to_string();
                // 空表达式没有可映射语义。
                if source.is_empty() {
                    // 返回明确空表达式诊断。
                    return Err(Diagnostic::new(
                        // 覆盖完整花括号。
                        self.span_from(start),
                        // 陈述失败原因。
                        "表达式不能为空",
                        // 给出修复建议。
                        "在花括号内填写受限表达式",
                    ));
                }
                // 计算去空白后内容的绝对起点。
                let trimmed_start = content_start + leading;
                // 计算去空白后内容的绝对终点。
                let trimmed_end = trimmed_start + source.len();
                // 返回表达式源码、外围跨度和内容跨度。
                return Ok((
                    // 返回规范化源码。
                    source,
                    // 返回包含花括号的完整跨度。
                    self.span_from(start),
                    // 返回用于表达式标记定位的内容跨度。
                    self.span_between(trimmed_start, trimmed_end),
                ));
            }
        }
    }

    // 计算给定 UTF-8 字节偏移的一基行列。
    fn line_column(&self, offset: usize) -> (usize, usize) {
        // 初始化一基行号。
        let mut line = 1;
        // 初始化一基字符列号。
        let mut column = 1;
        // 遍历起点之前的 Unicode 字符。
        for value in self.source[..offset].chars() {
            // 换行推进到下一行首列。
            if value == '\n' {
                // 增加行号。
                line += 1;
                // 重置列号。
                column = 1;
            // 非换行字符推进一列。
            } else {
                // 增加字符列号。
                column += 1;
            }
        }
        // 返回计算结果。
        (line, column)
    }
}
