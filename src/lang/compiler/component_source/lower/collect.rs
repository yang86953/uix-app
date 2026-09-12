use super::*;
pub(super) enum Definition<'a> {
    Render {
        parameters: &'a [Parameter],
        body: &'a [ComponentMember],
    },
    Function(&'a Function),
    Lambda {
        parameters: &'a [Parameter],
        body: &'a LambdaBody,
    },
}
impl Definition<'_> {
    pub(super) fn parameters(&self) -> &[Parameter] {
        match self {
            Self::Render { parameters, .. } | Self::Lambda { parameters, .. } => parameters,
            Self::Function(function) => &function.parameters,
        }
    }
}
impl<'a> Lower<'a> {
    pub(super) fn collect(&mut self) {
        for unit in self.checked.source().units.values() {
            for (index, declaration) in unit.declarations.iter().enumerate() {
                let source = unit.source_id;
                let owner = node(source, declaration.span);
                match &declaration.kind {
                    DeclarationKind::Component { parameters, body } => {
                        self.renderers.insert(
                            owner,
                            self.components[&DeclarationId {
                                source_id: source,
                                index,
                            }],
                        );
                        self.definitions
                            .insert(owner, Definition::Render { parameters, body });
                        self.scan_parameters(source, parameters);
                        for member in body {
                            match member {
                                ComponentMember::State { initial, .. } => {
                                    self.scan_expr(source, initial)
                                }
                                ComponentMember::Function {
                                    name,
                                    function,
                                    span,
                                } => {
                                    let id = node(source, *span);
                                    self.function_bindings.insert(node(source, name.span), id);
                                    self.definitions.insert(id, Definition::Function(function));
                                    self.scan_parameters(source, &function.parameters);
                                    self.scan_block(source, &function.body);
                                }
                                ComponentMember::Statement(statement) => {
                                    self.scan_statement(source, statement)
                                }
                            }
                        }
                    }
                    DeclarationKind::Function(function) => {
                        self.definitions
                            .insert(owner, Definition::Function(function));
                        self.scan_parameters(source, &function.parameters);
                        self.scan_block(source, &function.body);
                    }
                    DeclarationKind::Type(_) => {}
                }
            }
        }
    }
    fn scan_parameters(&mut self, source: SourceId, parameters: &'a [Parameter]) {
        for parameter in parameters {
            if let Some(expr) = &parameter.default {
                self.scan_expr(source, expr)
            }
        }
    }
    fn scan_block(&mut self, source: SourceId, block: &'a Block) {
        for statement in &block.statements {
            self.scan_statement(source, statement)
        }
    }
    fn scan_statement(&mut self, source: SourceId, statement: &'a Statement) {
        match &statement.kind {
            StatementKind::Let { value, .. } | StatementKind::Evaluate(value) => {
                self.scan_expr(source, value)
            }
            StatementKind::Assign { target, value } => {
                self.scan_expr(source, target);
                self.scan_expr(source, value)
            }
            StatementKind::Return(value) => {
                if let Some(value) = value {
                    self.scan_expr(source, value)
                }
            }
            StatementKind::If {
                condition,
                then_block,
                else_block,
            } => {
                self.scan_expr(source, condition);
                self.scan_block(source, then_block);
                if let Some(block) = else_block {
                    self.scan_block(source, block)
                }
            }
        }
    }
    fn scan_children(&mut self, source: SourceId, children: &'a [ViewChild]) {
        for child in children {
            if let ViewChild::Expression(expr) = child {
                self.scan_expr(source, expr)
            }
        }
    }
    fn scan_expr(&mut self, source: SourceId, expr: &'a Expr) {
        match &expr.kind {
            ExprKind::Lambda { parameters, body } => {
                self.definitions.insert(
                    node(source, expr.span),
                    Definition::Lambda { parameters, body },
                );
                self.scan_parameters(source, parameters);
                match body {
                    LambdaBody::Expression(expr) => self.scan_expr(source, expr),
                    LambdaBody::Block(block) => self.scan_block(source, block),
                }
            }
            ExprKind::Unary { value, .. } | ExprKind::Member { value, .. } => {
                self.scan_expr(source, value)
            }
            ExprKind::Binary { left, right, .. } => {
                self.scan_expr(source, left);
                self.scan_expr(source, right)
            }
            ExprKind::Index { value, index } => {
                self.scan_expr(source, value);
                self.scan_expr(source, index)
            }
            ExprKind::Conditional { condition, yes, no } => {
                self.scan_expr(source, condition);
                self.scan_expr(source, yes);
                self.scan_expr(source, no)
            }
            ExprKind::Array(values) => {
                for expr in values {
                    self.scan_expr(source, expr)
                }
            }
            ExprKind::Record(fields) => {
                for (_, expr) in fields {
                    self.scan_expr(source, expr)
                }
            }
            ExprKind::Call { callee, arguments } => {
                self.scan_expr(source, callee);
                for expr in arguments {
                    self.scan_expr(source, expr)
                }
            }
            ExprKind::Element(element) => {
                self.sites.insert(node(source, expr.span), self.sites.len());
                for attribute in &element.attributes {
                    self.scan_expr(source, &attribute.value)
                }
                self.scan_children(source, &element.children)
            }
            ExprKind::Fragment(children) => self.scan_children(source, children),
            _ => {}
        }
    }
}
