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

// 验证 FormSelectItem 的类型化选择字段生成契约。
#[test]
fn generates_typed_form_select_item_contract() {
    // 生成候选集合、标签、规则与交互配置。
    let snapshot = generate(r#"<Form model={profile} @submit="on_submit"><FormSelectItem field="level" label="等级" options={level_options} rules="required" searchable placeholder="请选择" disabled={locked} /><Button @click="submitForm">提交</Button></Form>"#).expect("FormSelectItem 应生成");
    // 核对类型化成员投影与公开字段组件。
    assert!(snapshot.contains("& mut __uix_form_model . level"));
    // 核对候选集合和独立标签。
    assert!(snapshot.contains("FormSelectItem :: new (\"level\")"));
    // 核对候选集合表达式。
    assert!(snapshot.contains("options (level_options)"));
    // 核对独立标签和必填规则。
    assert!(snapshot.contains("label (\"等级\")") && snapshot.contains("required (true)"));
    // 核对搜索分支、占位文本与动态禁用状态。
    assert!(snapshot.contains("searchable ()"));
    // 核对剩余选择字段配置。
    assert!(
        snapshot.contains("placeholder (\"请选择\")") && snapshot.contains("disabled (locked)")
    );
}

// 验证 FormCheckboxItem 的类型化布尔字段生成契约。
#[test]
fn generates_typed_form_checkbox_item_contract() {
    // 生成独立表单标签、控件文字、必选规则与禁用状态。
    let snapshot = generate(r#"<Form model={profile} @submit="on_submit"><FormCheckboxItem field="accepted" label="协议确认" text="我已阅读并同意" rules="required" disabled={locked} /><Button @click="submitForm">提交</Button></Form>"#).expect("FormCheckboxItem 应生成");
    // 核对 bool 成员类型化投影。
    assert!(snapshot.contains("& mut __uix_form_model . accepted"));
    // 核对公开复选字段构造器。
    assert!(snapshot.contains("FormCheckboxItem :: new (\"accepted\")"));
    // 核对独立 FormItem 标签。
    assert!(snapshot.contains("field_label (\"协议确认\")"));
    // 核对复选框自身说明文本。
    assert!(snapshot.contains("label (\"我已阅读并同意\")"));
    // 核对必选布尔规则和动态禁用状态。
    assert!(snapshot.contains("required (true)") && snapshot.contains("disabled (locked)"));
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
    // 选择字段缺少候选集合必须失败。
    let missing_options = generate(r#"<Form model={profile} @submit="save"><FormSelectItem field="level" /><Button @click="submitForm">提交</Button></Form>"#).expect_err("缺少 options");
    // 诊断必须指出缺少 options。
    assert!(missing_options.message.contains("options"));
    // 选择字段不得继承文本专用邮箱规则。
    let select_rule = generate(r#"<Form model={profile} @submit="save"><FormSelectItem field="level" options={levels} rules="email" /><Button @click="submitForm">提交</Button></Form>"#).expect_err("选择字段未知规则");
    // 诊断必须包含具体非法规则。
    assert!(select_rule.message.contains("email"));
    // 孤立选择字段项必须失败。
    let select_orphan =
        generate(r#"<FormSelectItem field="level" options={levels} />"#).expect_err("孤立选择字段");
    // 诊断必须说明直接子项边界。
    assert!(select_orphan.message.contains("直接子项"));
    // 复选字段不得继承文本专用邮箱规则。
    let checkbox_rule = generate(r#"<Form model={profile} @submit="save"><FormCheckboxItem field="accepted" rules="email" /><Button @click="submitForm">提交</Button></Form>"#).expect_err("复选字段未知规则");
    // 诊断必须包含具体非法规则。
    assert!(checkbox_rule.message.contains("email"));
    // 孤立复选字段项必须失败。
    let checkbox_orphan = generate(r#"<FormCheckboxItem field="accepted" />"#)
        // 获取越界字段诊断。
        .expect_err("孤立复选字段");
    // 诊断必须说明直接子项边界。
    assert!(checkbox_orphan.message.contains("直接子项"));
}
