//! 属性字面量到原始源码的跨度映射；语句与表达式仍只使用共享解析器。

use super::*;

pub(super) fn parse(attribute: &Attribute, source: &str) -> Result<ModuleActionBody, Diagnostic> {
    let AttributeValue::Literal(decoded) = &attribute.value else {
        return Err(fail(attribute.span, "body 必须是字符串字面量"));
    };
    let raw = &source[attribute.span.start..attribute.span.end];
    let mut cursor = Cursor::new(raw);
    cursor.identifier();
    cursor.skip_trivia()?;
    if !cursor.consume("=") {
        return Err(fail(attribute.span, "body 需要显式字符串主体"));
    }
    cursor.skip_trivia()?;
    if !cursor.consume("\"") {
        return Err(fail(attribute.span, "body 需要双引号"));
    }
    let start = cursor.offset();
    let end = raw.len() - 1;
    let mut offsets = Vec::with_capacity(decoded.len() + 1);
    let mut characters = raw[start..end].char_indices();
    while let Some((index, character)) = characters.next() {
        let value = if character == '\\' {
            characters
                .next()
                .map(|(_, c)| c)
                .ok_or_else(|| fail(attribute.span, "body 转义未闭合"))?
        } else {
            character
        };
        offsets.extend(std::iter::repeat_n(
            attribute.span.start + start + index,
            value.len_utf8(),
        ));
    }
    offsets.push(attribute.span.start + end);
    let trimmed = decoded.trim();
    let prefix = decoded.len() - decoded.trim_start().len();
    let positions = Cursor::new(source);
    let map = |span: &mut SourceSpan| {
        let start = offsets[(prefix + span.start).min(offsets.len() - 1)];
        let end = offsets[(prefix + span.end).min(offsets.len() - 1)];
        *span = positions.span_between(start, end);
    };
    let mut body = parse_module_body(
        trimmed,
        SourceSpan {
            start: 0,
            end: trimmed.len(),
            line: 1,
            column: 1,
        },
    )
    .map_err(|mut error| {
        map(&mut error.span);
        error
    })?;
    for point in &mut body.awaits {
        *point = offsets[(prefix + *point).min(offsets.len() - 1)];
    }
    match &mut body.body {
        ActionBody::Expression(value) => expression(value, &map),
        ActionBody::Block(value) => block(value, &map),
    }
    Ok(body)
}

fn expression(value: &mut Expression, map: &impl Fn(&mut SourceSpan)) {
    map(&mut value.span);
    match &mut value.kind {
        ExpressionKind::Unary { operand, .. } => expression(operand, map),
        ExpressionKind::Binary { left, right, .. } => {
            expression(left, map);
            expression(right, map);
        }
        ExpressionKind::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            expression(condition, map);
            expression(then_branch, map);
            expression(else_branch, map);
        }
        ExpressionKind::Array(values) => values.iter_mut().for_each(|value| expression(value, map)),
        ExpressionKind::Object(fields) => {
            for field in fields {
                map(&mut field.span);
                expression(&mut field.value, map);
            }
        }
        ExpressionKind::Member { object, .. } => expression(object, map),
        ExpressionKind::Index { object, index } => {
            expression(object, map);
            expression(index, map);
        }
        ExpressionKind::Call { callee, arguments } => {
            expression(callee, map);
            for argument in arguments {
                map(&mut argument.span);
                expression(&mut argument.value, map);
            }
        }
        ExpressionKind::Closure { body, .. } => expression(body, map),
        _ => {}
    }
}

fn block(value: &mut ActionBlock, map: &impl Fn(&mut SourceSpan)) {
    map(&mut value.span);
    for statement in &mut value.statements {
        match statement {
            ActionStatement::Let {
                initializer, span, ..
            } => {
                map(span);
                expression(initializer, map);
            }
            ActionStatement::Assign { value, span, .. } => {
                map(span);
                expression(value, map);
            }
            ActionStatement::Expression {
                expression: value,
                span,
            } => {
                map(span);
                expression(value, map);
            }
            ActionStatement::Return { value, span } => {
                map(span);
                if let Some(value) = value {
                    expression(value, map);
                }
            }
            ActionStatement::If {
                condition,
                then_block,
                else_block,
                span,
            } => {
                map(span);
                expression(condition, map);
                block(then_block, map);
                if let Some(no) = else_block {
                    block(no, map);
                }
            }
        }
    }
}
