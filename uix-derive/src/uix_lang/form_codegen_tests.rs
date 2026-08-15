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
    let snapshot = generate(r#"<Form model={profile} rules={[validate_profile, validate_access]} @submit="on_submit($event)" width="320px"><FormInputItem field="email" label="邮箱" rules="required,email" /><Button type="primary" @click="submitForm">提交</Button></Form>"#).expect("Form 应生成");
    // 核对模型、成员投影与规则。
    assert!(snapshot.contains("Form :: model (& (profile))"));
    assert!(snapshot.contains("& mut __uix_form_model . email"));
    assert!(snapshot.contains("label (\"邮箱\")"));
    assert!(snapshot.contains("required (true)") && snapshot.contains("email (true)"));
    // 顶层规则必须按数组顺序生成公开 model_rule 调用。
    let profile_rule = snapshot
        .find("model_rule (validate_profile)")
        .expect("缺少首条全模型规则");
    // 定位第二条全模型规则。
    let access_rule = snapshot
        .find("model_rule (validate_access)")
        .expect("缺少第二条全模型规则");
    // 规则顺序必须与文档数组一致。
    assert!(profile_rule < access_rule);
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
    assert!(snapshot.contains("options ((level_options) . clone ())"));
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

// 验证 FormRadioItem 的类型化单选组生成契约。
#[test]
fn generates_typed_form_radio_item_contract() {
    // 生成候选集合、分组名、必填规则和动态布局配置。
    let snapshot = generate(r#"<Form model={profile} @submit="on_submit"><FormRadioItem field="channel" label="通知渠道" options={channel_options} rules="required" groupName="profile-channel" disabled={locked} vertical={stacked} /><Button @click="submitForm">提交</Button></Form>"#).expect("FormRadioItem 应生成");
    // 核对 String 成员类型化投影。
    assert!(snapshot.contains("& mut __uix_form_model . channel"));
    // 核对公开单选组字段构造器。
    assert!(snapshot.contains("FormRadioItem :: new (\"channel\")"));
    // 核对独立标签和候选集合表达式。
    assert!(snapshot.contains("label (\"通知渠道\")"));
    // 核对候选集合配置。
    assert!(snapshot.contains("options ((channel_options) . clone ())"));
    // 核对分组名和必填规则。
    assert!(snapshot.contains("group_name (\"profile-channel\")"));
    // 核对必填和动态禁用配置。
    assert!(snapshot.contains("required (true)") && snapshot.contains("disabled (locked)"));
    // 核对纵向布局使用同类型条件分支。
    assert!(snapshot.contains("if stacked") && snapshot.contains("vertical ()"));
}

// 验证 FormSwitchItem 的类型化开关生成契约。
#[test]
fn generates_typed_form_switch_item_contract() {
    // 生成独立表单标签、必须开启规则与动态禁用状态。
    let snapshot = generate(r#"<Form model={profile} @submit="on_submit"><FormSwitchItem field="notifications" label="启用通知" rules="required" disabled={locked} /><Button @click="submitForm">提交</Button></Form>"#).expect("FormSwitchItem 应生成");
    // 核对 bool 成员类型化投影。
    assert!(snapshot.contains("& mut __uix_form_model . notifications"));
    // 核对公开开关字段构造器。
    assert!(snapshot.contains("FormSwitchItem :: new (\"notifications\")"));
    // 核对独立 FormItem 标签。
    assert!(snapshot.contains("label (\"启用通知\")"));
    // 核对必须开启规则和动态禁用状态。
    assert!(snapshot.contains("required (true)") && snapshot.contains("disabled (locked)"));
}

// 验证 FormSliderItem 的类型化 f64 字段生成契约。
#[test]
fn generates_typed_form_slider_item_contract() {
    // 生成独立标签、静态范围与动态步长。
    let snapshot = generate(r#"<Form model={profile} @submit="on_submit"><FormSliderItem field="volume" label="音量" min="0" max="100" step={volume_step} /><Button @click="submitForm">提交</Button></Form>"#).expect("FormSliderItem 应生成");
    // 核对 f64 成员类型化投影。
    assert!(snapshot.contains("& mut __uix_form_model . volume"));
    // 核对公开滑块字段构造器。
    assert!(snapshot.contains("FormSliderItem :: new (\"volume\""));
    // 核对闭区间两个静态端点。
    assert!(snapshot.contains("0f64") && snapshot.contains("100f64"));
    // 核对独立标签与动态步长。
    assert!(snapshot.contains("label (\"音量\")") && snapshot.contains("step (volume_step)"));
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
    // 顶层 rules 字符串不得冒充函数数组。
    let literal_rules = generate(r#"<Form model={profile} rules="validate_profile" @submit="save"><FormInputItem field="name" /><Button @click="submitForm">提交</Button></Form>"#).expect_err("字符串顶层 rules 必须失败");
    // 诊断必须给出数组写法。
    assert!(literal_rules.suggestion.contains("rules={["));
    // 动态规则集合不能替代静态函数数组。
    let dynamic_rules = generate(r#"<Form model={profile} rules={form_rules} @submit="save"><FormInputItem field="name" /><Button @click="submitForm">提交</Button></Form>"#).expect_err("动态顶层 rules 必须失败");
    // 诊断必须要求数组字面量。
    assert!(dynamic_rules.message.contains("数组字面量"));
    // 数组项不得提前调用校验函数。
    let called_rule = generate(r#"<Form model={profile} rules={[validate_profile()]} @submit="save"><FormInputItem field="name" /><Button @click="submitForm">提交</Button></Form>"#).expect_err("调用形态顶层 rule 必须失败");
    // 建议必须要求传入函数名。
    assert!(called_rule.suggestion.contains("函数名"));
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
    // 单选组缺少候选集合必须失败。
    let radio_options = generate(r#"<Form model={profile} @submit="save"><FormRadioItem field="channel" /><Button @click="submitForm">提交</Button></Form>"#).expect_err("单选组缺少 options");
    // 诊断必须指出缺少 options。
    assert!(radio_options.message.contains("options"));
    // 单选组不得继承文本专用邮箱规则。
    let radio_rule = generate(r#"<Form model={profile} @submit="save"><FormRadioItem field="channel" options={channels} rules="email" /><Button @click="submitForm">提交</Button></Form>"#).expect_err("单选组未知规则");
    // 诊断必须包含具体非法规则。
    assert!(radio_rule.message.contains("email"));
    // 孤立单选组字段项必须失败。
    let radio_orphan = generate(r#"<FormRadioItem field="channel" options={channels} />"#)
        // 获取越界字段诊断。
        .expect_err("孤立单选组字段");
    // 诊断必须说明直接子项边界。
    assert!(radio_orphan.message.contains("直接子项"));
    // 开关字段不得继承文本专用邮箱规则。
    let switch_rule = generate(r#"<Form model={profile} @submit="save"><FormSwitchItem field="notifications" rules="email" /><Button @click="submitForm">提交</Button></Form>"#).expect_err("开关字段未知规则");
    // 诊断必须包含具体非法规则。
    assert!(switch_rule.message.contains("email"));
    // 开关字段不得接受原始 Switch 之外的说明文字属性。
    let switch_attribute = generate(r#"<Form model={profile} @submit="save"><FormSwitchItem field="notifications" text="通知" /><Button @click="submitForm">提交</Button></Form>"#).expect_err("开关字段未知属性");
    // 诊断必须包含具体非法属性。
    assert!(switch_attribute.message.contains("text"));
    // 开关字段必须保持叶节点形状。
    let switch_child = generate(r#"<Form model={profile} @submit="save"><FormSwitchItem field="notifications"><Text>非法</Text></FormSwitchItem><Button @click="submitForm">提交</Button></Form>"#).expect_err("开关字段子节点");
    // 诊断必须指出开关字段不接受子节点。
    assert!(switch_child.message.contains("不接受子节点"));
    // 孤立开关字段项必须失败。
    let switch_orphan = generate(r#"<FormSwitchItem field="notifications" />"#)
        // 获取越界字段诊断。
        .expect_err("孤立开关字段");
    // 诊断必须说明直接子项边界。
    assert!(switch_orphan.message.contains("直接子项"));
    // 反向滑块范围必须在编译期失败。
    let slider_range = generate(r#"<Form model={profile} @submit="save"><FormSliderItem field="volume" min="10" max="1" /><Button @click="submitForm">提交</Button></Form>"#).expect_err("滑块字段反向范围");
    // 诊断必须说明 min/max 顺序。
    assert!(slider_range.message.contains("不能大于"));
    // 非正滑块步长必须在编译期失败。
    let slider_step = generate(r#"<Form model={profile} @submit="save"><FormSliderItem field="volume" step="0" /><Button @click="submitForm">提交</Button></Form>"#).expect_err("滑块字段零步长");
    // 诊断必须说明正数约束。
    assert!(slider_step.message.contains("大于 0"));
    // 滑块字段不得继承无意义的必填规则。
    let slider_rule = generate(r#"<Form model={profile} @submit="save"><FormSliderItem field="volume" rules="required" /><Button @click="submitForm">提交</Button></Form>"#).expect_err("滑块字段未知规则");
    // 诊断必须点名未登记属性。
    assert!(slider_rule.message.contains("rules"));
    // 滑块字段必须保持叶节点形状。
    let slider_child = generate(r#"<Form model={profile} @submit="save"><FormSliderItem field="volume"><Text>非法</Text></FormSliderItem><Button @click="submitForm">提交</Button></Form>"#).expect_err("滑块字段子节点");
    // 诊断必须指出滑块字段不接受子节点。
    assert!(slider_child.message.contains("不接受子节点"));
    // 孤立滑块字段项必须失败。
    let slider_orphan = generate(r#"<FormSliderItem field="volume" />"#)
        // 获取越界字段诊断。
        .expect_err("孤立滑块字段");
    // 诊断必须说明直接子项边界。
    assert!(slider_orphan.message.contains("直接子项"));
}
