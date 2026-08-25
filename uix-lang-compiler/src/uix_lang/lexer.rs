// 引入共享诊断和跨度类型。
use std::cell::Cell;

use super::{Diagnostic, SourceSpan};

// 保存已经定位到的最远源码位置，供后续跨度按增量更新行列。
#[derive(Clone, Copy)]
struct LineColumnCache {
    offset: usize,
    line: usize,
    column: usize,
    line_start: usize,
}

// 提供按 UTF-8 字符边界推进的源码词法游标。
pub(crate) struct Cursor<'a> {
    // 保存完整输入源码。
    source: &'a str,
    // 保存当前 UTF-8 字节偏移。
    offset: usize,
    // 保存跨度定位的最远扫描位置，避免每个 token 都从文件开头重扫。
    line_column_cache: Cell<LineColumnCache>,
}

// 实现解析器需要的最小词法操作。
impl<'a> Cursor<'a> {
    // 从文档起点创建词法游标。
    pub(crate) fn new(source: &'a str) -> Self {
        // 初始化零偏移游标。
        Self {
            source,
            offset: 0,
            line_column_cache: Cell::new(LineColumnCache {
                offset: 0,
                line: 1,
                column: 1,
                line_start: 0,
            }),
        }
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

    // 解析指令参数使用的单引号字符串。
    pub(crate) fn single_quoted_literal(&mut self) -> Result<String, Diagnostic> {
        // 保存字符串起点。
        let start = self.offset;
        // 要求起始单引号。
        if !self.consume("'") {
            // 返回缺少指令参数诊断。
            return Err(Diagnostic::new(
                // 指向当前位置。
                self.point_span(),
                // 陈述失败原因。
                "指令参数必须使用单引号字符串",
                // 给出合法示例。
                "使用 @import('./widget.uix')",
            ));
        }
        // 保存解码内容。
        let mut value = String::new();
        // 扫描到结束单引号。
        loop {
            // 读取下一字符或报告未闭合。
            let Some(next) = self.bump() else {
                // 返回未闭合字符串诊断。
                return Err(Diagnostic::new(
                    // 覆盖完整字符串。
                    self.span_from(start),
                    // 陈述失败原因。
                    "指令字符串缺少结束单引号",
                    // 给出修复建议。
                    "在参数末尾添加单引号",
                ));
            };
            // 单引号结束字符串。
            if next == '\'' {
                // 返回解码结果。
                return Ok(value);
            }
            // 反斜杠转义下一字符。
            if next == '\\' {
                // 读取转义目标。
                let Some(escaped) = self.bump() else {
                    // 返回悬空转义诊断。
                    return Err(Diagnostic::new(
                        // 覆盖完整字符串。
                        self.span_from(start),
                        // 陈述失败原因。
                        "指令字符串含未完成转义",
                        // 给出修复建议。
                        "补充被转义字符或删除反斜杠",
                    ));
                };
                // 保存被转义字符。
                value.push(escaped);
            } else {
                // 保存普通字符。
                value.push(next);
            }
        }
    }

    // 读取双引号包裹的内联样式源码。
    pub(crate) fn quoted_style_source(&mut self) -> Result<(String, SourceSpan), Diagnostic> {
        // 保存外层引号起点。
        let start = self.offset;
        // 要求起始双引号。
        if !self.consume("\"") {
            // 返回内联样式格式诊断。
            return Err(Diagnostic::new(
                // 指向当前位置。
                self.point_span(),
                // 陈述失败原因。
                "内联 style 必须使用双引号包裹",
                // 给出合法示例。
                "使用 style=\"color: red;\"",
            ));
        }
        // 保存内容起点。
        let content_start = self.offset;
        // 跟踪外层转义状态。
        let mut escaped = false;
        // 扫描到未转义双引号。
        loop {
            // 读取下一字符或报告未闭合。
            let Some(next) = self.bump() else {
                // 返回未闭合样式诊断。
                return Err(Diagnostic::new(
                    // 覆盖整个属性值。
                    self.span_from(start),
                    // 陈述失败原因。
                    "内联 style 缺少结束双引号",
                    // 给出修复建议。
                    "在内联样式末尾添加双引号",
                ));
            };
            // 已转义字符没有结构语义。
            if escaped {
                // 清除转义状态。
                escaped = false;
                // 继续扫描。
                continue;
            }
            // 反斜杠转义下一字符。
            if next == '\\' {
                // 标记转义状态。
                escaped = true;
                // 继续扫描。
                continue;
            }
            // 未转义双引号结束内容。
            if next == '"' {
                // 计算内容终点。
                let content_end = self.offset - 1;
                // 返回原始内容与精确跨度。
                return Ok((
                    // 复制样式源码。
                    self.source[content_start..content_end].to_string(),
                    // 保存内容跨度。
                    self.span_between(content_start, content_end),
                ));
            }
        }
    }

    // 读取顶层样式或主题花括号块源码。
    pub(crate) fn braced_style_source(
        &mut self,
    ) -> Result<(String, SourceSpan, SourceSpan), Diagnostic> {
        // 保存外围花括号起点。
        let start = self.offset;
        // 要求左花括号。
        if !self.consume("{") {
            // 返回缺少样式块诊断。
            return Err(Diagnostic::new(
                // 指向当前位置。
                self.point_span(),
                // 陈述失败原因。
                "样式声明缺少 {",
                // 给出修复建议。
                "在声明名后添加 { ... }",
            ));
        }
        // 保存内容起点。
        let content_start = self.offset;
        // 跟踪单引号字符串状态。
        let mut quoted = false;
        // 跟踪字符串转义状态。
        let mut escaped = false;
        // 扫描到块结束花括号。
        loop {
            // 输入结束表示样式块未闭合。
            if self.is_eof() {
                // 未闭合单引号优先报告样式值字符串错误。
                if quoted {
                    // 返回字符串专用诊断。
                    return Err(Diagnostic::new(
                        // 覆盖整个声明块。
                        self.span_from(start),
                        // 陈述失败原因。
                        "样式值中的字符串缺少结束单引号",
                        // 给出修复建议。
                        "在字符串末尾添加单引号",
                    ));
                }
                // 返回未闭合块诊断。
                return Err(Diagnostic::new(
                    // 覆盖整个声明块。
                    self.span_from(start),
                    // 陈述失败原因。
                    "样式声明缺少结束花括号",
                    // 给出修复建议。
                    "在样式属性之后添加 }",
                ));
            }
            // 字符串外单行注释需要忽略内部花括号。
            if !quoted && self.consume("//") {
                // 推进到换行。
                while self.peek().is_some_and(|value| value != '\n') {
                    // 消费注释字符。
                    self.bump();
                }
                // 继续扫描。
                continue;
            }
            // 字符串外块注释需要忽略内部花括号。
            if !quoted && self.consume("/*") {
                // 保存注释起点。
                let comment_start = self.offset - 2;
                // 搜索结束标记。
                while !self.starts_with("*/") {
                    // 输入结束表示注释未闭合。
                    if self.is_eof() {
                        // 返回注释诊断。
                        return Err(Diagnostic::new(
                            // 覆盖未闭合注释。
                            self.span_from(comment_start),
                            // 陈述失败原因。
                            "样式注释缺少结束标记 */",
                            // 给出修复建议。
                            "在注释末尾添加 */",
                        ));
                    }
                    // 消费注释字符。
                    self.bump();
                }
                // 消费结束标记。
                self.consume("*/");
                // 继续扫描。
                continue;
            }
            // 读取下一字符。
            let next = self.bump().expect("已确认样式块未结束");
            // 已转义字符没有结构语义。
            if escaped {
                // 清除转义状态。
                escaped = false;
                // 继续扫描。
                continue;
            }
            // 字符串内反斜杠开始转义。
            if quoted && next == '\\' {
                // 标记下一字符被转义。
                escaped = true;
                // 继续扫描。
                continue;
            }
            // 单引号切换字符串状态。
            if next == '\'' {
                // 切换字符串状态。
                quoted = !quoted;
                // 继续扫描。
                continue;
            }
            // 字符串外左花括号不属于样式值语法。
            if !quoted && next == '{' {
                // 返回非法嵌套诊断。
                return Err(Diagnostic::new(
                    // 指向嵌套花括号。
                    self.span_between(self.offset - 1, self.offset),
                    // 陈述失败原因。
                    "样式块不支持嵌套花括号",
                    // 给出修复建议。
                    "把每个样式类或主题声明放在顶层",
                ));
            }
            // 字符串外右花括号结束样式块。
            if !quoted && next == '}' {
                // 计算内容终点。
                let content_end = self.offset - 1;
                // 返回源码、外围跨度与内容跨度。
                return Ok((
                    // 复制块内容。
                    self.source[content_start..content_end].to_string(),
                    // 保存包含花括号的跨度。
                    self.span_from(start),
                    // 保存块内容跨度。
                    self.span_between(content_start, content_end),
                ));
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
        // 跟踪对象字面量带来的嵌套花括号深度。
        let mut nested_depth = 0usize;
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
            // 非字符串内的左花括号进入嵌套对象。
            if !quoted && next == '{' {
                // 增加对象嵌套深度。
                nested_depth += 1;
                // 继续扫描对象内容。
                continue;
            }
            // 嵌套对象的右花括号只退出一层。
            if !quoted && next == '}' && nested_depth > 0 {
                // 减少对象嵌套深度。
                nested_depth -= 1;
                // 继续寻找外层属性闭合花括号。
                continue;
            }
            // 非字符串且不在嵌套对象内的花括号结束表达式。
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
        let cached = self.line_column_cache.get();
        // 正向跨度只扫描上次定位之后的增量；解析器通常按源码顺序请求位置。
        let (scan_start, mut line, mut column, mut line_start) = if offset >= cached.offset {
            (cached.offset, cached.line, cached.column, cached.line_start)
        } else if offset >= cached.line_start {
            // 同一行内的父节点回查可直接从行首计算，不回扫此前所有行。
            (cached.line_start, cached.line, 1, cached.line_start)
        } else {
            // 父节点结束时可能回查更早的起点，保留最远缓存并从文档起点定位。
            (0, 1, 1, 0)
        };
        // 遍历尚未计入位置缓存的 Unicode 字符。
        for (relative_offset, value) in self.source[scan_start..offset].char_indices() {
            // 换行推进到下一行首列。
            if value == '\n' {
                // 增加行号。
                line += 1;
                // 重置列号。
                column = 1;
                // 保存新行在完整源码中的字节起点。
                line_start = scan_start + relative_offset + value.len_utf8();
            // 非换行字符推进一列。
            } else {
                // 增加字符列号。
                column += 1;
            }
        }
        if offset >= cached.offset {
            self.line_column_cache.set(LineColumnCache {
                offset,
                line,
                column,
                line_start,
            });
        }
        // 返回计算结果。
        (line, column)
    }
}

#[cfg(test)]
mod tests {
    use super::Cursor;

    #[test]
    fn incremental_line_column_cache_preserves_unicode_and_backward_spans() {
        let source = "甲乙\nA好\n末";
        let cursor = Cursor::new(source);

        let second_character = cursor.span_between("甲".len(), "甲乙".len());
        assert_eq!((second_character.line, second_character.column), (1, 2));

        let last_line = source.find('末').expect("夹具必须包含末字");
        let last_character = cursor.span_between(last_line, source.len());
        assert_eq!((last_character.line, last_character.column), (3, 1));

        let backward = source.find('好').expect("夹具必须包含好字");
        let backward_span = cursor.span_between(backward, backward + '好'.len_utf8());
        assert_eq!((backward_span.line, backward_span.column), (2, 2));

        let forward_again = cursor.span_between(last_line, source.len());
        assert_eq!((forward_again.line, forward_again.column), (3, 1));
    }
}
