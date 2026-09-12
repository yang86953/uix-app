use super::*;

#[derive(Clone, Copy)]
pub(super) enum Context<'a> {
    None,
    Declaration,
    Value,
    Statement,
    Type,
    Tag,
    Property(&'a Element),
    Member(&'a Expr),
}
pub(super) fn context(unit: &SourceUnit, at: usize, in_scope: bool) -> Context<'_> {
    let mut cursor = Cursor {
        at,
        context: if in_scope {
            Context::Statement
        } else {
            Context::Declaration
        },
    };
    for import in &unit.imports {
        if cursor.inside(import.span) {
            return Context::None;
        }
    }
    for declaration in &unit.declarations {
        if !cursor.inside(declaration.span) {
            continue;
        }
        match &declaration.kind {
            DeclarationKind::Type(ty) => cursor.ty(ty),
            DeclarationKind::Function(function) => cursor.function(function),
            DeclarationKind::Component { parameters, body } => {
                cursor.parameters(parameters);
                for member in body {
                    match member {
                        ComponentMember::State {
                            name, ty, initial, ..
                        } => {
                            cursor.expr(initial);
                            cursor.ty(ty);
                            cursor.definition(name);
                        }
                        ComponentMember::Function { name, function, .. } => {
                            cursor.function(function);
                            cursor.definition(name);
                        }
                        ComponentMember::Statement(statement) => cursor.statement(statement),
                    }
                }
            }
        }
        cursor.definition(&declaration.name);
    }
    cursor.context
}
struct Cursor<'a> {
    at: usize,
    context: Context<'a>,
}
impl<'a> Cursor<'a> {
    fn inside(&self, span: Span) -> bool {
        span.start <= self.at && self.at <= span.end
    }
    fn definition(&mut self, name: &Name) {
        if self.inside(name.span) {
            self.context = Context::None;
        }
    }
    fn parameters(&mut self, parameters: &'a [Parameter]) {
        for parameter in parameters {
            if let Some(default) = &parameter.default {
                self.expr(default);
            }
            if let Some(ty) = &parameter.ty {
                self.ty(ty);
            }
            self.definition(&parameter.name);
        }
    }
    fn function(&mut self, function: &'a Function) {
        self.parameters(&function.parameters);
        self.ty(&function.returns);
        self.block(&function.body);
    }
    fn ty(&mut self, ty: &'a TypeNode) {
        if !self.inside(ty.span) {
            return;
        }
        self.context = Context::Type;
        match &ty.kind {
            TypeKind::Missing => {}
            TypeKind::Named { arguments, .. } => {
                for ty in arguments {
                    self.ty(ty);
                }
            }
            TypeKind::Array(ty) => self.ty(ty),
            TypeKind::Record(fields) => {
                for (name, ty) in fields {
                    self.ty(ty);
                    self.definition(name);
                }
            }
            TypeKind::Function {
                parameters,
                returns,
            } => {
                self.ty(returns);
                for (name, ty) in parameters {
                    self.ty(ty);
                    self.definition(name);
                }
            }
        }
    }
    fn block(&mut self, block: &'a Block) {
        if self.inside(block.span) && block.span.start < block.span.end {
            self.context = Context::Statement;
        }
        for statement in &block.statements {
            self.statement(statement);
        }
    }
    fn statement(&mut self, statement: &'a Statement) {
        if !self.inside(statement.span) {
            return;
        }
        self.context = Context::Value;
        match &statement.kind {
            StatementKind::Let { name, ty, value } => {
                self.expr(value);
                if let Some(ty) = ty {
                    self.ty(ty);
                }
                self.definition(name);
            }
            StatementKind::Assign { target, value } => {
                self.expr(target);
                self.expr(value);
            }
            StatementKind::Evaluate(value) => self.expr(value),
            StatementKind::Return(value) => {
                if let Some(value) = value {
                    self.expr(value);
                }
            }
            StatementKind::If {
                condition,
                then_block,
                else_block,
            } => {
                self.expr(condition);
                self.block(then_block);
                if let Some(block) = else_block {
                    self.block(block);
                }
            }
        }
    }
    fn expr(&mut self, expr: &'a Expr) {
        if !self.inside(expr.span) {
            return;
        }
        self.context = Context::Value;
        match &expr.kind {
            ExprKind::String(_)
            | ExprKind::Integer(_)
            | ExprKind::Float(_)
            | ExprKind::Bool(_)
            | ExprKind::Unit => self.context = Context::None,
            ExprKind::Name(_) | ExprKind::Missing => {}
            ExprKind::Array(values) => {
                for value in values {
                    self.expr(value);
                }
            }
            ExprKind::Record(fields) => {
                for (name, value) in fields {
                    self.expr(value);
                    self.definition(name);
                }
            }
            ExprKind::Unary { value, .. } => self.expr(value),
            ExprKind::Binary { left, right, .. } => {
                self.expr(left);
                self.expr(right);
            }
            ExprKind::Conditional { condition, yes, no } => {
                self.expr(condition);
                self.expr(yes);
                self.expr(no);
            }
            ExprKind::Index { value, index } => {
                self.expr(value);
                self.expr(index);
            }
            ExprKind::Member { value, name } => {
                self.expr(value);
                if self.inside(name.span) {
                    self.context = Context::Member(value);
                }
            }
            ExprKind::Call { callee, arguments } => {
                self.expr(callee);
                for value in arguments {
                    self.expr(value);
                }
            }
            ExprKind::Lambda { parameters, body } => {
                self.parameters(parameters);
                match body {
                    LambdaBody::Expression(expr) => self.expr(expr),
                    LambdaBody::Block(block) => self.block(block),
                }
            }
            ExprKind::Fragment(children) => self.children(children),
            ExprKind::Element(element) => {
                self.context = Context::None;
                if self.inside(element.opening_span) {
                    self.context = Context::Property(element);
                }
                for name in element
                    .name
                    .iter()
                    .chain(element.closing_name.iter().flatten())
                {
                    if self.inside(name.span) {
                        self.context = Context::Tag;
                    }
                }
                for attribute in &element.attributes {
                    if self.inside(attribute.span) && self.at > attribute.name.span.end {
                        self.context = Context::Value;
                    }
                    self.expr(&attribute.value);
                    if self.inside(attribute.name.span) {
                        self.context = Context::Property(element);
                    }
                }
                self.children(&element.children);
            }
        }
    }
    fn children(&mut self, children: &'a [ViewChild]) {
        for child in children {
            match child {
                ViewChild::Expression(value) => self.expr(value),
                ViewChild::Text { span, .. } if self.inside(*span) => self.context = Context::None,
                _ => {}
            }
        }
    }
}
