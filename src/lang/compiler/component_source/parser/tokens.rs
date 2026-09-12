use super::*;

fn name_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}
fn name_continue(ch: char) -> bool {
    name_start(ch) || ch.is_numeric()
}

impl Parser<'_> {
    pub(super) fn start(&mut self) -> Result<usize> {
        self.trivia()?;
        Ok(self.pos)
    }

    pub(super) fn eof(&mut self) -> Result<bool> {
        self.trivia()?;
        Ok(self.pos == self.source.len())
    }

    pub(super) fn trivia(&mut self) -> Result<()> {
        loop {
            let start = self.pos;
            while let Some(ch) = self.source[self.pos..].chars().next() {
                if !ch.is_whitespace() {
                    break;
                }
                self.pos += ch.len_utf8();
            }
            if self.pos > start {
                self.token(start, Kind::Whitespace)?;
            }
            if self.source[self.pos..].starts_with("//") {
                let start = self.pos;
                self.pos += self.source[self.pos..]
                    .find('\n')
                    .unwrap_or(self.source.len() - self.pos);
                self.token(start, Kind::LineComment)?;
            } else if self.source[self.pos..].starts_with("/*") {
                let start = self.pos;
                let Some(end) = self.source[self.pos + 2..].find("*/") else {
                    return Err(self.error_at(start, "component-comment", "块注释缺少 */"));
                };
                self.pos += end + 4;
                self.token(start, Kind::BlockComment)?;
            } else {
                return Ok(());
            }
        }
    }

    pub(super) fn at(&mut self, token: &str) -> Result<bool> {
        self.trivia()?;
        let rest = &self.source[self.pos..];
        if !rest.starts_with(token) {
            return Ok(false);
        }
        let next = rest[token.len()..].chars().next();
        if token.chars().last().is_some_and(name_continue) && next.is_some_and(name_continue) {
            return Ok(false);
        }
        // 单字符运算符不能吃掉复合运算符的前缀。
        Ok(!matches!(
            (token, next),
            ("=", Some('=' | '>')) | ("!" | "<" | ">", Some('='))
        ))
    }

    pub(super) fn eat(&mut self, token: &str) -> Result<bool> {
        if self.at(token)? {
            let start = self.pos;
            self.pos += token.len();
            self.token(start, Kind::Code)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub(super) fn expect(&mut self, token: &str) -> Result<()> {
        if self.eat(token)? {
            Ok(())
        } else {
            Err(self.error("component-token", format!("此处需要 {token}")))
        }
    }

    pub(super) fn name(&mut self) -> Result<Name> {
        self.node()?;
        let start = self.start()?;
        if !self.source[self.pos..]
            .chars()
            .next()
            .is_some_and(name_start)
        {
            return Err(self.error("component-name", "此处需要标识符"));
        }
        self.pos += self.source[self.pos..]
            .chars()
            .take_while(|ch| name_continue(*ch))
            .map(char::len_utf8)
            .sum::<usize>();
        let text = &self.source[start..self.pos];
        if matches!(
            text,
            "import"
                | "from"
                | "as"
                | "export"
                | "component"
                | "function"
                | "type"
                | "state"
                | "let"
                | "if"
                | "else"
                | "return"
                | "true"
                | "false"
        ) {
            return Err(self.error_at(
                start,
                "component-reserved-name",
                format!("{text} 是保留字，不能用作标识符"),
            ));
        }
        self.token(start, Kind::Code)?;
        Ok(Name {
            text: text.into(),
            span: self.span(start),
        })
    }

    pub(super) fn path(&mut self) -> Result<Vec<Name>> {
        let mut path = vec![self.name()?];
        while self.eat(".")? {
            path.push(self.name()?);
        }
        Ok(path)
    }

    pub(super) fn string(&mut self) -> Result<(String, Span)> {
        let start = self.start()?;
        let quote = self.source[self.pos..].chars().next();
        let Some(quote @ ('\'' | '"')) = quote else {
            return Err(self.error("component-string", "此处需要引号字符串"));
        };
        self.pos += 1;
        let mut value = String::new();
        while let Some(ch) = self.source[self.pos..].chars().next() {
            self.pos += ch.len_utf8();
            if ch == quote {
                self.token(start, Kind::Literal)?;
                return Ok((value, self.span(start)));
            }
            if ch.is_control() {
                return Err(self.error_at(
                    self.pos - ch.len_utf8(),
                    "component-string",
                    "字符串控制字符需要转义",
                ));
            }
            if ch != '\\' {
                value.push(ch);
                continue;
            }
            let escape_start = self.pos - 1;
            let Some(escaped) = self.source[self.pos..].chars().next() else {
                break;
            };
            self.pos += escaped.len_utf8();
            value.push(match escaped {
                '\\' => '\\',
                '\'' => '\'',
                '"' => '"',
                '/' => '/',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                'b' => '\u{0008}',
                'f' => '\u{000c}',
                'u' => {
                    let high = self.hex4()?;
                    let code = if (0xd800..=0xdbff).contains(&high) {
                        if !self.source[self.pos..].starts_with("\\u") {
                            return Err(self.error_at(
                                escape_start,
                                "component-string",
                                "Unicode 高代理项需要低代理项",
                            ));
                        }
                        self.pos += 2;
                        let low = self.hex4()?;
                        if !(0xdc00..=0xdfff).contains(&low) {
                            return Err(self.error_at(
                                escape_start,
                                "component-string",
                                "无效的 Unicode 代理项对",
                            ));
                        }
                        0x10000 + ((high - 0xd800) << 10) + low - 0xdc00
                    } else {
                        high
                    };
                    char::from_u32(code).ok_or_else(|| {
                        self.error_at(escape_start, "component-string", "无效的 Unicode 标量")
                    })?
                }
                _ => {
                    return Err(self.error_at(
                        escape_start,
                        "component-string",
                        "不支持的字符串转义",
                    ));
                }
            });
        }
        Err(self.error_at(start, "component-string", "字符串缺少结束引号"))
    }

    fn hex4(&mut self) -> Result<u32> {
        let mut value = 0;
        for _ in 0..4 {
            let Some(ch) = self.source[self.pos..].chars().next() else {
                return Err(self.error("component-string", "Unicode 转义需要四位十六进制数"));
            };
            let Some(digit) = ch.to_digit(16) else {
                return Err(self.error("component-string", "Unicode 转义需要四位十六进制数"));
            };
            value = value * 16 + digit;
            self.pos += ch.len_utf8();
        }
        Ok(value)
    }

    pub(super) fn number(&mut self) -> Result<ExprKind> {
        let start = self.pos;
        self.digits();
        let mut float = false;
        if self.source[self.pos..].starts_with('.')
            && self.source[self.pos + 1..]
                .bytes()
                .next()
                .is_some_and(|b| b.is_ascii_digit())
        {
            self.pos += 1;
            self.digits();
            float = true;
        }
        if self.source[self.pos..].starts_with(['e', 'E']) {
            float = true;
            self.pos += 1;
            if self.source[self.pos..].starts_with(['+', '-']) {
                self.pos += 1;
            }
            let digits_start = self.pos;
            self.digits();
            if digits_start == self.pos {
                return Err(self.error("component-number", "指数需要数字"));
            }
        }
        let text = &self.source[start..self.pos];
        self.token(start, Kind::Literal)?;
        if float {
            let value: f64 = text
                .parse()
                .map_err(|_| self.error_at(start, "component-number", "无效的浮点数"))?;
            if !value.is_finite() {
                return Err(self.error_at(start, "component-number", "Float 必须为有限数"));
            }
            Ok(ExprKind::Float(value))
        } else {
            let value: u64 = text
                .parse()
                .map_err(|_| self.error_at(start, "component-number", "Int 字面量超过范围"))?;
            if value > i64::MAX as u64 + 1 {
                return Err(self.error_at(start, "component-number", "Int 字面量超过范围"));
            }
            Ok(ExprKind::Integer(value))
        }
    }

    fn digits(&mut self) {
        self.pos += self.source[self.pos..]
            .bytes()
            .take_while(|b| b.is_ascii_digit())
            .count();
    }

    // 参数前缀已能区分大部分 lambda；仅 () / (name) 需要看紧随的箭头。
    // 不扫描整个括号内容，以免把 JSX 文本当代码字符串，也不试建/丢弃 AST。
    pub(super) fn lambda_ahead(&mut self) -> Result<bool> {
        let saved = self.pos;
        let concrete = self.concrete.take();
        let result = self.lambda_ahead_inner();
        self.pos = saved;
        self.concrete = concrete;
        result
    }

    fn lambda_ahead_inner(&mut self) -> Result<bool> {
        self.expect("(")?;
        if self.eat(")")? {
            return self.at("=>");
        }
        if !self.source[self.pos..]
            .chars()
            .next()
            .is_some_and(name_start)
        {
            return Ok(false);
        }
        self.pos += self.source[self.pos..]
            .chars()
            .take_while(|ch| name_continue(*ch))
            .map(char::len_utf8)
            .sum::<usize>();
        if self.eat(")")? {
            return self.at("=>");
        }
        Ok(self.at(":")? || self.at("=")? || self.at(",")?)
    }
}
