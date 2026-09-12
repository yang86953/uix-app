//! R7RS-small 读取器的 M1 子集。
//!
//! S 表达式即 AST：读取结果直接是待求值值。嵌套深度、源码大小与字符串
//! 配额在读取期即受引擎限额约束；`quasiquote` 等后续里程碑语法以类型化
//! 错误拒绝，不静默解析成别的形式。

use super::error::SchemeError;
use super::number;
use super::value::Value;
use super::SchemeEngine;

/// 读取全部顶层 datum；空源码返回空向量。
pub(crate) fn read_all(
    engine: &mut SchemeEngine,
    source: &str,
) -> Result<Vec<Value>, SchemeError> {
    let maximum = engine.limits().maximum_source_bytes;
    if source.len() > maximum {
        return Err(SchemeError::SourceTooLarge {
            actual: source.len(),
            maximum,
        });
    }
    let mut reader = Reader {
        characters: source.chars().collect(),
        position: 0,
        line: 1,
        column: 1,
    };
    let mut data = Vec::new();
    loop {
        reader.skip_atmosphere(engine)?;
        if reader.peek().is_none() {
            return Ok(data);
        }
        // 读取期不触发回收：读取器局部结构（列表元素等）不在根集，
        // 分配量由源码字节上限约束，回收交给求值期安全点与关闭清扫。
        data.push(reader.read_datum(engine, 0)?);
    }
}

/// 读取单个 datum；返回 `(值, 消耗字节数)`，仅空白与注释时返回 None。
pub(crate) fn read_one(
    engine: &mut SchemeEngine,
    source: &str,
) -> Result<Option<(Value, usize)>, SchemeError> {
    let mut reader = Reader {
        characters: source.chars().collect(),
        position: 0,
        line: 1,
        column: 1,
    };
    reader.skip_atmosphere(engine)?;
    if reader.peek().is_none() {
        return Ok(None);
    }
    let datum = reader.read_datum(engine, 0)?;
    let consumed_chars = reader.position;
    let consumed_bytes: usize = source
        .chars()
        .take(consumed_chars)
        .map(char::len_utf8)
        .sum();
    Ok(Some((datum, consumed_bytes)))
}

struct Reader {
    characters: Vec<char>,
    position: usize,
    line: u32,
    column: u32,
}

impl Reader {
    fn position(&self) -> (u32, u32) {
        (self.line, self.column)
    }

    fn peek(&self) -> Option<char> {
        self.characters.get(self.position).copied()
    }

    fn peek_second(&self) -> Option<char> {
        self.characters.get(self.position + 1).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let character = self.characters.get(self.position).copied()?;
        self.position += 1;
        if character == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(character)
    }

    fn eof(&self) -> SchemeError {
        let (line, column) = self.position();
        SchemeError::UnexpectedEof { line, column }
    }

    fn unexpected(&self, character: char) -> SchemeError {
        let (line, column) = self.position();
        SchemeError::UnexpectedCharacter {
            line,
            column,
            character,
        }
    }

    fn unsupported(&self, feature: &'static str) -> SchemeError {
        let (line, column) = self.position();
        SchemeError::UnsupportedSyntax { line, column, feature }
    }

    /// 跳过空白、行注释、嵌套块注释与 datum 注释。
    fn skip_atmosphere(&mut self, engine: &mut SchemeEngine) -> Result<(), SchemeError> {
        loop {
            match self.peek() {
                Some(character) if character.is_whitespace() => {
                    self.bump();
                }
                Some(';') => {
                    while let Some(character) = self.peek() {
                        self.bump();
                        if character == '\n' {
                            break;
                        }
                    }
                }
                Some('#') if self.peek_second() == Some('|') => {
                    self.bump();
                    self.bump();
                    self.skip_block_comment()?;
                }
                Some('#') if self.peek_second() == Some(';') => {
                    self.bump();
                    self.bump();
                    engine.tick()?;
                    self.read_datum(engine, 0)?;
                }
                _ => return Ok(()),
            }
        }
    }

