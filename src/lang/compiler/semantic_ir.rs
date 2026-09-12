//! UIX Lang 完成名称分类后的可查询语义 IR。

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use crate::lang::compiler::CompileTarget;
use crate::lang::compiler::projection_schema::{ComponentCategory, UI_PROJECTION_SCHEMA};
use crate::lang::compiler::source_graph::SourceId;
use crate::lang::compiler::uix_lang::{
    Attribute, AttributeValue, ControlBinding, Declaration, Diagnostic, Document, Element, Node,
    SourceSpan,
};

/// 保存绑定到稳定源码身份的半开字节范围。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IrSpan {
    pub source_id: SourceId,
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

/// 区分语义模型中的顶层声明。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypedDeclarationKind {
    Style,
    Theme,
    Keyframes,
    Media,
    Widget,
    Record,
    Visual,
}

/// 保存已解析名称和来源的顶层声明。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedDeclaration {
    pub kind: TypedDeclarationKind,
    pub name: String,
    pub span: IrSpan,
    pub body: Vec<TypedNode>,
}

/// 区分内置组件、自定义组件与语言控制节点。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedElementKind {
    LibraryComponent { unit: String },
    Builtin {
        component_id: &'static str,
        category: ComponentCategory,
    },
    CustomWidget,
    Control,
}

/// 标识属性在 UI 投影中的职责。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypedAttributeRole {
    Property,
    Event,
    Style,
    Control,
}

/// 标识属性值已经解析到的闭合形状。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypedValueKind {
    Literal,
    Expression,
    InlineStyle,
}

/// 保存完成角色和值形状分类的属性。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedAttribute {
    pub name: String,
    pub role: TypedAttributeRole,
    pub value_kind: TypedValueKind,
    pub span: IrSpan,
}

/// 保存语义 IR 中一个可生成或控制元素。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedElement {
    pub name: String,
    pub kind: TypedElementKind,
    pub attributes: Vec<TypedAttribute>,
    pub children: Vec<TypedNode>,
    pub span: IrSpan,
}

/// 保存有序 UI 节点，不携带未经解析的源码字符串。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedNode {
    Element(TypedElement),
    Text(IrSpan),
    Interpolation(IrSpan),
}

/// 区分查询命中的语义节点类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticNodeKind {
    Declaration,
    Element,
    Attribute,
    Text,
    Interpolation,
}

impl SemanticNodeKind {
    /// 返回供 SourceMap、CLI 与 LSP 使用的稳定名称。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Declaration => "declaration",
            Self::Element => "element",
            Self::Attribute => "attribute",
            Self::Text => "text",
            Self::Interpolation => "interpolation",
        }
    }
}

/// 保存一个可由源码位置查询的稳定语义节点。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticNodeInfo {
    pub id: String,
    pub kind: SemanticNodeKind,
    pub name: String,
    pub span: IrSpan,
}

/// 保存一处由 schema 判定的 capability 使用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityRequirement {
    pub capability: &'static str,
    pub component: String,
    pub attribute: Option<String>,
    pub span: IrSpan,
}

/// 保存语义降低失败及其真实源码身份。
#[derive(Debug)]
pub(crate) struct SemanticFailure {
    pub(crate) source_id: SourceId,
    pub(crate) diagnostic: Diagnostic,
}

/// 保存 Compiler System 语义阶段的确定性结果。
#[derive(Debug, Clone)]
pub struct TypedUiIr {
    target: CompileTarget,
    // 类型化声明在分析完成后不可变，检查结果只共享拥有型序列。
    declarations: Arc<[TypedDeclaration]>,
    root: TypedElement,
    // 原始文档只在 lowering 时复制为发射文档，公开结果克隆无需重复持有整棵 AST。
    document: Arc<Document>,
}

impl TypedUiIr {
    /// 返回本次分析的入口形状。
    pub const fn target(&self) -> CompileTarget {
        self.target
    }

    /// 返回源码顺序中的具名声明。
    pub fn declarations(&self) -> &[TypedDeclaration] {
        self.declarations.as_ref()
    }

    /// 返回唯一根元素。
    pub const fn root(&self) -> &TypedElement {
        &self.root
    }

    /// 查询覆盖指定源码字节的最窄语义节点。
    pub fn semantic_node_at(&self, source_id: SourceId, offset: usize) -> Option<SemanticNodeInfo> {
        let mut best = None;
        for declaration in self.declarations.iter() {
            consider_node(
                &mut best,
                semantic_info(
                    SemanticNodeKind::Declaration,
                    &declaration.name,
                    declaration.span,
                ),
                source_id,
                offset,
            );
            for node in &declaration.body {
                find_node(node, source_id, offset, &mut best);
            }
        }
        find_element(&self.root, source_id, offset, &mut best);
        best
    }

