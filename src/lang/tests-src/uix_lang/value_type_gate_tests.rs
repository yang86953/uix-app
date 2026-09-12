//! `uix-lang-compiler/uix_lang/value_type_gate.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::{validate_value_type_capabilities, value_type_capability};
use crate::lang::compiler::uix_lang::widget_parser::registered_value_type;
use crate::lang::compiler::uix_lang::{Document, WidgetValueType, parse_document};

// 构造仅包含单个 Widget 的最小文档并取回解析结果。
fn widget_document(state_source: &str) -> Document {
    let source = format!(
        "<Widget name=\"Gate\" state=\"{state_source}\"><Text>t</Text></Widget><Gate />"
    );
    parse_document(&source).expect("门禁前置解析必须成功")
}

// 核心通用类型永不要求 capability。
#[test]
fn core_value_types_never_require_capability() {
    assert_eq!(value_type_capability(&WidgetValueType::String), None);
    assert_eq!(value_type_capability(&WidgetValueType::VecOfString), None);
    assert_eq!(
        value_type_capability(&WidgetValueType::OptionalString),
        None
    );
}

// 组件专属类型映射到拥有它的组件 capability。
#[test]
fn component_payload_types_map_to_owner_capability() {
    assert_eq!(
        value_type_capability(&WidgetValueType::Library("CascaderValue".into())),
        Some("tree-widgets")
    );
    assert_eq!(
        value_type_capability(&WidgetValueType::Library("Vec<UploadFile>".into())),
        Some("form-pattern")
    );
}

// 未启用 capability 时 typed state 给出定向诊断。
#[test]
fn disabled_payload_type_is_rejected_with_directed_diagnostic() {
    let document = widget_document("path: CascaderValue = []");
    let error =
        validate_value_type_capabilities(&document, |_| false).expect_err("未启用门禁必须失败");
    assert!(error.message.contains("CascaderValue"));
    assert!(error.message.contains("tree-widgets"));
}

// 启用对应 capability 后同一文档通过校验。
#[test]
fn enabled_payload_type_passes_gate() {
    let document = widget_document("queue: Vec<UploadFile> = []");
    let enabled = |name: &str| name == "form-pattern";
    validate_value_type_capabilities(&document, enabled).expect("已启用门禁应通过");
}

// record 字段走同一门禁路径。
#[test]
fn record_fields_share_the_same_gate() {
    let document = parse_document(
        "<Record name=\"Path\" fields=\"value: CascaderValue\" /><App><Text>t</Text></App>",
    )
    .expect("record 文档必须可解析");
    let error =
        validate_value_type_capabilities(&document, |_| false).expect_err("门禁必须失败");
    assert!(error.message.contains("record 字段"));
    // 核心类型在关闭状态下依旧通过，证明门禁只约束组件专属载荷。
    let core_document = parse_document(
        "<Record name=\"User\" fields=\"age: number\" /><App><Text>t</Text></App>",
    )
    .expect("核心 record 必须可解析");
    validate_value_type_capabilities(&core_document, |_| false)
        .expect("核心通用类型不应被门禁拒绝");
}

// 反向映射闭包：schema 登记的组件专属类型必须能还原为对应变体。
#[test]
fn schema_registry_round_trips_payload_variants() {
    for spec in crate::lang::compiler::projection_schema::UI_PROJECTION_SCHEMA.value_types() {
        if spec.capability.is_some() {
            let variant = registered_value_type(spec.name);
            assert!(variant.is_some(), "{} 登记缺少 lowering 映射", spec.name);
            assert_eq!(
                value_type_capability(variant.as_ref().expect("已检查")),
                spec.capability
            );
        }
    }
}
