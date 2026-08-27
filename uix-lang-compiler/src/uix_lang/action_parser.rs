// 引入 action 独立 AST、受限表达式解析与结构化诊断。
use super::{
    ActionBlock, ActionBody, ActionStatement, Diagnostic, SourceSpan, WidgetAction,
    parse_expression,
};
// 复用 Widget 字段的 Rust 兼容名称校验。
use super::widget_parser::validate_field_name;
// 引入声明名称去重集合。
use std::collections::HashSet;

// 解析源码顺序中的兼容单表达式和 do 块同步 action。
pub(super) fn parse_actions(
    source: &str,
    span: SourceSpan,
) -> Result<Vec<WidgetAction>, Diagnostic> {
    // 空字符串表示组件没有业务 action。
    if source.trim().is_empty() {
        return Ok(Vec::new());
    }
    // 保存源码顺序与名称唯一性。
    let mut actions = Vec::new();
    let mut names = HashSet::new();
    // 花括号、数组与调用内部逗号不切分声明。
    for (entry_start, entry_end) in split_action_entries(source, span)? {
        let entry = &source[entry_start..entry_end];
        let Some(colon) = entry.find(':') else {
            return Err(Diagnostic::new(
                subspan(span, source, entry_start, entry_end),
                format!("action 字段 {entry:?} 缺少冒号"),
                "使用 name: expression 或 name: do { ... }",
            ));
        };
        let name = entry[..colon].trim();
        let value_start = entry_start + colon + 1;
        let (value_start, value_end) = trim_range(source, value_start, entry_end);
        if name.is_empty() || value_start == value_end {
            return Err(Diagnostic::new(
                subspan(span, source, entry_start, entry_end),
                "action 声明的名称或主体为空",
                "在冒号两侧填写 action 名称与表达式或 do 块",
            ));
        }
        validate_field_name(
            name,
            subspan(span, source, entry_start, entry_start + colon),
        )?;
        // 框架内置操作不能被组件 action 遮蔽。
        if matches!(name, "setState" | "setStyle" | "setTheme" | "submitForm") {
            return Err(Diagnostic::new(
                subspan(span, source, entry_start, entry_start + colon),
                format!("action {name} 使用了保留的语言内置名称"),
                "使用 save、submit 或 increment 等业务名称",
            ));
        }
        if !names.insert(name.to_string()) {
            return Err(Diagnostic::new(
                subspan(span, source, entry_start, entry_start + colon),
                format!("action {name} 重复声明"),
                "合并同名 action 或使用不同名称",
            ));
        }
        let body_source = &source[value_start..value_end];
        let body_span = subspan(span, source, value_start, value_end);
        // do 关键字只在完整单词后接花括号时引入语句面。
        let body = if starts_keyword(body_source, 0, "do") {
            let mut parser = ActionBlockParser::new(body_source, body_span);
            parser.consume_keyword("do")?;
            parser.skip_whitespace();
            let block = parser.parse_block()?;
            parser.skip_whitespace();
            if !parser.is_end() {
                return Err(parser.error_here(
                    "do action 块后存在多余内容",
                    "删除闭合花括号后的文本，多个 action 使用顶层逗号分隔",
                ));
            }
            ActionBody::Block(block)
        } else {
            ActionBody::Expression(parse_expression(body_source, body_span)?)
        };
        actions.push(WidgetAction {
            name: name.to_string(),
            body,
            span: subspan(span, source, entry_start, entry_end),
        });
    }
    Ok(actions)
}

// 提供只负责 action 语句文法的 UTF-8 安全游标。
struct ActionBlockParser<'a> {
    source: &'a str,
    origin: SourceSpan,
    offset: usize,
}

impl<'a> ActionBlockParser<'a> {
    fn new(source: &'a str, origin: SourceSpan) -> Self {
        Self {
            source,
            origin,
            offset: 0,
        }
    }

    // 解析一个建立词法作用域的花括号块。
    fn parse_block(&mut self) -> Result<ActionBlock, Diagnostic> {
        self.skip_whitespace();
        let start = self.offset;
        self.expect_char(
            '{',
            "action do/if 后缺少左花括号",
            "使用 do { ... } 或 if condition { ... }",
        )?;
        let mut statements = Vec::new();
        loop {
            self.skip_whitespace();
            if self.peek() == Some('}') {
                self.bump();
                return Ok(ActionBlock {
                    statements,
                    span: self.span(start, self.offset),
                });
            }
            if self.is_end() {
                return Err(Diagnostic::new(
                    self.span(start, self.offset),
                    "action 块缺少右花括号",
                    "在语句列表末尾添加 }",
                ));
            }
            statements.push(self.parse_statement()?);
        }
    }