    /// 返回源码顺序中的全部组件与属性级 capability 要求。
    pub fn capability_requirements(&self) -> Vec<CapabilityRequirement> {
        let mut requirements = Vec::new();
        for declaration in self.declarations.iter() {
            for node in &declaration.body {
                collect_node_capabilities(node, &mut requirements);
            }
        }
        collect_element_capabilities(&self.root, &mut requirements);
        requirements
    }

    // 为 Rust Emitter 克隆文档，并把每个可生成元素标上真实源码身份。
    pub(crate) fn emission_document(&self) -> Document {
        let mut document = self.document.as_ref().clone();
        let custom_widgets = self
            .declarations
            .iter()
            .filter(|declaration| declaration.kind == TypedDeclarationKind::Widget)
            .map(|declaration| declaration.name.clone())
            .collect::<BTreeSet<_>>();
        let widget_sources = self
            .declarations
            .iter()
            .filter(|declaration| declaration.kind == TypedDeclarationKind::Widget)
            .map(|declaration| (declaration.name.as_str(), declaration.span.source_id))
            .collect::<BTreeMap<_, _>>();
        for declaration in &mut document.declarations {
            if let Declaration::Widget(widget) = declaration {
                if let Some(source_id) = widget_sources.get(widget.name.as_str()).copied() {
                    annotate_nodes(&mut widget.children, source_id, &custom_widgets);
                }
            }
        }
        annotate_element(
            &mut document.root,
            self.root.span.source_id,
            &custom_widgets,
        );
        document
    }

    // 返回自定义组件名称到真实声明来源的确定映射。
    pub(crate) fn widget_source_ids(&self) -> BTreeMap<String, SourceId> {
        self.declarations
            .iter()
            .filter(|declaration| declaration.kind == TypedDeclarationKind::Widget)
            .map(|declaration| (declaration.name.clone(), declaration.span.source_id))
            .collect()
    }

    // 返回 Record 名称到真实声明来源的确定映射。
    pub(crate) fn record_source_ids(&self) -> BTreeMap<String, SourceId> {
        self.declarations
            .iter()
            .filter(|declaration| declaration.kind == TypedDeclarationKind::Record)
            .map(|declaration| (declaration.name.clone(), declaration.span.source_id))
            .collect()
    }

    // 返回 Visual 常量名称到真实声明来源的确定映射。
    pub(crate) fn visual_source_ids(&self) -> BTreeMap<String, SourceId> {
        self.declarations
            .iter()
            .filter(|declaration| declaration.kind == TypedDeclarationKind::Visual)
            .map(|declaration| (declaration.name.clone(), declaration.span.source_id))
            .collect()
    }
}

fn annotate_nodes(nodes: &mut [Node], source_id: SourceId, custom_widgets: &BTreeSet<String>) {
    for node in nodes {
        if let Node::Element(element) = node {
            annotate_element(element, source_id, custom_widgets);
        }
    }
}

fn annotate_element(element: &mut Element, source_id: SourceId, custom_widgets: &BTreeSet<String>) {
    if element.name != "App" && !custom_widgets.contains(&element.name) {
        element.attributes.push(Attribute {
            name: crate::lang::compiler::uix_lang::SOURCE_ID_ATTRIBUTE.to_string(),
            value: AttributeValue::Literal(source_id.value().to_string()),
            span: element.span,
        });
    }
    annotate_nodes(&mut element.children, source_id, custom_widgets);
}

fn collect_node_capabilities(node: &TypedNode, requirements: &mut Vec<CapabilityRequirement>) {
    if let TypedNode::Element(element) = node {
        collect_element_capabilities(element, requirements);
    }
}

fn collect_element_capabilities(
    element: &TypedElement,
    requirements: &mut Vec<CapabilityRequirement>,
) {
    if matches!(element.kind, TypedElementKind::Builtin { .. }) {
        if let Some(capability) = UI_PROJECTION_SCHEMA
            .component(&element.name)
            .and_then(|component| component.capability)
        {
            requirements.push(CapabilityRequirement {
                capability,
                component: element.name.clone(),
                attribute: None,
                span: element.span,
            });
        }
        for attribute in &element.attributes {
            if let Some(requirement) =
                UI_PROJECTION_SCHEMA.attribute_capability(&element.name, &attribute.name)
            {
                requirements.push(CapabilityRequirement {
                    capability: requirement.capability,
                    component: element.name.clone(),
                    attribute: Some(attribute.name.clone()),
                    span: attribute.span,
                });
            }
        }
    }
    for child in &element.children {
        collect_node_capabilities(child, requirements);
    }
}

