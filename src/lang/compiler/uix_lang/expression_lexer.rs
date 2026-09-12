// 引入共享诊断和跨度类型。
use super::{Diagnostic, SourceSpan};

// 表示受限表达式词法标记及其精确跨度。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExpressionToken {
    // 保存标记种类和值。
    pub(crate) kind: ExpressionTokenKind,
    // 保存标记源码跨度。
    pub(crate) span: SourceSpan,
}

// 枚举表达式语法需要的最小标记集合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExpressionTokenKind {
    // 保存标识符文本。
    Identifier(String),
    // 保存数字源码。
    Number(String),
    // 保存解码后的字符串内容。
    String(String),
    // 表示 true 关键字。
    True,
    // 表示 false 关键字。
    False,
    // 表示加号。
    Plus,
    // 表示减号。
    Minus,
    // 表示乘号。
    Star,
    // 表示除号。
    Slash,
    // 表示取余号。
    Percent,
    // 表示逻辑非。
    Bang,
    // 表示相等比较。
    EqualEqual,
    // 表示不等比较。
    BangEqual,
    // 表示小于比较。
    Less,
    // 表示小于等于比较。
    LessEqual,
    // 表示大于比较。
    Greater,
    // 表示大于等于比较。
    GreaterEqual,
    // 表示逻辑与。
    AndAnd,
    // 表示逻辑或。
    OrOr,
    // 表示左圆括号。
    LeftParen,
    // 表示右圆括号。
    RightParen,
    // 表示左方括号。
    LeftBracket,
    // 表示右方括号。
    RightBracket,
    // 表示左花括号。
    LeftBrace,
    // 表示右花括号。
    RightBrace,
    // 表示点号。
    Dot,
    // 表示逗号。
    Comma,
    // 表示问号。
    Question,
    // 表示冒号。
    Colon,
    // 表示分号。
    Semicolon,
    // 表示单竖线，用于明确诊断闭包。
    Pipe,
    // 表示输入结束。
    End,
}

// 把表达式源码转换为带绝对位置的标记流。
pub(crate) fn lex_expression(
    // 接收已经去除外围定界符的表达式源码。
    source: &str,
    // 接收表达式首字符在文档中的绝对位置。
    origin: SourceSpan,
) -> Result<Vec<ExpressionToken>, Diagnostic> {
    // 创建局部表达式词法器。
    let mut lexer = ExpressionLexer::new(source, origin);
    // 保存源码顺序中的标记。
    let mut tokens = Vec::new();
    // 读取到显式结束标记。
    loop {
        // 读取下一个标记。
        let token = lexer.next_token()?;
        // 记录是否到达输入末尾。
        let is_end = matches!(token.kind, ExpressionTokenKind::End);
        // 保存当前标记。
        tokens.push(token);
        // 结束标记完成词法阶段。
        if is_end {
            // 退出读取循环。
            break;
        }
    }
    // 返回完整确定性标记流。
    Ok(tokens)
}

// 提供 UTF-8 安全的表达式局部词法游标。
struct ExpressionLexer<'a> {
    // 保存表达式源码。
    source: &'a str,
    // 保存表达式在文档中的起始位置。
    origin: SourceSpan,
    // 保存当前局部 UTF-8 字节偏移。
    offset: usize,
}

// 实现表达式标记扫描。
impl<'a> ExpressionLexer<'a> {
    // 创建位于表达式首字符的词法器。
    fn new(source: &'a str, origin: SourceSpan) -> Self {
        // 初始化零局部偏移。
        Self {
            // 保存源码借用。
            source,
            // 保存绝对位置基准。
            origin,
            // 从局部起点开始。
            offset: 0,
        }
    }

