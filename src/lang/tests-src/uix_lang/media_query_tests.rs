// 引入核心 View 生成与文档解析入口。
use super::{generate_document_view, parse_document};

// 解析并生成完整文档令牌文本。
fn tokens_for(source: &str) -> Result<String, super::Diagnostic> {
    let document = parse_document(source)?;
    generate_document_view(&document).map(|tokens| tokens.to_string())
}

// 验证 @media 条件层生成构建期宽度判定，px 与 screen token 阈值都保留。
#[test]
fn media_layers_generate_conditional_field_updates() {
    let tokens = tokens_for(
        r#"
        card { padding: 16px; height: 20px; }
        @media (min-width: 800px) and (max-width: #screenXL) { card { padding: 24px; } }
        <Container class="card" />
        "#,
    )
    .expect("@media 语法应合法");
    assert!(tokens.contains("uix_media_matches"), "{tokens}");
    assert!(tokens.contains("Some (800"), "px 阈值应保留：{tokens}");
    assert!(
        tokens.contains("screen_xl ()"),
        "screen token 应在构建期读取：{tokens}"
    );
    // 条件层内同时更新字段并登记声明存在性。
    assert!(
        tokens.contains("if :: uix_app :: ui :: __private :: uix_media_matches"),
        "{tokens}"
    );
}

// 验证同名字段的基础、条件与内联声明按 类 → @media（源码顺序）→ 内联 生成。
#[test]
fn media_layers_sit_between_classes_and_inline_in_source_order() {
    let tokens = tokens_for(
        r#"
        card { padding: 16px; }
        wide { gap: 4px; }
        @media (min-width: 800px) { card { padding: 24px; } }
        @media (max-width: 1279px) { wide { padding: 28px; } card { gap: 6px; } }
        <Container class="card wide" style="opacity: 0.5;" />
        "#,
    )
    .expect("多层声明应合法");
    let position = |needle: &str| {
        tokens
            .find(needle)
            .unwrap_or_else(|| panic!("缺少 {needle}"))
    };
    let base_card = position("EdgeInsets :: new (16");
    let base_wide = position("gap = 4");
    let media_first = position("EdgeInsets :: new (24");
    // 第二个块内按元素 class 链顺序：card 的 gap 覆盖先于 wide 的 padding 覆盖。
    let media_second_card = position("gap = 6");
    let media_second_wide = position("EdgeInsets :: new (28");
    let inline = position("opacity = 0.5");
    assert!(base_card < base_wide, "基础类按左到右顺序");
    assert!(base_wide < media_first, "条件层位于全部基础类之后");
    assert!(media_first < media_second_card, "条件层按源码顺序叠加");
    assert!(
        media_second_card < media_second_wide,
        "块内按元素 class 链顺序"
    );
    assert!(media_second_wide < inline, "内联最后覆盖");
}

// 验证 @media 覆盖沿 extends 链匹配派生类的使用者。
#[test]
fn media_override_of_base_class_reaches_derived_class_users() {
    let tokens = tokens_for(
        r#"
        base { padding: 16px; }
        derived { extends: base; gap: 4px; }
        @media (min-width: 800px) { base { padding: 24px; } }
        <Container class="derived" />
        "#,
    )
    .expect("继承链上的媒体覆盖应合法");
    assert!(tokens.contains("EdgeInsets :: new (24"), "{tokens}");
}

// 验证语义模型把 @media 块登记为独立声明并给出规范条件文本。
#[test]
fn semantic_model_lists_media_blocks() {
    let source = r#"
        card { padding: 16px; }
        @media (min-width: 800px) and (max-width: #screenXL) { card { padding: 24px; } }
        <Container class="card" />
    "#;
    let document = parse_document(source).expect("@media 语法应合法");
    let source_id = crate::source_graph::SourceId::from_source_name("media.uix");
    let model = crate::semantic_ir::lower_document(
        document,
        crate::CompileTarget::View,
        source_id,
        &[source_id],
    )
    .expect("语义模型应接受 @media");
    let media = model
        .declarations()
        .iter()
        .find(|declaration| declaration.kind == crate::semantic_ir::TypedDeclarationKind::Media)
        .expect("应登记 @media 声明");
    assert_eq!(
        media.name,
        "@media (min-width: 800px) and (max-width: #screenXL)"
    );
}

// 验证不支持的条件、阈值与块内容都给出确定诊断。
#[test]
fn rejects_unsupported_media_syntax() {
    for (source, expect) in [
        (
            "card { padding: 16px; } @media (min-width: 800px) { missing { padding: 1px; } } <Container class=\"card\" />",
            "未声明的样式类",
        ),
        (
            "card { padding: 16px; } @media (orientation: landscape) { card { padding: 1px; } } <Container class=\"card\" />",
            "暂不支持 orientation",
        ),
        (
            "card { padding: 16px; } @media (min-width: 800px) or (max-width: 1px) { card { padding: 1px; } } <Container class=\"card\" />",
            "只能用 and",
        ),
        (
            "card { padding: 16px; } @media (min-width: 50%) { card { padding: 1px; } } <Container class=\"card\" />",
            "必须是 px 长度或 screen token",
        ),
        (
            "card { padding: 16px; } @media (min-width: #padding) { card { padding: 1px; } } <Container class=\"card\" />",
            "已登记的 screen token",
        ),
        (
            "card { padding: 16px; } @media (min-width: 800px) and (min-width: 900px) { card { padding: 1px; } } <Container class=\"card\" />",
            "重复声明 min-width",
        ),
        (
            "card { padding: 16px; } @media (min-width: -1px) { card { padding: 1px; } } <Container class=\"card\" />",
            "非负有限 px",
        ),
        (
            "card { padding: 16px; } @media (min-width: 800px) { card:hover { padding: 1px; } } <Container class=\"card\" />",
            "暂不支持状态伪类",
        ),
        (
            "card { padding: 16px; } @media (min-width: 800px) { card { position: absolute; } } <Container class=\"card\" />",
            "暂不支持 position",
        ),
        (
            "card { padding: 16px; } @media (min-width: 800px) { card { transition: opacity 0.2s; } } <Container class=\"card\" />",
            "暂不支持 transition",
        ),
        (
            "card { padding: 16px; } @media (min-width: 800px) { } <Container class=\"card\" />",
            "至少需要一个类覆盖",
        ),
    ] {
        let error = tokens_for(source).expect_err(&format!("{source} 必须失败"));
        assert!(
            error.message.contains(expect),
            "{source}\n诊断应包含 {expect:?}，实际 {:?}",
            error.message
        );
    }
}