fn find_node(
    node: &TypedNode,
    source_id: SourceId,
    offset: usize,
    best: &mut Option<SemanticNodeInfo>,
) {
    match node {
        TypedNode::Element(element) => find_element(element, source_id, offset, best),
        TypedNode::Text(span) => consider_node(
            best,
            semantic_info(SemanticNodeKind::Text, "#text", *span),
            source_id,
            offset,
        ),
        TypedNode::Interpolation(span) => consider_node(
            best,
            semantic_info(SemanticNodeKind::Interpolation, "#expression", *span),
            source_id,
            offset,
        ),
    }
}

fn find_element(
    element: &TypedElement,
    source_id: SourceId,
    offset: usize,
    best: &mut Option<SemanticNodeInfo>,
) {
    consider_node(
        best,
        semantic_info(SemanticNodeKind::Element, &element.name, element.span),
        source_id,
        offset,
    );
    for attribute in &element.attributes {
        consider_node(
            best,
            semantic_info(SemanticNodeKind::Attribute, &attribute.name, attribute.span),
            source_id,
            offset,
        );
    }
    for child in &element.children {
        find_node(child, source_id, offset, best);
    }
}

fn consider_node(
    best: &mut Option<SemanticNodeInfo>,
    candidate: SemanticNodeInfo,
    source_id: SourceId,
    offset: usize,
) {
    if candidate.span.source_id != source_id
        || offset < candidate.span.start
        || offset >= candidate.span.end
    {
        return;
    }
    let candidate_len = candidate.span.end.saturating_sub(candidate.span.start);
    let replace = best
        .as_ref()
        .is_none_or(|current| candidate_len < current.span.end.saturating_sub(current.span.start));
    if replace {
        *best = Some(candidate);
    }
}

fn semantic_info(kind: SemanticNodeKind, name: &str, span: IrSpan) -> SemanticNodeInfo {
    SemanticNodeInfo {
        id: format!(
            "{}.{:016x}.{}.{}",
            kind.as_str(),
            span.source_id.value(),
            span.start,
            name
        ),
        kind,
        name: name.to_string(),
        span,
    }
}