    // 读取下一个非空白标记。
    fn next_token(&mut self) -> Result<ExpressionToken, Diagnostic> {
        // 跳过表达式内部 Unicode 空白。
        while self.peek().is_some_and(char::is_whitespace) {
            // 消费一个空白字符。
            self.bump();
        }
        // 保存标记起点。
        let start = self.offset;
        // 输入结束时返回显式哨兵。
        let Some(next) = self.peek() else {
            // 返回零宽结束标记。
            return Ok(self.token(ExpressionTokenKind::End, start));
        };
        // 标识符和关键字使用统一扫描。
        if next.is_ascii_alphabetic() || matches!(next, '_' | '$') {
            // 返回标识符类标记。
            return Ok(self.identifier(start));
        }
        // 数字字面量以十进制数字开头。
        if next.is_ascii_digit() {
            // 返回数字标记。
            return Ok(self.number(start));
        }
        // 单引号开始规范字符串字面量。
        if next == '\'' {
            // 返回字符串标记或未闭合诊断。
            return self.string(start);
        }
        // 优先消费双字符运算符。
        for (text, kind) in [
            // 匹配相等运算符。
            ("==", ExpressionTokenKind::EqualEqual),
            // 匹配不等运算符。
            ("!=", ExpressionTokenKind::BangEqual),
            // 匹配小于等于运算符。
            ("<=", ExpressionTokenKind::LessEqual),
            // 匹配大于等于运算符。
            (">=", ExpressionTokenKind::GreaterEqual),
            // 匹配逻辑与运算符。
            ("&&", ExpressionTokenKind::AndAnd),
            // 匹配逻辑或运算符。
            ("||", ExpressionTokenKind::OrOr),
        ] {
            // 当前前缀匹配时消费运算符。
            if self.consume(text) {
                // 返回匹配到的双字符标记。
                return Ok(self.token(kind, start));
            }
        }
        // 消费单字符标记。
        self.bump();
        // 映射单字符到标记种类。
        let kind = match next {
            // 映射加号。
            '+' => ExpressionTokenKind::Plus,
            // 映射减号。
            '-' => ExpressionTokenKind::Minus,
            // 映射乘号。
            '*' => ExpressionTokenKind::Star,
            // 映射除号。
            '/' => ExpressionTokenKind::Slash,
            // 映射取余号。
            '%' => ExpressionTokenKind::Percent,
            // 映射逻辑非。
            '!' => ExpressionTokenKind::Bang,
            // 映射小于号。
            '<' => ExpressionTokenKind::Less,
            // 映射大于号。
            '>' => ExpressionTokenKind::Greater,
            // 映射左圆括号。
            '(' => ExpressionTokenKind::LeftParen,
            // 映射右圆括号。
            ')' => ExpressionTokenKind::RightParen,
            // 映射左方括号。
            '[' => ExpressionTokenKind::LeftBracket,
            // 映射右方括号。
            ']' => ExpressionTokenKind::RightBracket,
            // 映射左花括号。
            '{' => ExpressionTokenKind::LeftBrace,
            // 映射右花括号。
            '}' => ExpressionTokenKind::RightBrace,
            // 映射成员点号。
            '.' => ExpressionTokenKind::Dot,
            // 映射参数逗号。
            ',' => ExpressionTokenKind::Comma,
            // 映射三元问号。
            '?' => ExpressionTokenKind::Question,
            // 映射三元或命名参数冒号。
            ':' => ExpressionTokenKind::Colon,
            // 映射语句分号以生成专用诊断。
            ';' => ExpressionTokenKind::Semicolon,
            // 映射闭包竖线以生成专用诊断。
            '|' => ExpressionTokenKind::Pipe,
            // 单等号属于赋值语句而非表达式子集。
            '=' => {
                // 返回赋值禁用诊断。
                return Err(self.error(
                    start,
                    "表达式不支持赋值",
                    "使用比较 ==，或把赋值逻辑移到 Rust 侧",
                ));
            }
            // 其余字符不属于规范子集。
            _ => {
                // 返回非法字符诊断。
                return Err(self.error(
                    start,
                    format!("表达式包含不支持的字符 {next}"),
                    "只使用文档列出的受限表达式结构",
                ));
            }
        };
        // 返回单字符标记。
        Ok(self.token(kind, start))
    }

    // 扫描标识符或布尔关键字。
    fn identifier(&mut self, start: usize) -> ExpressionToken {
        // 消费首字符。
        self.bump();
        // 消费后续标识符字符。
        while self.peek().is_some_and(|value| {
            // 允许字母数字、下划线和非首位美元符号。
            value.is_ascii_alphanumeric() || value == '_' || value == '$'
        }) {
            // 推进一个字符。
            self.bump();
        }
        // 借用完整标识符文本。
        let value = &self.source[start..self.offset];
        // 映射保留布尔字面量。
        let kind = match value {
            // 映射 true。
            "true" => ExpressionTokenKind::True,
            // 映射 false。
            "false" => ExpressionTokenKind::False,
            // 其余文本保持标识符。
            _ => ExpressionTokenKind::Identifier(value.to_string()),
        };
        // 返回带跨度标记。
        self.token(kind, start)
    }