    // 按关键字优先级解析当前语句。
    fn parse_statement(&mut self) -> Result<ActionStatement, Diagnostic> {
        self.skip_whitespace();
        if starts_keyword(self.source, self.offset, "let") {
            return self.parse_let();
        }
        if starts_keyword(self.source, self.offset, "if") {
            return self.parse_if();
        }
        if starts_keyword(self.source, self.offset, "return") {
            return self.parse_return();
        }
        self.parse_assignment_or_expression()
    }

    fn parse_let(&mut self) -> Result<ActionStatement, Diagnostic> {
        let start = self.offset;
        self.consume_keyword("let")?;
        self.skip_whitespace();
        let name_start = self.offset;
        let name = self.parse_identifier()?;
        validate_local_name(&name, self.span(name_start, self.offset))?;
        self.skip_whitespace();
        self.expect_char('=', "let 声明缺少 =", "使用 let name = expression;")?;
        let expression_start = self.offset;
        let expression_end = self.read_until_statement_end()?;
        let (expression_start, expression_end) =
            trim_range(self.source, expression_start, expression_end);
        let initializer = parse_expression(
            &self.source[expression_start..expression_end],
            self.span(expression_start, expression_end),
        )?;
        Ok(ActionStatement::Let {
            name,
            initializer,
            mutable: false,
            span: self.span(start, self.offset),
        })
    }

    fn parse_if(&mut self) -> Result<ActionStatement, Diagnostic> {
        let start = self.offset;
        self.consume_keyword("if")?;
        let condition_start = self.offset;
        let block_start = self.read_until_top_level_block()?;
        let (condition_start, condition_end) =
            trim_range(self.source, condition_start, block_start);
        if condition_start == condition_end {
            return Err(Diagnostic::new(
                self.span(start, block_start),
                "if 缺少条件表达式",
                "使用 if condition { ... }",
            ));
        }
        let condition = parse_expression(
            &self.source[condition_start..condition_end],
            self.span(condition_start, condition_end),
        )?;
        let then_block = self.parse_block()?;
        self.skip_whitespace();
        let else_block = if starts_keyword(self.source, self.offset, "else") {
            self.consume_keyword("else")?;
            self.skip_whitespace();
            Some(self.parse_block()?)
        } else {
            None
        };
        Ok(ActionStatement::If {
            condition,
            then_block,
            else_block,
            span: self.span(start, self.offset),
        })
    }

    fn parse_return(&mut self) -> Result<ActionStatement, Diagnostic> {
        let start = self.offset;
        self.consume_keyword("return")?;
        let value_start = self.offset;
        let value_end = self.read_until_statement_end()?;
        let (value_start, value_end) = trim_range(self.source, value_start, value_end);
        let value = if value_start == value_end {
            None
        } else {
            Some(parse_expression(
                &self.source[value_start..value_end],
                self.span(value_start, value_end),
            )?)
        };
        Ok(ActionStatement::Return {
            value,
            span: self.span(start, self.offset),
        })
    }

    fn parse_assignment_or_expression(&mut self) -> Result<ActionStatement, Diagnostic> {
        let start = self.offset;
        let end = self.read_until_statement_end()?;
        let (source_start, source_end) = trim_range(self.source, start, end);
        if source_start == source_end {
            return Err(Diagnostic::new(
                self.span(start, self.offset),
                "action 块包含空语句",
                "删除多余分号或填写表达式",
            ));
        }
        let statement_source = &self.source[source_start..source_end];
        if let Some(equal) = top_level_assignment(statement_source) {
            let name = statement_source[..equal].trim();
            validate_local_name(name, self.span(source_start, source_start + equal))?;
            let value_offset = source_start + equal + 1;
            let (value_start, value_end) = trim_range(self.source, value_offset, source_end);
            if value_start == value_end {
                return Err(Diagnostic::new(
                    self.span(source_start, source_end),
                    "局部赋值缺少右值表达式",
                    "使用 name = expression;",
                ));
            }
            let value = parse_expression(
                &self.source[value_start..value_end],
                self.span(value_start, value_end),
            )?;
            return Ok(ActionStatement::Assign {
                name: name.to_string(),
                value,
                span: self.span(start, self.offset),
            });
        }
        let expression = parse_expression(statement_source, self.span(source_start, source_end))?;
        Ok(ActionStatement::Expression {
            expression,
            span: self.span(start, self.offset),
        })
    }