    fn skip_block_comment(&mut self) -> Result<(), SchemeError> {
        let mut depth = 1_u32;
        loop {
            match self.peek() {
                None => return Err(self.eof()),
                Some('#') if self.peek_second() == Some('|') => {
                    self.bump();
                    self.bump();
                    depth += 1;
                }
                Some('|') if self.peek_second() == Some('#') => {
                    self.bump();
                    self.bump();
                    depth -= 1;
                    if depth == 0 {
                        return Ok(());
                    }
                }
                Some(_) => {
                    self.bump();
                }
            }
        }
    }

    fn read_datum(
        &mut self,
        engine: &mut SchemeEngine,
        depth: u32,
    ) -> Result<Value, SchemeError> {
        let maximum = engine.limits().maximum_reader_depth;
        if depth > maximum {
            return Err(SchemeError::ReaderDepthExceeded { maximum });
        }
        engine.tick()?;
        self.skip_atmosphere(engine)?;
        let character = self.peek().ok_or_else(|| self.eof())?;
        match character {
            '(' => {
                self.bump();
                self.read_list_tail(engine, depth)
            }
            ')' => Err(self.unexpected(')')),
            '"' => self.read_string(engine),
            '\'' => {
                self.bump();
                let quoted = self.read_datum(engine, depth + 1)?;
                wrap_with_wrapper(engine, quoted, "quote")
            }
            '`' => {
                self.bump();
                let quoted = self.read_datum(engine, depth + 1)?;
                wrap_with_wrapper(engine, quoted, "quasiquote")
            }
            ',' => {
                self.bump();
                let wrapper = if self.peek() == Some('@') {
                    self.bump();
                    "unquote-splicing"
                } else {
                    "unquote"
                };
                let quoted = self.read_datum(engine, depth + 1)?;
                wrap_with_wrapper(engine, quoted, wrapper)
            }
            '#' => self.read_hash(engine, depth),
            '|' => Err(self.unsupported("竖线转义符号")),
            _ => self.read_atom(engine),
        }
    }

    /// 读取到匹配的 `)` 并把元素构造成（可能是非严格的）列表。
    fn read_list_tail(
        &mut self,
        engine: &mut SchemeEngine,
        depth: u32,
    ) -> Result<Value, SchemeError> {
        let mut elements = Vec::new();
        let mut tail = Value::Null;
        loop {
            self.skip_atmosphere(engine)?;
            match self.peek() {
                None => {
                    let (line, column) = self.position();
                    return Err(SchemeError::UnterminatedList { line, column });
                }
                Some(')') => {
                    self.bump();
                    break;
                }
                Some('.') if self.is_dot_token() => {
                    self.bump();
                    tail = self.read_datum(engine, depth + 1)?;
                    self.skip_atmosphere(engine)?;
                    match self.peek() {
                        Some(')') => {
                            self.bump();
                            break;
                        }
                        None => {
                            let (line, column) = self.position();
                            return Err(SchemeError::UnterminatedList { line, column });
                        }
                        Some(_) => {
                            return Err(SchemeError::InvalidSyntax {
                                form: "dotted list",
                                reason: "点号之后只允许一个尾值",
                            });
                        }
                    }
                }
                Some(_) => {
                    elements.push(self.read_datum(engine, depth + 1)?);
                }
            }
        }
        let mut list = tail;
        for element in elements.into_iter().rev() {
            list = engine.new_pair(element, list)?;
        }
        Ok(list)
    }

    fn is_dot_token(&self) -> bool {
        if self.peek() != Some('.') {
            return false;
        }
        match self.peek_second() {
            None => true,
            Some(next) => is_delimiter(next),
        }
    }

