//! 显式清除位置后用 AST 的结构化 PartialEq 比较；不能改写 Debug 文本中的字符串。
use super::*;
fn span(value: &mut Span) {
    *value = Span { start: 0, end: 0 };
}
fn name(value: &mut Name) {
    span(&mut value.span);
}
pub(super) fn clear(parsed: &mut ParsedSource) {
    for import in &mut parsed.imports {
        span(&mut import.span);
        span(&mut import.source_span);
        for imported in &mut import.names {
            name(&mut imported.imported);
            name(&mut imported.local);
        }
    }
    for declaration in &mut parsed.declarations {
        span(&mut declaration.span);
        name(&mut declaration.name);
        match &mut declaration.kind {
            DeclarationKind::Type(value) => ty(value),
            DeclarationKind::Function(value) => function(value),
            DeclarationKind::Component { parameters, body } => {
                parameters.iter_mut().for_each(parameter);
                for member in body {
                    match member {
                        ComponentMember::State {
                            name: n,
                            ty: t,
                            initial,
                            span: s,
                        } => {
                            name(n);
                            ty(t);
                            expr(initial);
                            span(s);
                        }
                        ComponentMember::Function {
                            name: n,
                            function: f,
                            span: s,
                        } => {
                            name(n);
                            function(f);
                            span(s);
                        }
                        ComponentMember::Statement(value) => statement(value),
                    }
                }
            }
        }
    }
}
fn parameter(value: &mut Parameter) {
    name(&mut value.name);
    span(&mut value.span);
    if let Some(value) = &mut value.ty {
        ty(value);
    }
    if let Some(value) = &mut value.default {
        expr(value);
    }
}
fn function(value: &mut Function) {
    value.parameters.iter_mut().for_each(parameter);
    ty(&mut value.returns);
    block(&mut value.body);
}
fn ty(value: &mut TypeNode) {
    span(&mut value.span);
    match &mut value.kind {
        TypeKind::Named { path, arguments } => {
            path.iter_mut().for_each(name);
            arguments.iter_mut().for_each(ty);
        }
        TypeKind::Array(value) => ty(value),
        TypeKind::Record(fields) => {
            for (n, t) in fields {
                name(n);
                ty(t);
            }
        }
        TypeKind::Function {
            parameters,
            returns,
        } => {
            for (n, t) in parameters {
                name(n);
                ty(t);
            }
            ty(returns);
        }
    }
}
fn block(value: &mut Block) {
    span(&mut value.span);
    value.statements.iter_mut().for_each(statement);
}
fn statement(value: &mut Statement) {
    span(&mut value.span);
    match &mut value.kind {
        StatementKind::Let {
            name: n,
            ty: t,
            value,
        } => {
            name(n);
            if let Some(t) = t {
                ty(t);
            }
            expr(value);
        }
        StatementKind::Assign { target, value } => {
            expr(target);
            expr(value);
        }
        StatementKind::Evaluate(value) => expr(value),
        StatementKind::Return(value) => {
            if let Some(value) = value {
                expr(value);
            }
        }
        StatementKind::If {
            condition,
            then_block,
            else_block,
        } => {
            expr(condition);
            block(then_block);
            if let Some(value) = else_block {
                block(value);
            }
        }
    }
}
fn children(values: &mut [ViewChild]) {
    for value in values {
        match value {
            ViewChild::Text { span: s, .. } => span(s),
            ViewChild::Expression(value) => expr(value),
        }
    }
}
fn expr(value: &mut Expr) {
    span(&mut value.span);
    match &mut value.kind {
        ExprKind::Unit
        | ExprKind::Bool(_)
        | ExprKind::Integer(_)
        | ExprKind::Float(_)
        | ExprKind::String(_) => {}
        ExprKind::Name(value) => name(value),
        ExprKind::Array(values) => values.iter_mut().for_each(expr),
        ExprKind::Record(values) => {
            for (n, value) in values {
                name(n);
                expr(value);
            }
        }
        ExprKind::Unary { value, .. } => expr(value),
        ExprKind::Binary { left, right, .. } => {
            expr(left);
            expr(right);
        }
        ExprKind::Conditional { condition, yes, no } => {
            expr(condition);
            expr(yes);
            expr(no);
        }
        ExprKind::Member { value, name: n } => {
            expr(value);
            name(n);
        }
        ExprKind::Index { value, index } => {
            expr(value);
            expr(index);
        }
        ExprKind::Call { callee, arguments } => {
            expr(callee);
            arguments.iter_mut().for_each(expr);
        }
        ExprKind::Lambda { parameters, body } => {
            parameters.iter_mut().for_each(parameter);
            match body {
                LambdaBody::Expression(value) => expr(value),
                LambdaBody::Block(value) => block(value),
            }
        }
        ExprKind::Element(value) => {
            if let Some(names) = &mut value.closing_name {
                names.iter_mut().for_each(name);
            }
            value.name.iter_mut().for_each(name);
            for attribute in &mut value.attributes {
                name(&mut attribute.name);
                span(&mut attribute.span);
                expr(&mut attribute.value);
            }
            children(&mut value.children);
        }
        ExprKind::Fragment(values) => children(values),
    }
}