    // 读取到不位于表达式定界符内的分号并消费它。
    fn read_until_statement_end(&mut self) -> Result<usize, Diagnostic> {
        let start = self.offset;
        let mut scanner = DelimiterScanner::default();
        while let Some(character) = self.peek() {
            let index = self.offset;
            self.bump();
            if scanner.advance(character)? && character == ';' {
                return Ok(index);
            }
            if scanner.at_top_level() && character == '}' {
                self.offset = index;
                break;
            }
        }
        Err(Diagnostic::new(
            self.span(start, self.offset),
            "action 语句缺少分号",
            "在 let、赋值、表达式或 return 语句末尾添加 ;",
        ))
    }

    // 读取 if 条件后的首个顶层左花括号但不消费。
    fn read_until_top_level_block(&mut self) -> Result<usize, Diagnostic> {
        let start = self.offset;
        let mut scanner = DelimiterScanner::default();
        while let Some(character) = self.peek() {
            let index = self.offset;
            if scanner.at_top_level() && character == '{' {
                return Ok(index);
            }
            self.bump();
            scanner.advance(character)?;
        }
        Err(Diagnostic::new(
            self.span(start, self.offset),
            "if 条件后缺少语句块",
            "使用 if condition { ... }",
        ))
    }

    fn parse_identifier(&mut self) -> Result<String, Diagnostic> {
        let start = self.offset;
        let Some(first) = self.peek() else {
            return Err(self.error_here("缺少局部名称", "填写合法小写 Rust 标识符"));
        };
        if !first.is_ascii_alphabetic() && first != '_' {
            return Err(self.error_here("局部名称不是标识符", "填写合法小写 Rust 标识符"));
        }
        self.bump();
        while self
            .peek()
            .is_some_and(|value| value.is_ascii_alphanumeric() || value == '_')
        {
            self.bump();
        }
        Ok(self.source[start..self.offset].to_string())
    }

    fn consume_keyword(&mut self, keyword: &str) -> Result<(), Diagnostic> {
        if !starts_keyword(self.source, self.offset, keyword) {
            return Err(self.error_here(
                format!("缺少 action 关键字 {keyword}"),
                "使用规范登记的 action 语句",
            ));
        }
        self.offset += keyword.len();
        Ok(())
    }

    fn expect_char(
        &mut self,
        expected: char,
        message: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Result<(), Diagnostic> {
        if self.peek() != Some(expected) {
            return Err(self.error_here(message, suggestion));
        }
        self.bump();
        Ok(())
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.bump();
        }
    }

    fn peek(&self) -> Option<char> {
        self.source[self.offset..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.offset += character.len_utf8();
        Some(character)
    }

    fn is_end(&self) -> bool {
        self.offset >= self.source.len()
    }

    fn span(&self, start: usize, end: usize) -> SourceSpan {
        subspan(self.origin, self.source, start, end)
    }

    fn error_here(&self, message: impl Into<String>, suggestion: impl Into<String>) -> Diagnostic {
        Diagnostic::new(self.span(self.offset, self.offset), message, suggestion)
    }
}

// 保存表达式扫描期间的定界符与字符串状态。
#[derive(Default)]
struct DelimiterScanner {
    paren: i32,
    bracket: i32,
    brace: i32,
    quoted: bool,
    escaped: bool,
}

impl DelimiterScanner {
    // 返回字符处理后是否仍位于顶层；顶层分号由调用方消费。
    fn advance(&mut self, character: char) -> Result<bool, Diagnostic> {
        if self.quoted {
            if self.escaped {
                self.escaped = false;
            } else if character == '\\' {
                self.escaped = true;
            } else if character == '\'' {
                self.quoted = false;
            }
            return Ok(false);
        }
        match character {
            '\'' => self.quoted = true,
            '(' => self.paren += 1,
            ')' => self.paren -= 1,
            '[' => self.bracket += 1,
            ']' => self.bracket -= 1,
            '{' => self.brace += 1,
            '}' => self.brace -= 1,
            _ => {}
        }
        // 这里只报告事实，具体诊断由表达式解析器或块解析器补充。
        Ok(self.at_top_level())
    }

