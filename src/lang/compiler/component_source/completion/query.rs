use super::context::{Context, context};
use super::*;
use crate::lang::compiler::component_source::{
    parser::concrete::{Kind, parse_concrete},
    symbols::{binding_target, node, preview},
};
use crate::lang::{
    compiler::{DiagnosticPhase, source_graph::SourceId},
    runtime::{self, Type as DataType},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Variable,
    Function,
    Component,
    Type,
    Property,
    Method,
    Keyword,
}
#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: String,
    pub target: Option<SymbolTarget>,
}
#[derive(Debug, Clone)]
pub struct Completion {
    pub replacement: Span,
    pub items: Vec<CompletionItem>,
    pub is_incomplete: bool,
}

impl Inspection {
    /// 当前快照中的补全；部分分析只在已检查范围/失败点之前使用词法事实。
    pub fn complete(
        &self,
        source: SourceId,
        offset: usize,
    ) -> Result<Completion, CompilerDiagnostic> {
        let Some(file) = self.linked.source_graph.file(source) else {
            return Err(position_error(source, "未知补全来源"));
        };
        let offset = offset.min(file.source.len());
        if !file.source.is_char_boundary(offset) {
            return Err(position_error(source, "补全位置不在 UTF-8 字符边界"));
        }
        let replacement = parser::identifier_range(&file.source, offset);
        let point = replacement.start;
        let prefix = &file.source[point..offset];
        let mut output = Completion {
            replacement,
            items: Vec::new(),
            is_incomplete: self.diagnostic.is_some(),
        };
        // 资源限额失败不能转成继续分析的许可。
        if self
            .diagnostic
            .as_ref()
            .is_some_and(|d| d.code.ends_with("-limit"))
        {
            return Ok(output);
        }
        let (_, tokens) = parse_concrete(&file.source, file.path.clone())?;
        if tokens.iter().any(|token| {
            token.span.start <= point
                && point < token.span.end
                && matches!(
                    token.kind,
                    Kind::Literal | Kind::Text | Kind::LineComment | Kind::BlockComment
                )
        }) {
            return Ok(output);
        }
        let scope = self
            .scopes
            .iter()
            .filter(|scope| {
                scope.range.source == source
                    && scope.range.start <= point
                    && point < scope.range.end
            })
            .min_by_key(|scope| scope.range.end - scope.range.start);
        let usable = scope.filter(|scope| {
            scope.finished
                || scope.checked_until >= point
                || self
                    .diagnostic
                    .as_ref()
                    .is_none_or(|d| d.source_id == source && d.start >= point)
        });
        let unit = &self.linked.units[&source];
        let context = context(unit, point, scope.is_some());
        let mut candidates = BTreeMap::new();
        let globals = unit
            .bindings
            .iter()
            .map(|(name, binding)| (name.clone(), binding_target(&self.linked, binding)))
            .collect::<BTreeMap<_, _>>();
        match context {
            Context::None => {}
            Context::Declaration => {
                for name in parser::DECLARATION_WORDS {
                    insert(
                        &mut candidates,
                        name,
                        CompletionKind::Keyword,
                        "declaration".into(),
                        None,
                    );
                }
            }
            Context::Value | Context::Statement => {
                if let Some(scope) = usable {
                    let mut values = globals;
                    for (name, target) in scope.visible_at(point) {
                        match target {
                            Some(target) => {
                                values.insert(name, target);
                            }
                            None => {
                                values.remove(&name);
                            }
                        }
                    }
                    self.named(&mut candidates, values, |kind| {
                        matches!(kind, CompletionKind::Variable | CompletionKind::Function)
                    });
                    for name in ["true", "false"] {
                        insert(
                            &mut candidates,
                            name,
                            CompletionKind::Keyword,
                            "keyword".into(),
                            None,
                        );
                    }
                    if matches!(context, Context::Statement) {
                        for name in ["let", "if", "return"] {
                            insert(
                                &mut candidates,
                                name,
                                CompletionKind::Keyword,
                                "statement".into(),
                                None,
                            );
                        }
                    }
                }
            }
            Context::Type => {
                self.named(&mut candidates, globals, |kind| {
                    kind == CompletionKind::Type
                });
                for name in [
                    "Unit", "Bool", "Int", "Float", "String", "Bytes", "View", "Array", "Option",
                    "Result",
                ] {
                    insert(
                        &mut candidates,
                        name,
                        CompletionKind::Type,
                        "built-in type".into(),
                        None,
                    );
                }
            }
            Context::Tag => {
                if let Some(scope) = usable {
                    let mut globals = globals;
                    for name in scope.variables_at(point).keys() {
                        globals.remove(name);
                    }
                    self.named(&mut candidates, globals, |kind| {
                        kind == CompletionKind::Component
                    });
                }
            }
            Context::Property(element) => {
                if let (Some(scope), [name]) = (usable, element.name.as_slice()) {
                    if !scope.variables_at(point).contains_key(&name.text) {
                        if let Some(target) = globals.get(&name.text) {
                            if let Some(signature) = self.component(target) {
                                for (name, ty) in &signature.parameters {
                                    if element.attributes.iter().any(|attribute| {
                                        attribute.name.text == *name
                                            && !(attribute.name.span.start <= point
                                                && point <= attribute.name.span.end)
                                    }) {
                                        continue;
                                    }
                                    insert(
                                        &mut candidates,
                                        name,
                                        CompletionKind::Property,
                                        preview(format_args!(
                                            "{ty:?}{}",
                                            if signature.required.contains(name) {
                                                " (required)"
                                            } else {
                                                " (optional)"
                                            }
                                        )),
                                        None,
                                    );
                                }
                                if !element.attributes.iter().any(|attribute| {
                                    attribute.name.text == "key"
                                        && attribute.name.span.start != point
                                }) {
                                    insert(
                                        &mut candidates,
                                        "key",
                                        CompletionKind::Property,
                                        "Int | String (identity)".into(),
                                        None,
                                    );
                                }
                            }
                        }
                    }
                }
            }
            Context::Member(receiver) => {
                if usable.is_some() {
                    if let Some(ty) = self.types.get(&node(source, receiver.span)) {
                        members(&mut candidates, ty);
                    }
                }
            }
        }
        output.items = candidates
            .into_values()
            .filter(|item| item.label.starts_with(prefix))
            .collect();
        if output.items.len() > 256 {
            output.items.truncate(256);
            output.is_incomplete = true;
        }
        Ok(output)
    }
    fn named(
        &self,
        candidates: &mut BTreeMap<String, CompletionItem>,
        names: BTreeMap<String, SymbolTarget>,
        allowed: impl Fn(CompletionKind) -> bool,
    ) {
        for (label, target) in names {
            let Some((kind, detail)) = self.describe(&target) else {
                continue;
            };
            if allowed(kind) {
                insert(candidates, &label, kind, detail, Some(target));
            }
        }
    }
    fn describe(&self, target: &SymbolTarget) -> Option<(CompletionKind, String)> {
        Some(match target {
            SymbolTarget::Binding(id) => (
                CompletionKind::Variable,
                preview(format_args!("{:?}", self.bindings.get(id)?.ty)),
            ),
            SymbolTarget::Function(id) => (
                CompletionKind::Function,
                preview(format_args!("{:?}", self.functions.get(id)?)),
            ),
            SymbolTarget::Declaration(id) => {
                let declaration = &self.linked.units[&id.source_id].declarations[id.index];
                match &declaration.kind {
                    DeclarationKind::Component { .. } => {
                        (CompletionKind::Component, "component".into())
                    }
                    DeclarationKind::Type(_) => (CompletionKind::Type, "type alias".into()),
                    DeclarationKind::Function(_) => return None,
                }
            }
            SymbolTarget::Native { package, name } => {
                let export = self.native_imports.get(package)?.get(name)?;
                (
                    match export {
                        NativeExport::Component(_) => CompletionKind::Component,
                        NativeExport::Type(_) => CompletionKind::Type,
                        NativeExport::Function(_) => CompletionKind::Function,
                    },
                    preview(format_args!("{package}/{name}: {export:?}")),
                )
            }
            SymbolTarget::NativeParameter { .. } => return None,
        })
    }
    fn component(&self, target: &SymbolTarget) -> Option<&ComponentSignature> {
        match target {
            SymbolTarget::Declaration(id) => self.components.get(id),
            SymbolTarget::Native { package, name } => {
                match self.native_imports.get(package)?.get(name)? {
                    NativeExport::Component(signature) => Some(signature),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}
/// 第一条声明尚未成形时，仅返回共同语法中的声明关键字，不猜测符号或类型。
pub fn declaration_completion(source: &str, offset: usize) -> Option<Completion> {
    let replacement = parser::declaration_prefix(source, offset)?;
    let prefix = &source[replacement.start..offset];
    Some(Completion {
        replacement,
        is_incomplete: true,
        items: parser::DECLARATION_WORDS
            .into_iter()
            .filter(|word| word.starts_with(prefix))
            .map(|word| CompletionItem {
                label: word.into(),
                kind: CompletionKind::Keyword,
                detail: "declaration".into(),
                target: None,
            })
            .collect(),
    })
}
fn insert(
    items: &mut BTreeMap<String, CompletionItem>,
    label: &str,
    kind: CompletionKind,
    detail: String,
    target: Option<SymbolTarget>,
) {
    items.insert(
        label.into(),
        CompletionItem {
            label: label.into(),
            kind,
            detail,
            target,
        },
    );
}
fn members(items: &mut BTreeMap<String, CompletionItem>, ty: &Type) {
    if let Some(fields) = ty.fields() {
        for (name, ty) in fields {
            insert(
                items,
                &name,
                CompletionKind::Property,
                preview(format_args!("{ty:?}")),
                None,
            );
        }
        return;
    }
    if ty.element().is_some() {
        for name in ["map", "filter"] {
            insert(
                items,
                name,
                CompletionKind::Method,
                "array callback".into(),
                None,
            );
        }
    }
    if ty.element().is_some() || matches!(ty, Type::Data(DataType::String | DataType::Bytes)) {
        insert(
            items,
            "length",
            CompletionKind::Property,
            "Int".into(),
            None,
        );
    }
    if let Type::Data(ty) = ty {
        for name in runtime::method_names(ty) {
            insert(
                items,
                name,
                CompletionKind::Method,
                preview(format_args!(
                    "{:?}",
                    runtime::method_signature(ty, name).unwrap()
                )),
                None,
            );
        }
    }
}
fn position_error(source_id: SourceId, message: &str) -> CompilerDiagnostic {
    CompilerDiagnostic {
        source_id,
        code: "component-completion-position",
        phase: DiagnosticPhase::Source,
        source_name: "<completion>".into(),
        start: 0,
        end: 0,
        line: 1,
        column: 1,
        message: message.into(),
        suggestion: "使用当前源码快照中的 UTF-8 字节位置".into(),
    }
}
