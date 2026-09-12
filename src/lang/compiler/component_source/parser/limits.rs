use super::*;

// 解析器调用深度并不等于 AST 深度：括号里的左结合链可以再被外层链包裹。
// 每次组装都检查实际树高；已检查的子树至多 64 层，检查/错误回收不会先构造
// 一个任意深树。工作量上界为节点数 × 64，不复制子树。
pub(super) trait TreeDepth {
    fn tree_depth(&self) -> usize;
}

impl Parser<'_> {
    pub(super) fn checked<T: TreeDepth>(&self, value: T) -> Result<T> {
        if value.tree_depth() > MAX_DEPTH {
            Err(self.error(
                "component-depth-limit",
                "组件语法树深度超过上限，请拆分为局部值",
            ))
        } else {
            Ok(value)
        }
    }
}

fn maximum(values: impl IntoIterator<Item = usize>) -> usize {
    values.into_iter().max().unwrap_or(0)
}

impl TreeDepth for TypeNode {
    fn tree_depth(&self) -> usize {
        1 + match &self.kind {
            TypeKind::Named { arguments, .. } => {
                maximum(arguments.iter().map(TreeDepth::tree_depth))
            }
            TypeKind::Array(value) => value.tree_depth(),
            TypeKind::Record(fields) => maximum(fields.iter().map(|(_, ty)| ty.tree_depth())),
            TypeKind::Function {
                parameters,
                returns,
            } => {
                maximum(parameters.iter().map(|(_, ty)| ty.tree_depth())).max(returns.tree_depth())
            }
        }
    }
}

impl TreeDepth for Parameter {
    fn tree_depth(&self) -> usize {
        1 + self
            .ty
            .as_ref()
            .map_or(0, TreeDepth::tree_depth)
            .max(self.default.as_ref().map_or(0, TreeDepth::tree_depth))
    }
}

impl TreeDepth for Expr {
    fn tree_depth(&self) -> usize {
        1 + match &self.kind {
            ExprKind::Unit
            | ExprKind::Bool(_)
            | ExprKind::Integer(_)
            | ExprKind::Float(_)
            | ExprKind::String(_)
            | ExprKind::Name(_) => 0,
            ExprKind::Array(values) => maximum(values.iter().map(TreeDepth::tree_depth)),
            ExprKind::Record(fields) => maximum(fields.iter().map(|(_, expr)| expr.tree_depth())),
            ExprKind::Unary { value, .. } | ExprKind::Member { value, .. } => value.tree_depth(),
            ExprKind::Binary { left, right, .. } => left.tree_depth().max(right.tree_depth()),
            ExprKind::Index { value, index } => value.tree_depth().max(index.tree_depth()),
            ExprKind::Conditional { condition, yes, no } => condition
                .tree_depth()
                .max(yes.tree_depth())
                .max(no.tree_depth()),
            ExprKind::Call { callee, arguments } => callee
                .tree_depth()
                .max(maximum(arguments.iter().map(TreeDepth::tree_depth))),
            ExprKind::Lambda { parameters, body } => {
                maximum(parameters.iter().map(TreeDepth::tree_depth)).max(match body {
                    LambdaBody::Expression(expr) => expr.tree_depth(),
                    LambdaBody::Block(block) => block.tree_depth(),
                })
            }
            ExprKind::Element(element) => maximum(
                element
                    .attributes
                    .iter()
                    .map(|attribute| attribute.value.tree_depth()),
            )
            .max(maximum(element.children.iter().map(TreeDepth::tree_depth))),
            ExprKind::Fragment(children) => maximum(children.iter().map(TreeDepth::tree_depth)),
        }
    }
}

impl TreeDepth for ViewChild {
    fn tree_depth(&self) -> usize {
        match self {
            ViewChild::Text { .. } => 1,
            ViewChild::Expression(expr) => expr.tree_depth(),
        }
    }
}

impl TreeDepth for Block {
    fn tree_depth(&self) -> usize {
        1 + maximum(self.statements.iter().map(TreeDepth::tree_depth))
    }
}

impl TreeDepth for Statement {
    fn tree_depth(&self) -> usize {
        1 + match &self.kind {
            StatementKind::Let { ty, value, .. } => ty
                .as_ref()
                .map_or(0, TreeDepth::tree_depth)
                .max(value.tree_depth()),
            StatementKind::Assign { target, value } => target.tree_depth().max(value.tree_depth()),
            StatementKind::Evaluate(value) => value.tree_depth(),
            StatementKind::If {
                condition,
                then_block,
                else_block,
            } => condition
                .tree_depth()
                .max(then_block.tree_depth())
                .max(else_block.as_ref().map_or(0, TreeDepth::tree_depth)),
            StatementKind::Return(value) => value.as_ref().map_or(0, TreeDepth::tree_depth),
        }
    }
}