    fn at_top_level(&self) -> bool {
        !self.quoted && self.paren == 0 && self.bracket == 0 && self.brace == 0
    }
}

// 按所有表达式定界符之外的逗号切分 action 声明范围。
fn split_action_entries(source: &str, span: SourceSpan) -> Result<Vec<(usize, usize)>, Diagnostic> {
    let mut entries = Vec::new();
    let mut start = 0;
    let mut scanner = DelimiterScanner::default();
    for (index, character) in source.char_indices() {
        let top_before = scanner.at_top_level();
        if character == ',' && top_before {
            let range = trim_range(source, start, index);
            if range.0 == range.1 {
                return Err(Diagnostic::new(
                    subspan(span, source, start, index),
                    "actions 包含空声明",
                    "删除连续逗号或补充 action 声明",
                ));
            }
            entries.push(range);
            start = index + 1;
            continue;
        }
        scanner.advance(character)?;
        if scanner.paren < 0 || scanner.bracket < 0 || scanner.brace < 0 {
            return Err(Diagnostic::new(
                subspan(span, source, index, index + character.len_utf8()),
                "actions 定界符不匹配",
                "配对圆括号、方括号与花括号",
            ));
        }
    }
    if scanner.quoted || scanner.paren != 0 || scanner.bracket != 0 || scanner.brace != 0 {
        return Err(Diagnostic::new(
            span,
            "actions 包含未闭合字符串或定界符",
            "闭合单引号、圆括号、方括号与花括号",
        ));
    }
    let range = trim_range(source, start, source.len());
    // 尾随逗号是成员块的多行书写惯用法；空尾段直接忽略。
    if range.0 != range.1 {
        entries.push(range);
    }
    Ok(entries)
}

// 查找单个语句中的顶层赋值等号，比较运算符不命中。
fn top_level_assignment(source: &str) -> Option<usize> {
    let mut scanner = DelimiterScanner::default();
    let bytes = source.as_bytes();
    for (index, character) in source.char_indices() {
        if scanner.at_top_level()
            && character == '='
            && index
                .checked_sub(1)
                .is_none_or(|previous| bytes[previous] != b'=')
            && bytes.get(index + 1).is_none_or(|next| *next != b'=')
        {
            return Some(index);
        }
        let _ = scanner.advance(character);
    }
    None
}

fn validate_local_name(name: &str, span: SourceSpan) -> Result<(), Diagnostic> {
    if name
        .chars()
        .next()
        .is_some_and(|value| value.is_ascii_lowercase() || value == '_')
        && syn::parse_str::<syn::Ident>(name).is_ok()
    {
        return Ok(());
    }
    Err(Diagnostic::new(
        span,
        format!("action 局部名称 {name:?} 不是合法 Rust 标识符"),
        "使用小写或下划线开头、且不是 Rust 关键字的名称",
    ))
}

fn starts_keyword(source: &str, offset: usize, keyword: &str) -> bool {
    source[offset..].starts_with(keyword)
        && source[offset + keyword.len()..]
            .chars()
            .next()
            .is_none_or(|next| !next.is_ascii_alphanumeric() && next != '_')
}

fn trim_range(source: &str, mut start: usize, mut end: usize) -> (usize, usize) {
    while start < end {
        let character = source[start..end].chars().next().expect("范围非空");
        if !character.is_whitespace() {
            break;
        }
        start += character.len_utf8();
    }
    while start < end {
        let character = source[start..end].chars().next_back().expect("范围非空");
        if !character.is_whitespace() {
            break;
        }
        end -= character.len_utf8();
    }
    (start, end)
}

// 从属性跨度与局部源码偏移构造精确到 action 子结构的跨度。
fn subspan(origin: SourceSpan, source: &str, start: usize, end: usize) -> SourceSpan {
    let prefix = &source[..start];
    let line_breaks = prefix
        .chars()
        .filter(|character| *character == '\n')
        .count();
    let column = if let Some(last_line) = prefix.rsplit('\n').next().filter(|_| line_breaks > 0) {
        last_line.chars().count() + 1
    } else {
        origin.column + prefix.chars().count()
    };
    SourceSpan {
        start: origin.start + start,
        end: origin.start + end,
        line: origin.line + line_breaks,
        column,
    }
}
