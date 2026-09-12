//! 仅消费 CheckedSource 的身份、类型和效果事实，产出两条执行路径共用的 IR。
use super::*;
use crate::lang::{
    compiler::{DiagnosticPhase, source_graph::SourceId},
    runtime::{self, components as rt},
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;
mod collect;
mod expression;
use collect::Definition;
type Result<T> = std::result::Result<T, CompilerDiagnostic>;

/// 从根文件选择明确导出的组件；不会重新读取文件或猜测原生实现。
/// 返回构建用 IR。动态执行须显式 uix-dynamic；AOT 后端生成原生函数。
pub fn lower(checked: &CheckedSource, entry_export: &str) -> Result<rt::Program> {
    let root = checked.source().source_graph.root();
    let entry = checked.source().units[&root]
        .declarations
        .iter()
        .enumerate()
        .find(|(_, declaration)| {
            declaration.exported
                && declaration.name.text == entry_export
                && matches!(declaration.kind, DeclarationKind::Component { .. })
        })
        .map(|(index, _)| DeclarationId {
            source_id: root,
            index,
        });
    let mut lower = Lower {
        checked,
        functions: checked
            .functions()
            .keys()
            .enumerate()
            .map(|(id, node)| (*node, id))
            .collect(),
        components: checked
            .components()
            .keys()
            .enumerate()
            .map(|(id, node)| (*node, id))
            .collect(),
        bindings: checked
            .bindings()
            .keys()
            .enumerate()
            .map(|(id, node)| (*node, id))
            .collect(),
        definitions: BTreeMap::new(),
        function_bindings: BTreeMap::new(),
        renderers: BTreeMap::new(),
        sites: BTreeMap::new(),
    };
    let entry = entry.ok_or_else(|| {
        lower.error(
            node(root, Span { start: 0, end: 0 }),
            format!("根文件未导出组件 {entry_export}"),
        )
    })?;
    lower.collect();
    let captures = lower.captures()?;
    let mut functions = Vec::new();
    for (id, index) in &lower.functions {
        let definition = lower
            .definitions
            .get(id)
            .ok_or_else(|| lower.error(*id, "受检函数缺少结构化定义"))?;
        let renderer = lower.renderers.get(id).copied();
        let component = renderer.or_else(|| {
            lower
                .renderers
                .iter()
                .find(|(owner, _)| contains(**owner, *id))
                .map(|(_, component)| *component)
        });
        let parameters = if renderer.is_some() {
            Vec::new()
        } else {
            definition
                .parameters()
                .iter()
                .map(|parameter| lower.binding(id.source, parameter.name.span))
                .collect()
        };
        let defaults = if renderer.is_some() {
            Vec::new()
        } else {
            definition
                .parameters()
                .iter()
                .map(|parameter| {
                    parameter
                        .default
                        .as_ref()
                        .map(|expr| lower.expression_body(id.source, expr))
                        .transpose()
                })
                .collect::<Result<Vec<_>>>()?
        };
        let mut signature = checked.functions()[id].clone();
        if renderer.is_some() {
            signature.parameters.clear();
            signature.minimum_arguments = 0;
            signature.returns = Box::new(rt::Type::View);
        }
        let body = match definition {
            Definition::Render { body, .. } => rt::Body::Dynamic(
                body.iter()
                    .filter_map(|member| {
                        if let ComponentMember::Statement(statement) = member {
                            Some(lower.statement(id.source, statement))
                        } else {
                            None
                        }
                    })
                    .collect::<Result<Vec<_>>>()?
                    .into(),
            ),
            Definition::Function(function) => lower.block(id.source, &function.body)?,
            Definition::Lambda { body, .. } => match body {
                LambdaBody::Block(block) => lower.block(id.source, block)?,
                LambdaBody::Expression(expr) => lower.expression_body(id.source, expr)?,
            },
        };
        functions.push(rt::Function {
            signature,
            parameters,
            defaults,
            captures: captures[*index].iter().copied().collect(),
            component,
            body,
            location: lower.location(*id),
        });
    }
    let mut components = Vec::new();
    for (id, _) in &lower.components {
        let declaration = &checked.source().units[&id.source_id].declarations[id.index];
        let DeclarationKind::Component { parameters, body } = &declaration.kind else {
            unreachable!()
        };
        let owner = node(id.source_id, declaration.span);
        components.push(rt::Component {
            name: format!(
                "{}::{}",
                checked
                    .source()
                    .source_graph
                    .file(id.source_id)
                    .unwrap()
                    .path,
                declaration.name.text
            ),
            signature: checked.components()[id].clone(),
            inputs: parameters
                .iter()
                .map(|parameter| lower.binding(id.source_id, parameter.name.span))
                .collect(),
            defaults: parameters
                .iter()
                .map(|parameter| {
                    parameter
                        .default
                        .as_ref()
                        .map(|expr| lower.expression_body(id.source_id, expr))
                        .transpose()
                })
                .collect::<Result<_>>()?,
            states: body
                .iter()
                .filter_map(|member| {
                    if let ComponentMember::State { name, initial, .. } = member {
                        Some(
                            lower
                                .expression_body(id.source_id, initial)
                                .map(|body| (lower.binding(id.source_id, name.span), body)),
                        )
                    } else {
                        None
                    }
                })
                .collect::<Result<_>>()?,
            render: lower.functions[&owner],
            location: lower.location(owner),
        });
    }
    let bindings = checked
        .bindings()
        .iter()
        .map(|(id, binding)| {
            let component = lower.renderers.get(&binding.owner);
            let owner = match (binding.kind, component) {
                (BindingKind::Input | BindingKind::State, Some(component)) => {
                    rt::Owner::Component(*component)
                }
                _ => rt::Owner::Function(lower.functions[&binding.owner]),
            };
            let kind = match binding.kind {
                BindingKind::Input if component.is_some() => rt::BindingKind::Input,
                BindingKind::Input => rt::BindingKind::Parameter,
                BindingKind::State => rt::BindingKind::State,
                BindingKind::Local => rt::BindingKind::Local,
                BindingKind::Function => {
                    rt::BindingKind::Function(lower.functions[&lower.function_bindings[id]])
                }
            };
            let ty = if let rt::BindingKind::Function(target) = kind {
                rt::Type::Function(functions[target].signature.clone())
            } else {
                binding.ty.clone()
            };
            rt::Binding {
                name: binding.name.clone(),
                ty,
                owner,
                kind,
            }
        })
        .collect();
    let natives = checked
        .native_imports()
        .iter()
        .flat_map(|(package, exports)| {
            exports
                .iter()
                .map(move |(name, export)| (rt::ExportKey::new(package, name), export.clone()))
        })
        .collect();
    Ok(super::prune::reachable(rt::Program {
        components,
        functions,
        bindings,
        natives,
        entry: lower.components[&entry],
    }))
}

struct Lower<'a> {
    checked: &'a CheckedSource,
    functions: BTreeMap<NodeId, rt::FunctionId>,
    components: BTreeMap<DeclarationId, rt::ComponentId>,
    bindings: BTreeMap<NodeId, rt::BindingId>,
    definitions: BTreeMap<NodeId, Definition<'a>>,
    function_bindings: BTreeMap<NodeId, NodeId>,
    renderers: BTreeMap<NodeId, rt::ComponentId>,
    sites: BTreeMap<NodeId, usize>,
}
fn node(source: SourceId, span: Span) -> NodeId {
    NodeId {
        source,
        start: span.start,
        end: span.end,
    }
}
fn contains(outer: NodeId, inner: NodeId) -> bool {
    outer.source == inner.source && outer.start <= inner.start && outer.end >= inner.end
}
impl Lower<'_> {
    fn binding(&self, source: SourceId, span: Span) -> rt::BindingId {
        self.bindings[&node(source, span)]
    }
    fn location(&self, id: NodeId) -> runtime::Location {
        let file = self.checked.source().source_graph.file(id.source).unwrap();
        let prefix = &file.source[..id.start];
        runtime::Location {
            source: file.path.clone(),
            line: prefix.bytes().filter(|byte| *byte == b'\n').count() + 1,
            column: prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1,
        }
    }
    fn error(&self, id: NodeId, message: impl Into<String>) -> CompilerDiagnostic {
        let location = self.location(id);
        CompilerDiagnostic {
            code: "component-lower",
            phase: DiagnosticPhase::Semantic,
            source_id: id.source,
            source_name: location.source,
            start: id.start,
            end: id.end,
            line: location.line,
            column: location.column,
            message: message.into(),
            suggestion: "从同一 CheckedSource 生成组件产物；入口须是根文件的组件导出".into(),
        }
    }
    fn block(&self, source: SourceId, block: &Block) -> Result<rt::Body> {
        Ok(rt::Body::Dynamic(
            block
                .statements
                .iter()
                .map(|statement| self.statement(source, statement))
                .collect::<Result<Vec<_>>>()?
                .into(),
        ))
    }
    fn expression_body(&self, source: SourceId, expression: &Expr) -> Result<rt::Body> {
        Ok(rt::Body::Dynamic(Arc::from([rt::ir::Statement {
            location: self.location(node(source, expression.span)),
            kind: rt::ir::StatementKind::Return(Some(self.expr(source, expression)?)),
        }])))
    }
    fn statement(&self, source: SourceId, statement: &Statement) -> Result<rt::ir::Statement> {
        use rt::ir::StatementKind as S;
        let kind = match &statement.kind {
            StatementKind::Let { name, value, .. } => {
                S::Let(self.binding(source, name.span), self.expr(source, value)?)
            }
            StatementKind::Assign { target, value } => {
                let Some(ResolvedName::Binding(id)) =
                    &self.checked.expressions()[&node(source, target.span)].resolution
                else {
                    return Err(self.error(node(source, target.span), "赋值目标缺少绑定身份"));
                };
                S::Set(self.bindings[id], self.expr(source, value)?)
            }
            StatementKind::Evaluate(expr) => S::Evaluate(self.expr(source, expr)?),
            StatementKind::Return(expr) => S::Return(
                expr.as_ref()
                    .map(|expr| self.expr(source, expr))
                    .transpose()?,
            ),
            StatementKind::If {
                condition,
                then_block,
                else_block,
            } => S::If(
                self.expr(source, condition)?,
                then_block
                    .statements
                    .iter()
                    .map(|statement| self.statement(source, statement))
                    .collect::<Result<_>>()?,
                else_block
                    .iter()
                    .flat_map(|block| block.statements.iter())
                    .map(|statement| self.statement(source, statement))
                    .collect::<Result<_>>()?,
            ),
        };
        Ok(rt::ir::Statement {
            kind,
            location: self.location(node(source, statement.span)),
        })
    }
    fn captures(&self) -> Result<Vec<BTreeSet<rt::BindingId>>> {
        let mut captures = vec![BTreeSet::new(); self.functions.len()];
        let mut dependents = vec![BTreeSet::new(); self.functions.len()];
        for (function, bindings) in self.checked.captures() {
            for binding in bindings {
                let fact = &self.checked.bindings()[binding];
                if matches!(fact.kind, BindingKind::Local)
                    || (fact.kind == BindingKind::Input
                        && !self.renderers.contains_key(&fact.owner))
                {
                    captures[self.functions[function]].insert(self.bindings[binding]);
                }
            }
        }
        let mut declarations = self.functions.keys().copied().peekable();
        let mut active: Vec<NodeId> = Vec::new();
        for (expression, fact) in self.checked.expressions() {
            // 按源区间扫一次函数树，避免对每个函数引用扫描全部声明。
            while declarations
                .peek()
                .is_some_and(|id| (id.source, id.start) <= (expression.source, expression.start))
            {
                let id = declarations.next().unwrap();
                while active.last().is_some_and(|parent| !contains(*parent, id)) {
                    active.pop();
                }
                active.push(id);
            }
            while active
                .last()
                .is_some_and(|id| id.source != expression.source || id.end <= expression.start)
            {
                active.pop();
            }
            if let Some(ResolvedName::Function(target)) = &fact.resolution {
                // lambda 的创建属于外层函数，不属于 lambda 自己的函数体。
                let owner = active
                    .iter()
                    .rev()
                    .find(|owner| contains(**owner, *expression) && **owner != *expression);
                if let Some(owner) = owner {
                    dependents[self.functions[target]].insert(self.functions[owner]);
                }
            }
        }
        let binding_owners = self
            .checked
            .bindings()
            .values()
            .map(|fact| self.functions[&fact.owner])
            .collect::<Vec<_>>();
        let mut pending = (0..captures.len()).collect::<VecDeque<_>>();
        let mut work = 0usize;
        while let Some(target) = pending.pop_front() {
            for caller in &dependents[target] {
                let needed = captures[target]
                    .iter()
                    .copied()
                    .filter(|binding| binding_owners[*binding] != *caller)
                    .collect::<Vec<_>>();
                let mut changed = false;
                for binding in needed {
                    work += 1;
                    if work > 262144 {
                        return Err(self.error(
                            *self.functions.keys().next().unwrap(),
                            "闭包提升超过分析预算",
                        ));
                    }
                    changed |= captures[*caller].insert(binding);
                }
                if changed {
                    pending.push_back(*caller);
                }
            }
        }
        Ok(captures)
    }
}
