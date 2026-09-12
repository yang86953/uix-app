// 验证 Visual 声明的具名字段生成、拒绝路径与真实来源定位。

use std::collections::BTreeMap;

use crate::lang::compiler::source_graph::SourceId;

use super::{
    Declaration, VisualValue, generate_document_items, parse_document, with_source_markers,
};

#[test]
fn generates_named_visual_const_for_uix_items() {
    let document = parse_document(
        r#"
<Visual
  name="ICON_VISUAL"
  type="IconVisual"
  defaultSize={24.0}
  glyphScale={0.85}
  fallbackScale={0.55}
  color={icon_text_color()}
  statusColors={[success_color(), error_color()]}
/>
<Text>图标</Text>
"#,
    )
    .expect("合法 Visual 文档应成功解析");

    let tokens = generate_document_items(&document)
        .expect("Visual 项生成应成功")
        .to_string();

    assert!(
        tokens.contains("pub (crate) static ICON_VISUAL : IconVisual"),
        "{tokens}"
    );
    assert!(
        tokens.contains("pub (crate) const ICON_VISUAL_REF : & 'static IconVisual = & ICON_VISUAL"),
        "{tokens}"
    );
    assert!(tokens.contains("default_size : 24.0"), "{tokens}");
    assert!(tokens.contains("glyph_scale : 0.85"), "{tokens}");
    assert!(tokens.contains("fallback_scale : 0.55"), "{tokens}");
    assert!(tokens.contains("color : (icon_text_color) ()"), "{tokens}");
    assert!(
        tokens.contains("status_colors : [(success_color) () , (error_color) ()]"),
        "{tokens}"
    );
}

#[test]
fn rejects_invalid_or_ambiguous_visual_declarations() {
    let duplicate = parse_document(
        r#"<Visual name="ICON_VISUAL" type="IconVisual" defaultSize={24} default_size={20} /><Text>根</Text>"#,
    )
    .expect_err("同义字段不得映射为同一 Rust 字段");
    assert!(
        duplicate.message.contains("重复 Rust 字段"),
        "{}",
        duplicate.message
    );

    let invalid_name = parse_document(
        r#"<Visual name="iconVisual" type="IconVisual" size={24} /><Text>根</Text>"#,
    )
    .expect_err("Visual 名称必须是常量形状");
    assert!(
        invalid_name.message.contains("SCREAMING_SNAKE_CASE"),
        "{}",
        invalid_name.message
    );

    let reserved_name = parse_document(
        r#"<Visual name="ICON_VISUAL_REF" type="IconVisual" size={24} /><Text>根</Text>"#,
    )
    .expect_err("_REF 后缀必须留给编译器生成静态借用");
    assert!(reserved_name.suggestion.contains("_REF"));

    let nested = parse_document(
        r#"<Container><Visual name="ICON_VISUAL" type="IconVisual" size={24} /></Container>"#,
    )
    .expect_err("Visual 不得嵌套到视图树");
    assert!(nested.message.contains("顶层声明区"), "{}", nested.message);
}

#[test]
fn visual_codegen_error_keeps_named_source_context() {
    let mut document = parse_document(
        r#"<Visual name="BROKEN_VISUAL" type="BrokenVisual" field={1} /><Text>根</Text>"#,
    )
    .expect("初始 Visual 必须合法");
    let field_span = match document.declarations.first_mut() {
        Some(Declaration::Visual(visual)) => {
            let field = visual.fields.first_mut().expect("Visual 必须有字段");
            field.rust_name = "type".to_string();
            assert!(matches!(field.value, VisualValue::Expression(_)));
            field.span
        }
        _ => panic!("首个声明必须是 Visual"),
    };
    let root_source = SourceId::from_source_name("main.uix");
    let visual_source = SourceId::from_source_name("visuals.uix");
    let visual_sources = BTreeMap::from([("BROKEN_VISUAL".to_string(), visual_source)]);

    let error = with_source_markers(
        root_source,
        BTreeMap::new(),
        BTreeMap::new(),
        visual_sources,
        || generate_document_items(&document),
    )
    .expect_err("Rust 关键字字段必须被生成器拒绝");

    assert_eq!(error.source_id, Some(visual_source));
    assert_eq!(error.span, field_span);
}
