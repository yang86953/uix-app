//! `uix-lang-compiler/semantic_ir.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::{TypedAttributeRole, TypedDeclarationKind, TypedElementKind, lower_document};
use crate::lang::compiler::CompileTarget;
use crate::lang::compiler::source_graph::SourceId;
use crate::lang::compiler::uix_lang::parse_document;
use std::sync::Arc;

#[test]
fn clone_shares_private_immutable_ir_storage() {
    let document =
        parse_document("<Widget name=\"Greeting\"><Text>Hello</Text></Widget><Greeting />")
            .expect("测试源码必须通过解析");
    let source = SourceId::from_source_name("clone.uix");
    let ir = lower_document(document, CompileTarget::View, source, &[source])
        .expect("语义 IR 必须建立");
    let cloned = ir.clone();

    // 公开根与查询语义保持拥有型值，私有不可变声明及 AST 共享同一存储。
    assert_eq!(ir.root, cloned.root);
    assert!(Arc::ptr_eq(&ir.declarations, &cloned.declarations));
    assert!(Arc::ptr_eq(&ir.document, &cloned.document));
}

#[test]
fn semantic_ir_classifies_components_custom_widgets_and_attribute_roles() {
    let document = parse_document(
        "<Widget name=\"Greeting\"><Text @click=\"save\" style=\"color: red;\">Hi</Text></Widget><Greeting />",
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
    let click = ir
        .semantic_node_at(source, text.attributes[0].span.start)
        .expect("事件属性必须可由位置查询");
    assert_eq!(click.name, "@click");
    assert_eq!(click.kind, super::SemanticNodeKind::Attribute);
}

#[test]
fn semantic_ir_classifies_kernel_view_as_internal_control() {
    // KernelView 是框架模板边界，不进入公开组件 schema。
    let document = parse_document("<KernelView value={build_kernel()} />")
        .expect("基础 View 桥接源码必须通过解析");
    // 使用稳定来源身份建立完整语义 IR。
    let source = SourceId::from_source_name("kernel.uix");
    let ir = lower_document(document, CompileTarget::View, source, &[])
        .expect("KernelView 必须通过语义降低");
    // 该元素只能归入语言内部控制，不得伪装成公开内置组件。
    assert!(matches!(ir.root().kind, TypedElementKind::Control));

    // 带 UIX 展示子树的基础内核宿主使用同一内部控制边界。
    let host =
        parse_document("<KernelHost value={build_kernel}><Icon name=\"minus\" /></KernelHost>")
            .expect("基础内核宿主源码必须通过解析");
    // 使用独立语义降低确认不会进入公开 schema。
    let host = lower_document(host, CompileTarget::View, source, &[])
        .expect("KernelHost 必须通过语义降低");
    // 宿主和直接注入都属于框架内部控制。
    assert!(matches!(host.root().kind, TypedElementKind::Control));

    // 多节点列表桥接同样只是框架内部控制元素。
    let children = parse_document("<KernelChildren value={build_children()} />")
        .expect("基础节点列表桥接必须通过解析");
    // 语义降低不得把列表桥接暴露为公开组件。
    let children = lower_document(children, CompileTarget::View, source, &[])
        .expect("KernelChildren 必须通过语义降低");
    // 确认内部控制分类。
    assert!(matches!(children.root().kind, TypedElementKind::Control));
}

#[test]
fn semantic_ir_collects_component_and_attribute_capabilities() {
    let document = parse_document(
        "<Column><Avatar text=\"A\" /><Avatar src=\"a.png\" /><QRCode value=\"x\" /></Column>",
    )
    .expect("测试源码必须通过解析");
    let source = SourceId::from_source_name("capability.uix");
    let ir =
        lower_document(document, CompileTarget::View, source, &[]).expect("语义 IR 必须建立");
    let requirements = ir.capability_requirements();
    assert_eq!(requirements.len(), 2);
    assert_eq!(requirements[0].capability, "image-codecs");
    assert_eq!(requirements[0].attribute.as_deref(), Some("src"));
    assert_eq!(requirements[1].capability, "qrcode");
    assert!(requirements[1].attribute.is_none());
}
