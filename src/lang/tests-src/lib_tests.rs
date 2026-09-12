//! `uix-lang-compiler/lib.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use std::path::Path;

use super::{
    CompileTarget, CompilerSystem, DiagnosticPhase, QueryEntry, QueryKind, check_inline,
    compile_inline, format_inline,
};
use crate::source_graph::SourceId;

#[test]
fn shared_commands_compile_all_public_target_shapes() {
    let view = compile_inline("<Text>Hello</Text>", "<view>", CompileTarget::View)
        .expect("View 编译应成功");
    assert!(view.tokens.to_string().contains("Label :: new"));
    assert!(!view.tokens.to_string().contains("__uix_source_marker"));
    assert!(view.tracked_files.is_empty());
    assert!(!view.source_map.entries().is_empty());
    assert_eq!(
        view.source_map.entries()[0].source_id,
        view.source_graph.root()
    );
    assert_eq!(
        view.compilation_key.root_content_hash,
        view.source_graph
            .file(view.source_graph.root())
            .expect("根源码必须存在")
            .content_hash
    );

    let app = compile_inline("<App><Text>Hello</Text></App>", "<app>", CompileTarget::App)
        .expect("App 编译应成功");
    assert!(app.tokens.to_string().contains("App :: new"));

    let items = compile_inline(
        "<Record name=\"User\" fields=\"name: String\" /><Text>Hello</Text>",
        "<items>",
        CompileTarget::Items,
    )
    .expect("Record 编译应成功");
    assert!(items.tokens.to_string().contains("struct User"));
}

#[test]
fn items_target_accepts_declaration_only_resources_without_relaxing_views() {
    let source = r#"<Visual name="ONLY_VISUAL" type="OnlyVisual" value={1.0} />"#;
    let inline = compile_inline(source, "<visual-items>", CompileTarget::Items)
        .expect("Items 内嵌资源应允许只有模块级声明");
    assert!(inline.tokens.to_string().contains("ONLY_VISUAL"));

    let strict = compile_inline(source, "<visual-view>", CompileTarget::View)
        .expect_err("View 入口仍必须声明根元素");
    assert_eq!(strict.code, "UIX1000");
    assert!(strict.message.contains("缺少根元素"));

    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/uix_lang/items/visual_only.uix");
    let output = CompilerSystem::new()
        .compile_file(&file, CompileTarget::Items)
        .expect("Items 文件资源应允许只有模块级声明");
    assert!(output.tokens.to_string().contains("ONLY_VISUAL"));
    let strict_file = CompilerSystem::new()
        .compile_file(&file, CompileTarget::View)
        .expect_err("同一文件作为 View 时仍必须拒绝缺失根元素");
    assert_eq!(strict_file.code, "UIX1000");
}

#[test]
fn auto_check_selects_target_from_parsed_root() {
    let system = CompilerSystem::new();
    let view = system
        .check_inline_auto("<Text>Hello</Text>", "<view-auto>")
        .expect("普通根应自动选择 View");
    assert_eq!(view.ir.target(), CompileTarget::View);

    let app = system
        .check_inline_auto("<App title=\"Auto\"><Text>Hello</Text></App>", "<app-auto>")
        .expect("App 根应自动选择 App");
    assert_eq!(app.ir.target(), CompileTarget::App);
}

#[test]
fn source_map_preserves_recursive_import_sources() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/uix_lang/imports/root.uix");
    let output = CompilerSystem::new()
        .compile_file(&root, CompileTarget::View)
        .expect("递归导入闭包必须可编译");
    let sources = output
        .source_map
        .entries()
        .iter()
        .map(|entry| entry.source_name.as_str())
        .collect::<Vec<_>>();
    assert!(
        sources
            .iter()
            .any(|source| source.ends_with("pages/page.uix")),
        "SourceMap 必须保留直接导入来源：{sources:?}"
    );
    assert!(
        sources
            .iter()
            .any(|source| source.ends_with("shared/helper.uix")),
        "SourceMap 必须保留递归导入来源：{sources:?}"
    );
}

#[test]
fn shared_commands_preserve_structured_diagnostics() {
    let error = compile_inline("<Mystery />", "demo.uix", CompileTarget::View)
        .expect_err("未知标签必须失败");
    assert_eq!(error.source_name, "demo.uix");
    assert_eq!(error.source_id, SourceId::from_source_name("demo.uix"));
    assert_eq!(error.code, "UIX2000");
    assert_eq!(error.phase, DiagnosticPhase::Semantic);
    assert_eq!(error.line, 1);
    assert!(error.message.contains("Mystery"));
    assert!(!error.suggestion.is_empty());
}

#[test]
fn shared_commands_distinguish_syntax_and_import_diagnostics() {
    let syntax = compile_inline("<Text>", "syntax.uix", CompileTarget::View)
        .expect_err("未闭合标签必须失败");
    assert_eq!(syntax.code, "UIX1000");
    assert_eq!(syntax.phase, DiagnosticPhase::Syntax);

    let import = compile_inline(
        "@import('./card.uix', 'Card')\n<Text />",
        "import.uix",
        CompileTarget::View,
    )
    .expect_err("内嵌导入必须失败");
    assert_eq!(import.code, "UIX1100");
    assert_eq!(import.phase, DiagnosticPhase::Import);
}