    fn read_hash(&mut self, engine: &mut SchemeEngine, depth: u32) -> Result<Value, SchemeError> {
        self.bump();
        match self.peek() {
            None => Err(self.eof()),
            Some('t') => {
                self.read_hash_word("rue")?;
                Ok(Value::Bool(true))
            }
            Some('f') => {
                self.read_hash_word("alse")?;
                Ok(Value::Bool(false))
            }
            Some('(') => {
                self.bump();
                let list = self.read_list_tail(engine, depth)?;
                let elements = engine.value_to_vec("vector literal", &list)?;
                engine.new_vector_from(elements)
            }
            Some('\\') => {
                self.bump();
                self.read_character()
            }
            Some('u') if self.starts_with("u8(") => {
                self.bump();
                self.bump();
                self.bump();
                self.read_bytevector(engine)
            }
            Some('x' | 'b' | 'o' | 'd' | 'e' | 'i') => {
                self.read_prefixed_number(engine)
            }
            Some(character) => Err(self.unexpected(character)),
        }
    }

    /// 读取 `#u8(...)` 到闭合 `)`；元素必须是 0..=255 的精确整数。
    fn read_bytevector(&mut self, engine: &mut SchemeEngine) -> Result<Value, SchemeError> {
        let mut bytes: Vec<u8> = Vec::new();
        loop {
            self.skip_atmosphere(engine)?;
            match self.peek() {
                None => {
                    let (line, column) = self.position();
                    return Err(SchemeError::UnterminatedList { line, column });
                }
                Some(')') => {
                    self.bump();
                    return engine.new_bytevector_from(bytes);
                }
                Some(_) => {
                    let element = self.read_datum(engine, 1)?;
                    let Value::Fixnum(byte) = element else {
                        return Err(self.unsupported("bytevector 非字节元素"));
                    };
                    if !(0..=255).contains(&byte) {
                        return Err(self.unsupported("bytevector 非字节元素"));
                    }
                    bytes.push(byte as u8);
                }
            }
        }
    }

    /// 读取 `#x` / `#b` / `#o` / `#d` / `#e` / `#i` 及其组合数值前缀。
    fn read_prefixed_number(
        &mut self,
        engine: &mut SchemeEngine,
    ) -> Result<Value, SchemeError> {
        let mut radix: Option<u32> = None;
        let mut force_exact = false;
        let mut force_inexact = false;
        let mut saw_prefix = false;
        loop {
            match self.peek() {
                Some('x') if radix.is_none() => {
                    radix = Some(16);
                    saw_prefix = true;
                    self.bump();
                }
                Some('b') if radix.is_none() => {
                    radix = Some(2);
                    saw_prefix = true;
                    self.bump();
                }
                Some('o') if radix.is_none() => {
                    radix = Some(8);
                    saw_prefix = true;
                    self.bump();
                }
                Some('d') if radix.is_none() => {
                    radix = Some(10);
                    saw_prefix = true;
                    self.bump();
                }
                Some('e') if !force_exact && !force_inexact => {
                    force_exact = true;
                    saw_prefix = true;
                    self.bump();
                }
                Some('i') if !force_exact && !force_inexact => {
                    force_inexact = true;
                    saw_prefix = true;
                    self.bump();
                }
                // 组合前缀的第二个 `#`（如 `#e#x10`）。
                Some('#') if saw_prefix => {
                    self.bump();
                }
                _ => break,
            }
        }
        if !saw_prefix {
            return Err(self.unexpected(self.peek().unwrap_or('\0')));
        }
        engine.tick()?;
        let token = self.read_token();
        if token.is_empty() {
            return Err(self.eof());
        }
        let (line, column) = self.position();
        number::parse_literal(&token, radix.unwrap_or(10), force_exact, force_inexact)
            .map(|number| number.to_value())
            .ok_or(SchemeError::UnexpectedCharacter {
                line,
                column,
                character: token.chars().next().unwrap_or('\0'),
            })
    }

