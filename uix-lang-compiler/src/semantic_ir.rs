//! UIX Lang 完成名称分类后的可查询语义 IR。

use std::collections::BTreeSet;

use crate::CompileTarget;
use crate::projection_schema::{ComponentCategory, UI_PROJECTION_SCHEMA};
use crate::source_graph::SourceId;
use crate::uix_lang::{
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
    Widget,
    Record,
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

/// 保存 Compiler System 语义阶段的确定性结果。
#[derive(Debug, Clone)]
pub struct TypedUiIr {
    target: CompileTarget,
    declarations: Vec<TypedDeclaration>,
    root: TypedElement,
    document: Document,
}

impl TypedUiIr {
    /// 返回本次分析的入口形状。
    pub const fn target(&self) -> CompileTarget {
        self.target
    }

    /// 返回源码顺序中的具名声明。
    pub fn declarations(&self) -> &[TypedDeclaration] {
        &self.declarations
    }

    /// 返回唯一根元素。
    pub const fn root(&self) -> &TypedElement {
        &self.root
    }

    // 只允许 Rust Emitter 消费已经进入 IR 的原始结构事实。
    pub(crate) const fn document(&self) -> &Document {
        &self.document
    }
}

/// 把完成导入合并的 AST 降低为带稳定来源的语义 IR。
pub(crate) fn lower_document(
    document: Document,
    target: CompileTarget,
    root_source: SourceId,
    declaration_sources: &[SourceId],
) -> Result<TypedUiIr, Diagnostic> {
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
            typed_declaration(
                declaration,
                declaration_sources
                    .get(index)
                    .copied()
                    .unwrap_or(root_source),
                &custom_widgets,
            )
            .transpose()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let root = typed_element(&document.root, root_source, &custom_widgets)?;
    Ok(TypedUiIr {
        target,
        declarations,
        root,
        document,
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
        })
        .collect()
}

fn typed_element(
    element: &Element,
    source_id: SourceId,
    custom_widgets: &BTreeSet<String>,
) -> Result<TypedElement, Diagnostic> {
    let kind = if let Some(component) = UI_PROJECTION_SCHEMA.component(&element.name) {
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
    let role = if attribute.name.starts_with("on") {
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
    matches!(name, "If" | "ElseIf" | "Else" | "For" | "Slot")
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
mod tests {
    use super::{TypedAttributeRole, TypedDeclarationKind, TypedElementKind, lower_document};
    use crate::CompileTarget;
    use crate::source_graph::SourceId;
    use crate::uix_lang::parse_document;

    #[test]
    fn semantic_ir_classifies_components_custom_widgets_and_attribute_roles() {
        let document = parse_document(
            "<Widget name=\"Greeting\"><Text onClick={save} style=\"color: red;\">Hi</Text></Widget><Greeting />",
        )
        .expect("测试源码必须通过解析");
        let source = SourceId::from_source_name("demo.uix");
        let ir = lower_document(document, CompileTarget::View, source, &[source])
            .expect("语义 IR 必须建立");
        assert!(matches!(ir.root().kind, TypedElementKind::CustomWidget));
        assert_eq!(ir.declarations()[0].kind, TypedDeclarationKind::Widget);
        let super::TypedNode::Element(text) = &ir.declarations()[0].body[0] else {
            panic!("组件体必须包含 Text 元素");
        };
        assert!(matches!(text.kind, TypedElementKind::Builtin { .. }));
        assert_eq!(text.attributes[0].role, TypedAttributeRole::Event);
        assert_eq!(text.attributes[1].role, TypedAttributeRole::Style);
    }
}
