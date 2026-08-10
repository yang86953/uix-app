// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 解析完整文档。
    let document = parse_document(source)?;
    // 生成公开 View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Form 首批完整生成契约。
#[test]
fn generates_typed_form_contract() {
    // 生成模型、字段、规则与提交按钮。
    let snapshot = generate(r#"<Form model={profile} @submit="on_submit($event)" width="320px"><FormInputItem field="email" label="邮箱" rules="required,email" /><Button type="primary" @click="submitForm">提交</Button></Form>"#).expect("Form 应生成");
    // 核对模型、成员投影与规则。
    assert!(snapshot.contains("Form :: model (& (profile))"));
    assert!(snapshot.contains("& mut __uix_form_model . email"));
    assert!(snapshot.contains("label (\"邮箱\")"));
    assert!(snapshot.contains("required (true)") && snapshot.contains("email (true)"));
    // 核对类型化回调、触发器和公共属性。
    assert!(snapshot.contains("on_submit") && snapshot.contains("__uix_submitted_model"));
    assert!(snapshot.contains("__uix_submit_form . submit_typed"));
    assert!(snapshot.contains("width (320.0)"));
}

// 验证缺失模型和错误提交入口。
#[test]
fn rejects_incomplete_form_contracts() {
    // 缺失模型必须失败。
    let missing = generate(r#"<Form @submit="save"><FormInputItem field="name" /><Button @click="submitForm">提交</Button></Form>"#).expect_err("缺少 model");
    // 诊断必须包含 model。
    assert!(missing.message.contains("model"));
    // 普通处理器不能冒充 submitForm。
    let wrong = generate(r#"<Form model={profile} @submit="save"><FormInputItem field="name" /><Button @click="save">提交</Button></Form>"#).expect_err("错误提交按钮");
    // 诊断必须指出 submitForm 结构。
    assert!(wrong.suggestion.contains("submitForm"));
}

// 验证规则与父子边界。
#[test]
fn rejects_invalid_form_field_shapes() {
    // 未登记规则必须失败。
    let rule = generate(r#"<Form model={profile} @submit="save"><FormInputItem field="name" rules="required,length" /><Button @click="submitForm">提交</Button></Form>"#).expect_err("未知规则");
    // 诊断必须包含具体规则。
    assert!(rule.message.contains("length"));
    // 孤立字段项必须失败。
    let orphan = generate(r#"<FormInputItem field="name" />"#).expect_err("孤立字段");
    // 诊断必须说明直接子项边界。
    assert!(orphan.message.contains("直接子项"));
}