/// 把完成导入合并的 AST 降低为带稳定来源的语义 IR。
pub(crate) fn lower_document(
    document: Document,
    target: CompileTarget,
    root_source: SourceId,
    declaration_sources: &[SourceId],
) -> Result<TypedUiIr, SemanticFailure> {
    let custom_widgets = document
        .declarations
        .iter()
        .filter_map(|declaration| match declaration {
            Declaration::Widget(widget) => Some(widget.name.clone()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let declarations = document
        .declarations
        .iter()
        .enumerate()
        .filter_map(|(index, declaration)| {
            let source_id = declaration_sources
                .get(index)
                .copied()
                .unwrap_or(root_source);
            typed_declaration(declaration, source_id, &custom_widgets)
                .map_err(|diagnostic| SemanticFailure {
                    source_id,
                    diagnostic,
                })
                .transpose()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let root =
        typed_element(&document.root, root_source, &custom_widgets).map_err(|diagnostic| {
            SemanticFailure {
                source_id: root_source,
                diagnostic,
            }
        })?;
    Ok(TypedUiIr {
        target,
        declarations: declarations.into(),
        root,
        document: Arc::new(document),
    })
}

fn typed_declaration(
    declaration: &Declaration,
    source_id: SourceId,
    custom_widgets: &BTreeSet<String>,
) -> Result<Option<TypedDeclaration>, Diagnostic> {
    let value = match declaration {
        Declaration::StyleClass(value) => TypedDeclaration {
            kind: TypedDeclarationKind::Style,
            name: value.name.clone(),
            span: ir_span(source_id, value.span),
            body: Vec::new(),
        },
        Declaration::Theme(value) => TypedDeclaration {
            kind: TypedDeclarationKind::Theme,
            name: value.name.clone(),
            span: ir_span(source_id, value.span),
            body: Vec::new(),
        },
        Declaration::Keyframes(value) => TypedDeclaration {
            kind: TypedDeclarationKind::Keyframes,
            name: value.name.clone(),
            span: ir_span(source_id, value.span),
            body: Vec::new(),
        },
        Declaration::Media(value) => TypedDeclaration {
            kind: TypedDeclarationKind::Media,
            name: value.query.canonical_text(),
            span: ir_span(source_id, value.span),
            body: Vec::new(),
        },
        Declaration::Widget(value) => TypedDeclaration {
            kind: TypedDeclarationKind::Widget,
            name: value.name.clone(),
            span: ir_span(source_id, value.span),
            body: typed_nodes(&value.children, source_id, custom_widgets)?,
        },
        Declaration::Record(value) => TypedDeclaration {
            kind: TypedDeclarationKind::Record,
            name: value.name.clone(),
            span: ir_span(source_id, value.span),
            body: Vec::new(),
        },
        Declaration::Visual(value) => TypedDeclaration {
            kind: TypedDeclarationKind::Visual,
            name: value.name.clone(),
            span: ir_span(source_id, value.span),
            body: Vec::new(),
        },
        Declaration::Import(_) | Declaration::Export(_) => return Ok(None),
    };
    Ok(Some(value))
}

fn typed_nodes(
    nodes: &[Node],
    source_id: SourceId,
    custom_widgets: &BTreeSet<String>,
) -> Result<Vec<TypedNode>, Diagnostic> {
    nodes
        .iter()
        .map(|node| match node {
            Node::Element(element) => {
                typed_element(element, source_id, custom_widgets).map(TypedNode::Element)
            }
            Node::Text(text) => Ok(TypedNode::Text(ir_span(source_id, text.span))),
            Node::Interpolation(expression) => Ok(TypedNode::Interpolation(ir_span(
                source_id,
                expression.span,
            ))),
            // 成员块在声明解析阶段剥离；出现即代表内部装配错误。
            Node::WidgetMember(_) => Err(Diagnostic::new(
                SourceSpan {
                    start: 0,
                    end: 0,
                    line: 1,
                    column: 1,
                },
                "成员声明块不是可投影节点",
                "成员块只能作为 <Widget> 直接声明，由声明解析器剥离",
            )),
        })
        .collect()
}

fn typed_element(
    element: &Element,
    source_id: SourceId,
    custom_widgets: &BTreeSet<String>,
) -> Result<TypedElement, Diagnostic> {
    let kind = if let Some(component) = crate::lang::compiler::components::declaration(&element.name) {
        TypedElementKind::LibraryComponent { unit: component.unit }
    } else if let Some(component) = UI_PROJECTION_SCHEMA.component(&element.name) {
        TypedElementKind::Builtin {
            component_id: component.id,
            category: component.category,
        }
    } else if custom_widgets.contains(&element.name) {
        TypedElementKind::CustomWidget
    } else if is_control_element(&element.name) {
        TypedElementKind::Control
    } else {
        return Err(Diagnostic::new(
            element.span,
            format!("未登记的 UIX 标签 {}", element.name),
            format!(
                "使用自定义 Widget 或已登记组件：{}",
                UI_PROJECTION_SCHEMA.supported_component_hint()
            ),
        ));
    };
    let attributes = element
        .attributes
        .iter()
        .map(|attribute| typed_attribute(attribute, source_id, element.control.as_ref()))
        .collect();
    Ok(TypedElement {
        name: element.name.clone(),
        kind,
        attributes,
        children: typed_nodes(&element.children, source_id, custom_widgets)?,
        span: ir_span(source_id, element.span),
    })
}

fn typed_attribute(
    attribute: &Attribute,
    source_id: SourceId,
    control: Option<&ControlBinding>,
) -> TypedAttribute {
    let role = if attribute.name.starts_with('@') {
        TypedAttributeRole::Event
    } else if matches!(
        attribute.name.as_str(),
        "style" | "class" | "animation" | "transition"
    ) {
        TypedAttributeRole::Style
    } else if control.is_some()
        && matches!(
            attribute.name.as_str(),
            "condition" | "each" | "item" | "index" | "key"
        )
    {
        TypedAttributeRole::Control
    } else {
        TypedAttributeRole::Property
    };
    let value_kind = match &attribute.value {
        AttributeValue::Literal(_) => TypedValueKind::Literal,
        AttributeValue::Expression(_) => TypedValueKind::Expression,
        AttributeValue::InlineStyle(_) => TypedValueKind::InlineStyle,
    };
    TypedAttribute {
        name: attribute.name.clone(),
        role,
        value_kind,
        span: ir_span(source_id, attribute.span),
    }
}

fn is_control_element(name: &str) -> bool {
    matches!(
        name,
        "If" | "ElseIf" | "Else" | "For" | "Slot" | "KernelView" | "KernelHost" | "KernelChildren"
    )
}

fn ir_span(source_id: SourceId, span: SourceSpan) -> IrSpan {
    IrSpan {
        source_id,
        start: span.start,
        end: span.end,
        line: span.line,
        column: span.column,
    }
}

#[cfg(test)]
#[path = "../tests-src/semantic_ir_tests.rs"]
mod tests;