    fn starts_with(&self, text: &str) -> bool {
        text.chars().enumerate().all(|(offset, character)| {
            self.characters.get(self.position + offset) == Some(&character)
        })
    }

    /// 消费 `#t` / `#true` / `#f` / `#false` 的剩余部分；当前位于首字符。
    fn read_hash_word(&mut self, long: &str) -> Result<(), SchemeError> {
        self.bump();
        if self.try_consume_word(long) || self.try_consume_word("") {
            return Ok(());
        }
        Err(self.unexpected(self.peek().unwrap_or('\0')))
    }

    /// 尝试消费后缀；要求后缀结束处是定界符或输入结束。
    fn try_consume_word(&mut self, suffix: &str) -> bool {
        let mut probe = self.position;
        for expected in suffix.chars() {
            match self.characters.get(probe) {
                Some(actual) if *actual == expected => probe += 1,
                _ => return false,
            }
        }
        match self.characters.get(probe) {
            None => {}
            Some(next) if is_delimiter(*next) => {}
            _ => return false,
        }
        for _ in 0..(probe - self.position) {
            self.bump();
        }
        true
    }

    fn read_character(&mut self) -> Result<Value, SchemeError> {
        let first = self.bump().ok_or_else(|| self.eof())?;
        if !first.is_ascii_alphabetic() {
            return Ok(Value::Char(first));
        }
        // 字母开头时可能为命名字符或 \x 十六进制转义。
        let mut name = String::new();
        name.push(first);
        while let Some(next) = self.peek() {
            if is_delimiter(next) {
                break;
            }
            name.push(self.bump().unwrap_or(next));
        }
        match name.as_str() {
            "space" => Ok(Value::Char(' ')),
            "newline" => Ok(Value::Char('\n')),
            "tab" => Ok(Value::Char('\t')),
            "x" => {
                let (line, column) = self.position();
                Err(SchemeError::UnexpectedCharacter {
                    line,
                    column,
                    character: 'x',
                })
            }
            _ if name.len() > 1
                && name.starts_with('x')
                && name[1..].chars().all(|digit| digit.is_ascii_hexdigit()) =>
            {
                self.read_hex_character(&name[1..])
            }
            _ if name.chars().count() == 1 => Ok(Value::Char(first)),
            _ => {
                let (line, column) = self.position();
                Err(SchemeError::UnexpectedCharacter {
                    line,
                    column,
                    character: first,
                })
            }
        }
    }

    /// 读取 `#\\xHH...` 形式的十六进制字符；`digits` 不含 `x` 前缀。
    fn read_hex_character(&mut self, digits: &str) -> Result<Value, SchemeError> {
        let mut digits = digits.to_string();
        while let Some(next) = self.peek() {
            if next == ';' {
                self.bump();
                break;
            }
            if is_delimiter(next) {
                break;
            }
            digits.push(self.bump().unwrap_or(next));
        }
        let (line, column) = self.position();
        let code = u32::from_str_radix(&digits, 16).map_err(|_| {
            SchemeError::UnexpectedCharacter {
                line,
                column,
                character: digits.chars().next().unwrap_or('\0'),
            }
        })?;
        char::from_u32(code).map(Value::Char).ok_or(SchemeError::UnexpectedCharacter {
            line,
            column,
            character: '\0',
        })
    }