    // 扫描十进制整数或小数。
    fn number(&mut self, start: usize) -> ExpressionToken {
        // 消费整数部分。
        while self.peek().is_some_and(|value| value.is_ascii_digit()) {
            // 推进一位数字。
            self.bump();
        }
        // 只有点号后仍为数字时才消费小数部分。
        if self.starts_with(".")
            // 检查点号后一字符。
            && self.source[self.offset + 1..]
                // 读取点号后一字符。
                .chars()
                // 要求至少一位小数。
                .next()
                // 验证十进制数字。
                .is_some_and(|value| value.is_ascii_digit())
        {
            // 消费小数点。
            self.bump();
            // 消费小数位。
            while self.peek().is_some_and(|value| value.is_ascii_digit()) {
                // 推进一位数字。
                self.bump();
            }
        }
        // 保存未改写的数字源码。
        let value = self.source[start..self.offset].to_string();
        // 返回数字标记。
        self.token(ExpressionTokenKind::Number(value), start)
    }

    // 扫描并解码单引号字符串。
    fn string(&mut self, start: usize) -> Result<ExpressionToken, Diagnostic> {
        // 消费起始单引号。
        self.bump();
        // 保存解码结果。
        let mut value = String::new();
        // 扫描到结束单引号。
        loop {
            // 读取下一个字符或报告未闭合。
            let Some(next) = self.bump() else {
                // 返回未闭合字符串诊断。
                return Err(self.error(
                    start,
                    "表达式字符串缺少结束单引号",
                    "在字符串末尾添加单引号",
                ));
            };
            // 单引号结束字符串。
            if next == '\'' {
                // 返回字符串标记。
                return Ok(self.token(ExpressionTokenKind::String(value), start));
            }
            // 反斜杠转义下一字符。
            if next == '\\' {
                // 读取转义目标或报告悬空转义。
                let Some(escaped) = self.bump() else {
                    // 返回悬空转义诊断。
                    return Err(self.error(
                        start,
                        "表达式字符串含未完成转义",
                        "补充被转义字符或删除反斜杠",
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

    // 返回当前位置字符。
    fn peek(&self) -> Option<char> {
        // 从局部偏移读取 Unicode 字符。
        self.source[self.offset..].chars().next()
    }

    // 消费当前位置字符。
    fn bump(&mut self) -> Option<char> {
        // 读取字符。
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
        // 不匹配时保持偏移。
        if !self.starts_with(value) {
            // 报告未消费。
            return false;
        }
        // 按文本字节数推进。
        self.offset += value.len();
        // 报告消费成功。
        true
    }

    // 构造从局部起点到当前偏移的标记。
    fn token(&self, kind: ExpressionTokenKind, start: usize) -> ExpressionToken {
        // 返回带绝对跨度的标记。
        ExpressionToken {
            // 保存标记种类。
            kind,
            // 映射局部跨度到文档。
            span: self.span(start, self.offset),
        }
    }

    // 构造当前局部范围的诊断。
    fn error(
        &self,
        start: usize,
        message: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Diagnostic {
        // 返回结构化诊断。
        Diagnostic::new(self.span(start, self.offset), message, suggestion)
    }

    // 把局部字节范围映射为绝对跨度。
    fn span(&self, start: usize, end: usize) -> SourceSpan {
        // 计算局部起点相对的行列。
        let (line_delta, local_column) = local_line_column(&self.source[..start]);
        // 起点仍在首行时叠加外部列。
        let column = if line_delta == 0 {
            // 把局部一基列叠加到外部首列。
            self.origin.column + local_column - 1
        // 后续行从文档首列计算。
        } else {
            // 使用换行后的局部列。
            local_column
        };
        // 返回绝对跨度。
        SourceSpan {
            // 加上表达式在文档中的字节起点。
            start: self.origin.start + start,
            // 加上表达式在文档中的字节终点。
            end: self.origin.start + end,
            // 加上表达式首行号。
            line: self.origin.line + line_delta,
            // 保存计算后的字符列。
            column,
        }
    }
}

// 计算文本末端相对起点的一基行列。
fn local_line_column(source: &str) -> (usize, usize) {
    // 初始化零行增量。
    let mut line_delta = 0;
    // 初始化一基列。
    let mut column = 1;
    // 遍历局部前缀字符。
    for value in source.chars() {
        // 换行重置列并增加行增量。
        if value == '\n' {
            // 增加一行。
            line_delta += 1;
            // 重置到首列。
            column = 1;
        // 其他字符增加字符列。
        } else {
            // 增加一列。
            column += 1;
        }
    }
    // 返回相对行列。
    (line_delta, column)
}
