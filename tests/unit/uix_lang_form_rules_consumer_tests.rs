// 引入生成代码承诺调用的公开 prelude。
use crate::prelude::*;

// 定义真实消费者使用的类型化表单模型。
#[derive(Clone)]
struct RulesProfile {
    // 保存邮箱字段。
    email: String,
}

// 定义符合全模型规则签名的 Rust 校验函数。
fn validate_profile(model: &RulesProfile) -> Result<(), String> {
    // 拒绝空邮箱并返回用户可见错误。
    (!model.email.is_empty())
        .then_some(())
        .ok_or_else(|| "邮箱不能为空".to_string())
}

// 定义类型化提交回调。
fn submit_profile(_model: RulesProfile) -> Result<(), String> {
    // 消费者示例始终允许提交。
    Ok(())
}

// 验证 Form rules 生成物只依赖公开 UIX 运行时契约。
#[test]
fn form_model_rules_compile_against_public_uix_api() {
    // 创建调用方拥有的业务模型状态。
    let profile = State::new(RulesProfile {
        // 提供稳定初始邮箱。
        email: "owner@example.com".to_string(),
    });
    // 展开带全模型规则、字段规则与类型化提交回调的真实文档。
    let _view: ViewNode = uix!(
        r#"<Form model={profile} rules={[validate_profile]} @submit="submit_profile($event)"><FormInputItem field="email" label="邮箱" rules="required,email" /><Button @click="submitForm">提交</Button></Form>"#
    );
}