    fn read_string(&mut self, engine: &mut SchemeEngine) -> Result<Value, SchemeError> {
        self.bump();
        let mut text = String::new();
        let start = self.position();
        loop {
            let character = self
                .bump()
                .ok_or(SchemeError::UnterminatedString { line: start.0, column: start.1 })?;
            match character {
                '"' => break,
                '\\' => {
                    let escape = self.bump().ok_or(SchemeError::UnterminatedString {
                        line: start.0,
                        column: start.1,
                    })?;
                    match escape {
                        'a' => text.push('\u{7}'),
                        'b' => text.push('\u{8}'),
                        't' => text.push('\t'),
                        'n' => text.push('\n'),
                        'r' => text.push('\r'),
                        '"' => text.push('"'),
                        '\\' => text.push('\\'),
                        '\n' => {}
                        'x' => {
                            let mut digits = String::new();
                            loop {
                                let next = self.bump().ok_or(SchemeError::UnterminatedString {
                                        line: start.0,
                                        column: start.1,
                                    })?;
                                if next == ';' {
                                    break;
                                }
                                digits.push(next);
                            }
                            let (line, column) = self.position();
                            let code = u32::from_str_radix(digits.trim(), 16).map_err(|_| {
                                SchemeError::UnexpectedCharacter {
                                    line,
                                    column,
                                    character: 'x',
                                }
                            })?;
                            let decoded = char::from_u32(code).ok_or(SchemeError::UnexpectedCharacter {
                                line,
                                column,
                                character: '\0',
                            })?;
                            text.push(decoded);
                        }
                        other => return Err(self.unexpected(other)),
                    }
                }
                other => text.push(other),
            }
        }
        engine.new_string_from(text)
    }

    fn read_atom(&mut self, engine: &mut SchemeEngine) -> Result<Value, SchemeError> {
        let token = self.read_token();
        if token.is_empty() {
            let character = self.peek().unwrap_or('\0');
            return Err(self.unexpected(character));
        }
        engine.tick()?;
        if let Some(number) = parse_number(&token)? {
            return Ok(number);
        }
        Ok(Value::Symbol(token.as_str().into()))
    }

    /// 读取到定界符为止的裸 token。
    fn read_token(&mut self) -> String {
        let mut token = String::new();
        while let Some(character) = self.peek() {
            if is_delimiter(character) {
                break;
            }
            token.push(self.bump().unwrap_or(character));
        }
        token
    }
}

fn is_delimiter(character: char) -> bool {
    character.is_whitespace()
        || matches!(character, '(' | ')' | '"' | ';' | '\'' | '`' | ',' | '|')
}

/// 解析数值字面量：十进制整数（含大数）、有理数、flonum 与特殊记法；
/// 返回 `None` 表示 token 不是数字。
fn parse_number(token: &str) -> Result<Option<Value>, SchemeError> {
    if token == "." {
        return Err(SchemeError::InvalidSyntax {
            form: "datum",
            reason: "点号只允许出现在列表尾部",
        });
    }
    if !looks_numeric(token) {
        return Ok(None);
    }
    match number::parse_literal(token, 10, false, false) {
        Some(number) => Ok(Some(number.to_value())),
        None => Err(SchemeError::InvalidNumber {
            text: token.to_string(),
        }),
    }
}

/// token 是否应当按数字解析（而非符号）。
fn looks_numeric(token: &str) -> bool {
    let body = token.strip_prefix(['+', '-']).unwrap_or(token);
    if body.is_empty() {
        return false;
    }
    if matches!(token, "+inf.0" | "-inf.0" | "+nan.0" | "-nan.0") {
        return true;
    }
    let mut characters = body.chars();
    match characters.next() {
        Some(first) if first.is_ascii_digit() => true,
        // `.5` 是 flonum 记法；`...`（椭圆）与 `.` 保持符号。
        Some('.') if body.chars().any(|character| character != '.') => true,
        _ => false,
    }
}

/// 把值包装成 `(wrapper value)`；`'x` / `` `x`` / `,x` 语法糖的共享实现。
pub(crate) fn wrap_with_quote(
    engine: &mut SchemeEngine,
    quoted: Value,
) -> Result<Value, SchemeError> {
    wrap_with_wrapper(engine, quoted, "quote")
}

fn wrap_with_wrapper(
    engine: &mut SchemeEngine,
    quoted: Value,
    wrapper: &str,
) -> Result<Value, SchemeError> {
    let symbol = Value::Symbol(wrapper.into());
    let inner = engine.new_pair(quoted, Value::Null)?;
    engine.new_pair(symbol, inner)
}
