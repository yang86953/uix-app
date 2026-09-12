//! 查询受检源码闭包中的身份、定义和引用。位置属于当前 CheckedSource 快照，
//! 不跨编辑版本复用；原生接口不包含实现位置时，定义查询明确为空。
use super::*;
use crate::lang::compiler::source_graph::SourceId;
use std::collections::BTreeMap;

mod index;
mod presentation;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SymbolTarget {
    Binding(NodeId),
    Function(NodeId),
    Declaration(DeclarationId),
    Native {
        package: String,
        name: String,
    },
    NativeParameter {
        package: String,
        name: String,
        parameter: String,
    },
}

/// 名称的精确字节区间作为键；括号、注释和原始标签文本不是名称引用。
#[derive(Debug, Clone)]
pub struct NameFact {
    pub target: Option<SymbolTarget>,
    /// 字段/方法等没有名义定义时仍可查询受检表达式类型。
    pub expression: Option<NodeId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Component,
    Function,
    Type,
    Input,
    State,
    Local,
    Property,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OccurrenceKind {
    Declaration,
    Import,
    Reference,
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub target: SymbolTarget,
    pub name: String,
    pub kind: SymbolKind,
    pub definition: Option<NodeId>,
    pub extent: Option<NodeId>,
    pub parent: Option<SymbolTarget>,
    pub exported: bool,
}

#[derive(Debug, Clone)]
pub struct Occurrence {
    pub range: NodeId,
    pub kind: OccurrenceKind,
    pub fact: NameFact,
}

/// 索引借用同一不可变受检来源，不读取磁盘，不克隆类型/AST 或保存执行程序。
#[derive(Debug)]
pub struct SymbolIndex<'a> {
    checked: &'a CheckedSource,
    symbols: BTreeMap<SymbolTarget, Symbol>,
    occurrences: BTreeMap<NodeId, Occurrence>,
}

impl CheckedSource {
    pub fn symbol_index(&self) -> SymbolIndex<'_> {
        SymbolIndex::new(self)
    }
}

impl SymbolIndex<'_> {
    pub fn symbols(&self) -> &BTreeMap<SymbolTarget, Symbol> {
        &self.symbols
    }
    pub fn occurrences(&self) -> &BTreeMap<NodeId, Occurrence> {
        &self.occurrences
    }
    /// 半开区间查找；光标落在名称结尾、注释或字符串中不会命中相邻名称。
    pub fn at(&self, source: SourceId, offset: usize) -> Option<&Occurrence> {
        let before = NodeId {
            source,
            start: offset,
            end: usize::MAX,
        };
        self.occurrences
            .range(..=before)
            .next_back()
            .map(|(_, value)| value)
            .filter(|value| value.range.source == source && offset < value.range.end)
    }
    pub fn definition(&self, target: &SymbolTarget) -> Option<NodeId> {
        self.symbols.get(target)?.definition
    }
    /// 仅查询当前来源闭包。import 的选中导出和显式别名均属于引用而非目标定义。
    pub fn references(&self, target: &SymbolTarget, include_declaration: bool) -> Vec<NodeId> {
        self.occurrences
            .values()
            .filter(|value| {
                value.fact.target.as_ref() == Some(target)
                    && (include_declaration || value.kind != OccurrenceKind::Declaration)
            })
            .map(|value| value.range)
            .collect()
    }
}

pub(super) fn node(source: SourceId, span: Span) -> NodeId {
    NodeId {
        source,
        start: span.start,
        end: span.end,
    }
}

pub(super) fn binding_target(linked: &LinkedSource, binding: &Binding) -> SymbolTarget {
    match binding {
        Binding::Declaration(id) => {
            let declaration = &linked.units[&id.source_id].declarations[id.index];
            if matches!(declaration.kind, DeclarationKind::Function(_)) {
                SymbolTarget::Function(node(id.source_id, declaration.span))
            } else {
                SymbolTarget::Declaration(*id)
            }
        }
        Binding::NativeImport { package, name } => SymbolTarget::Native {
            package: package.clone(),
            name: name.clone(),
        },
    }
}

pub(super) fn resolved_target(value: &ResolvedName) -> Option<SymbolTarget> {
    Some(match value {
        ResolvedName::Binding(id) => SymbolTarget::Binding(*id),
        ResolvedName::Function(id) => SymbolTarget::Function(*id),
        ResolvedName::Component(id) => SymbolTarget::Declaration(*id),
        ResolvedName::NativeFunction { package, name }
        | ResolvedName::NativeComponent { package, name } => SymbolTarget::Native {
            package: package.clone(),
            name: name.clone(),
        },
        ResolvedName::Method(_) | ResolvedName::Length => return None,
    })
}