#[test]
fn check_and_aot_share_semantic_diagnostic_contract() {
    let source = "<Mystery />";
    let aot = compile_inline(source, "same.uix", CompileTarget::View)
        .expect_err("AOT 必须拒绝未知标签");
    let check = check_inline(source, "same.uix", CompileTarget::View)
        .expect_err("check 必须拒绝未知标签");
    assert_eq!(aot, check);
}

#[test]
fn compiler_system_owns_schema_and_returns_typed_ir() {
    let system = CompilerSystem::new();
    assert_eq!(system.schema().version(), 3);
    let checked = system
        .check_inline("<Text>Hello</Text>", "typed.uix", CompileTarget::View)
        .expect("合法文档必须通过检查");
    assert_eq!(checked.ir.root().name, "Text");
    assert_eq!(
        checked.ir.root().span.source_id,
        checked.source_graph.root()
    );
}

#[test]
fn shared_formatter_returns_lossless_idempotent_result() {
    let first = format_inline("<Column><Text>A</Text></Column>", "fmt.uix")
        .expect("合法源码必须可格式化");
    assert!(first.changed);
    assert_eq!(first.cst.reconstruct(), first.formatted);
    let second =
        format_inline(&first.formatted, "fmt.uix").expect("格式化结果必须可再次格式化");
    assert!(!second.changed);
    assert_eq!(second.formatted, first.formatted);
}

#[test]
fn compiler_system_query_and_incremental_key_share_schema_versions() {
    let system = CompilerSystem::new();
    let queried = system.query(QueryKind::Components);
    assert_eq!(queried.kind.as_str(), "components");
    assert!(matches!(queried.entries[0], QueryEntry::Component(_)));

    let first = system
        .check_inline("<Text>A</Text>", "key.uix", CompileTarget::View)
        .expect("合法源码必须可检查");
    let changed = system
        .check_inline("<Text>B</Text>", "key.uix", CompileTarget::View)
        .expect("变更源码必须可检查");
    assert_ne!(first.compilation_key, changed.compilation_key);
    assert_eq!(
        first.compilation_key.schema_version,
        system.schema().version()
    );
    assert_eq!(
        first.compilation_key.language_version,
        system.schema().language_version()
    );
}

#[cfg(not(feature = "qrcode"))]
#[test]
fn missing_capability_is_a_shared_semantic_diagnostic() {
    let source = "<QRCode value=\"hello\" />";
    let aot = compile_inline(source, "capability.uix", CompileTarget::View)
        .expect_err("未启用 qrcode 时 AOT 必须拒绝组件");
    let check = check_inline(source, "capability.uix", CompileTarget::View)
        .expect_err("未启用 qrcode 时 check 必须拒绝组件");
    assert_eq!(aot, check);
    assert_eq!(aot.code, "UIX2001");
    assert!(aot.message.contains("qrcode"));
}

#[cfg(not(feature = "image-codecs"))]
#[test]
fn attribute_capability_does_not_reject_unrelated_component_shape() {
    check_inline("<Avatar text=\"UI\" />", "avatar.uix", CompileTarget::View)
        .expect("纯文字 Avatar 不依赖 image-codecs");
    let error = check_inline(
        "<Avatar src=\"avatar.png\" />",
        "avatar.uix",
        CompileTarget::View,
    )
    .expect_err("Avatar src 必须要求 image-codecs");
    assert_eq!(error.code, "UIX2001");
    assert!(error.message.contains("src"));
}

#[test]
fn overlay_imports_preserve_unsaved_content_and_source_identity() {
    let directory = std::env::temp_dir().join(format!(
        "uix-lang-overlay-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("系统时间必须有效")
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).expect("临时目录必须可创建");
    let root = directory.join("root.uix");
    let card = directory.join("card.uix");
    std::fs::write(&root, "@import('./card.uix', 'Card')\n<Card />").expect("根文件必须可写");
    std::fs::write(
        &card,
        "@export('Card')\n<Widget name=\"Card\"><Text>Disk</Text></Widget>\n<Text />",
    )
    .expect("导入文件必须可写");
    let overlays = std::collections::BTreeMap::from([(
        card.clone(),
        "@export('Card')\n<Widget name=\"Card\"><Mystery /></Widget>\n<Text />".to_string(),
    )]);
    let error = CompilerSystem::new()
        .check_file_with_overlays(&root, &overlays, CompileTarget::View)
        .expect_err("未保存的未知标签必须产生诊断");
    assert!(error.message.contains("Mystery"));
    assert_eq!(
        error.source_name,
        std::fs::canonicalize(&card)
            .expect("导入文件必须可规范化")
            .display()
            .to_string()
    );
    std::fs::remove_dir_all(&directory).expect("临时目录必须可清理");
}
