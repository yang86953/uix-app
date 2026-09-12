//! 同一源码闭包的共享名称、类型、捕获与效果事实。生成器和动态装载均消费它，
//! 不各自解释名称或重新读取来源。此阶段本身不创建运行实例。

use super::*;
use crate::lang::{
    compiler::{DiagnosticPhase, source_graph::SourceId},
    runtime::{self, Effect, Type as DataType},
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

mod body;
mod expression;
mod scopes;
pub(super) use scopes::inspect;
mod symbols;
mod types;
use super::completion::LexicalScope;
use super::symbols::NameFact;
pub(super) use types::native_export_cost;
pub use types::*;
// 与 parser 相同：递归成功路径不为完整诊断预留大型 Result 错误栈槽。
type Result<T> = std::result::Result<T, Box<CompilerDiagnostic>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeId {
    pub source: SourceId,
    pub start: usize,
    pub end: usize,
}
impl NodeId {
    fn new(source: SourceId, span: Span) -> Self {
        Self {
            source,
            start: span.start,
            end: span.end,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedName {
    Binding(NodeId),
    Function(NodeId),
    Component(DeclarationId),
    NativeFunction { package: String, name: String },
    NativeComponent { package: String, name: String },
    Method(String),
    Length,
}

#[derive(Debug, Clone)]
pub struct ExpressionFact {
    pub ty: Type,
    pub effect: Effect,
    pub call_effect: Option<Effect>,
    pub resolution: Option<ResolvedName>,
    pub constant: Option<runtime::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingKind {
    Input,
    State,
    Local,
    Function,
}
#[derive(Debug, Clone)]
pub struct BindingFact {
    pub name: String,
    pub ty: Type,
    pub owner: NodeId,
    pub kind: BindingKind,
}

/// 只读的受检产物；不能通过公开字段修改 AST 后继续冒用既有检查结果。
#[derive(Debug, Clone)]
pub struct CheckedSource {
    linked: LinkedSource,
    expressions: BTreeMap<NodeId, ExpressionFact>,
    components: BTreeMap<DeclarationId, ComponentSignature>,
    functions: BTreeMap<NodeId, FunctionSignature>,
    captures: BTreeMap<NodeId, BTreeSet<NodeId>>,
    bindings: BTreeMap<NodeId, BindingFact>,
    native_imports: NativeLibraries,
    names: BTreeMap<NodeId, NameFact>,
}
impl CheckedSource {
    pub fn source(&self) -> &LinkedSource {
        &self.linked
    }
    pub fn expressions(&self) -> &BTreeMap<NodeId, ExpressionFact> {
        &self.expressions
    }
    pub fn components(&self) -> &BTreeMap<DeclarationId, ComponentSignature> {
        &self.components
    }
    pub fn functions(&self) -> &BTreeMap<NodeId, FunctionSignature> {
        &self.functions
    }
    pub fn captures(&self) -> &BTreeMap<NodeId, BTreeSet<NodeId>> {
        &self.captures
    }
    pub fn bindings(&self) -> &BTreeMap<NodeId, BindingFact> {
        &self.bindings
    }
    pub fn native_imports(&self) -> &NativeLibraries {
        &self.native_imports
    }
    pub fn names(&self) -> &BTreeMap<NodeId, NameFact> {
        &self.names
    }
}

#[derive(Debug, Clone)]
struct Effects {
    base: Effect,
    calls: BTreeSet<NodeId>,
}
impl Default for Effects {
    fn default() -> Self {
        Self {
            base: Effect::Pure,
            calls: BTreeSet::new(),
        }
    }
}
impl Effects {
    fn known(effect: Effect) -> Self {
        Self {
            base: effect,
            ..Self::default()
        }
    }
    fn function(id: NodeId) -> Self {
        Self {
            calls: BTreeSet::from([id]),
            ..Self::default()
        }
    }
    fn add(&mut self, other: &Self) {
        self.base = self.base.max(other.base);
        self.calls.extend(&other.calls);
    }
    fn resolved(&self, functions: &BTreeMap<NodeId, Effect>) -> Effect {
        self.calls
            .iter()
            .fold(self.base, |effect, id| effect.max(functions[id]))
    }
}

#[derive(Clone)]
struct Fact {
    ty: Type,
    effects: Effects,
    callable: Option<Effects>,
    resolution: Option<ResolvedName>,
    constant: Option<runtime::Value>,
}
impl Fact {
    fn pure(ty: Type) -> Self {
        Self {
            ty,
            effects: Effects::default(),
            callable: None,
            resolution: None,
            constant: None,
        }
    }
}

#[derive(Clone)]
enum VariableKind {
    Input,
    State,
    Local,
    Function(NodeId),
}
#[derive(Clone)]
struct Variable {
    id: NodeId,
    owner: NodeId,
    ty: Type,
    kind: VariableKind,
    callable: Option<Effects>,
}
#[derive(Clone)]
struct Scope {
    source: SourceId,
    owner: NodeId,
    vars: BTreeMap<String, Variable>,
    locals: BTreeSet<String>,
    unavailable: BTreeSet<String>,
    returns: Option<Type>,
    effects: Effects,
    trace: usize,
    visible_from: usize,
}
impl Scope {
    fn new(owner: NodeId, returns: Option<Type>) -> Self {
        Self {
            source: owner.source,
            owner,
            vars: BTreeMap::new(),
            locals: BTreeSet::new(),
            unavailable: BTreeSet::new(),
            returns,
            effects: Effects::default(),
            trace: usize::MAX,
            visible_from: owner.start,
        }
    }
}

struct Requirement {
    at: NodeId,
    effects: Effects,
    maximum: Effect,
    message: &'static str,
}

struct Checker<'a> {
    linked: &'a LinkedSource,
    libraries: &'a NativeLibraries,
    native_imports: NativeLibraries,
    aliases: BTreeMap<DeclarationId, Type>,
    visiting_types: BTreeSet<DeclarationId>,
    components: BTreeMap<DeclarationId, ComponentSignature>,
    functions: BTreeMap<NodeId, FunctionSignature>,
    declared_functions: BTreeMap<DeclarationId, NodeId>,
    renderers: BTreeSet<NodeId>,
    function_effects: BTreeMap<NodeId, Effects>,
    expressions: BTreeMap<NodeId, Fact>,
    captures: BTreeMap<NodeId, BTreeSet<NodeId>>,
    bindings: BTreeMap<NodeId, BindingFact>,
    requirements: Vec<Requirement>,
    work: usize,
    names: BTreeMap<NodeId, NameFact>,
    scopes: Vec<LexicalScope>,
    record_scopes: bool,
}

fn checker<'a>(
    linked: &'a LinkedSource,
    libraries: &'a NativeLibraries,
    record_scopes: bool,
) -> Checker<'a> {
    Checker {
        linked,
        libraries,
        native_imports: BTreeMap::new(),
        aliases: BTreeMap::new(),
        visiting_types: BTreeSet::new(),
        components: BTreeMap::new(),
        functions: BTreeMap::new(),
        declared_functions: BTreeMap::new(),
        renderers: BTreeSet::new(),
        function_effects: BTreeMap::new(),
        expressions: BTreeMap::new(),
        captures: BTreeMap::new(),
        bindings: BTreeMap::new(),
        requirements: Vec::new(),
        work: 0,
        names: BTreeMap::new(),
        scopes: Vec::new(),
        record_scopes,
    }
}

fn analyze(checker: &mut Checker<'_>) -> Result<BTreeMap<NodeId, Effect>> {
    checker.native_interfaces()?;
    checker.signatures()?;
    checker.bodies()?;
    let effects = checker.solve_effects();
    for requirement in &checker.requirements {
        if requirement.effects.resolved(&effects) > requirement.maximum {
            return Err(checker.error(
                requirement.at.source,
                Span {
                    start: requirement.at.start,
                    end: requirement.at.end,
                },
                "component-effect",
                requirement.message,
            ));
        }
    }
    for (id, signature) in &mut checker.functions {
        signature.effect = effects[id];
    }
    Ok(effects)
}

/// 检查由 link_file 连接的闭包及显式原生导出签名。不读取/猜测官方组件库。
pub fn check(
    linked: LinkedSource,
    libraries: &NativeLibraries,
) -> std::result::Result<CheckedSource, CompilerDiagnostic> {
    let mut checker = checker(&linked, libraries, false);
    let effects = analyze(&mut checker).map_err(|error| *error)?;
    let expressions = checker
        .expressions
        .into_iter()
        .map(|(id, mut fact)| {
            let call_effect = fact
                .callable
                .as_ref()
                .map(|effect| effect.resolved(&effects));
            if let (Type::Function(signature), Some(effect)) = (&mut fact.ty, call_effect) {
                signature.effect = effect;
            }
            (
                id,
                ExpressionFact {
                    ty: fact.ty,
                    effect: fact.effects.resolved(&effects),
                    call_effect,
                    resolution: fact.resolution,
                    constant: fact.constant,
                },
            )
        })
        .collect();
    let Checker {
        components,
        functions,
        captures,
        bindings,
        native_imports,
        names,
        ..
    } = checker;
    Ok(CheckedSource {
        linked,
        expressions,
        components,
        functions,
        captures,
        bindings,
        native_imports,
        names,
    })
}

impl Checker<'_> {
    fn error(
        &self,
        source: SourceId,
        span: Span,
        code: &'static str,
        message: impl Into<String>,
    ) -> Box<CompilerDiagnostic> {
        let file = self
            .linked
            .source_graph
            .file(source)
            .expect("linked source identity");
        let prefix = &file.source[..span.start];
        Box::new(CompilerDiagnostic {
            code,
            phase: DiagnosticPhase::Semantic,
            source_id: source,
            source_name: file.path.clone(),
            start: span.start,
            end: span.end,
            line: prefix.bytes().filter(|byte| *byte == b'\n').count() + 1,
            column: prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1,
            message: message.into(),
            suggestion: "使用显式类型、词法名称和受检组件接口；持久状态写入放在事件函数中".into(),
        })
    }
    fn charge(&mut self, source: SourceId, span: Span, amount: usize) -> Result<()> {
        self.work = self.work.saturating_add(amount);
        if self.work > 262_144 {
            Err(self.error(
                source,
                span,
                "component-analysis-limit",
                "组件语义分析超过工作预算",
            ))
        } else {
            Ok(())
        }
    }
    fn native(
        &self,
        package: &str,
        name: &str,
        source: SourceId,
        span: Span,
    ) -> Result<&NativeExport> {
        self.native_imports
            .get(package)
            .and_then(|library| library.get(name))
            .ok_or_else(|| {
                self.error(
                    source,
                    span,
                    "component-native-export",
                    format!("原生库 {package} 未提供受检导出 {name}"),
                )
            })
    }
    fn require_effect(
        &mut self,
        source: SourceId,
        span: Span,
        effects: Effects,
        maximum: Effect,
        message: &'static str,
    ) {
        self.requirements.push(Requirement {
            at: NodeId::new(source, span),
            effects,
            maximum,
            message,
        });
    }
    fn solve_effects(&self) -> BTreeMap<NodeId, Effect> {
        let mut effects = self
            .function_effects
            .iter()
            .map(|(id, effect)| (*id, effect.base))
            .collect::<BTreeMap<_, _>>();
        let mut users = BTreeMap::<NodeId, BTreeSet<NodeId>>::new();
        for (caller, effect) in &self.function_effects {
            for callee in &effect.calls {
                users.entry(*callee).or_default().insert(*caller);
            }
        }
        let mut queue = effects.keys().copied().collect::<VecDeque<_>>();
        while let Some(callee) = queue.pop_front() {
            for caller in users.get(&callee).into_iter().flatten() {
                let next = effects[caller].max(effects[&callee]);
                if next != effects[caller] {
                    effects.insert(*caller, next);
                    queue.push_back(*caller);
                }
            }
        }
        effects
    }
    fn compatible(
        &mut self,
        source: SourceId,
        span: Span,
        actual: &Fact,
        expected: &Type,
    ) -> Result<()> {
        let valid = match (&actual.ty, expected) {
            (Type::Function(actual_signature), Type::Function(expected_signature)) => {
                if actual_signature.parameters == expected_signature.parameters
                    && actual_signature.returns == expected_signature.returns
                    && actual_signature.minimum_arguments <= expected_signature.minimum_arguments
                {
                    self.require_effect(
                        source,
                        span,
                        actual
                            .callable
                            .clone()
                            .unwrap_or_else(|| Effects::known(actual_signature.effect)),
                        expected_signature.effect,
                        "函数效果超出目标接口允许范围",
                    );
                    true
                } else {
                    false
                }
            }
            (actual, Type::View) => actual.is_view(),
            (actual, expected) => actual == expected,
        };
        if valid {
            Ok(())
        } else {
            Err(self.error(
                source,
                span,
                "component-type",
                format!("需要 {expected:?}，实际为 {:?}", actual.ty),
            ))
        }
    }
}
