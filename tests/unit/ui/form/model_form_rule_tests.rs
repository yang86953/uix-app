// 引入表单 Module 的公开类型。
use super::*;
// 引入类型化业务模型使用的响应式状态。
use crate::ui::State;
// 引入断言提交回调顺序所需的可变共享状态。
use std::cell::{Cell, RefCell};
// 引入规则与回调共享所有权。
use std::rc::Rc;

// 定义测试使用的双字段业务模型。
#[derive(Clone, Debug, PartialEq, Eq)]
struct PasswordForm {
    // 保存密码字段。
    password: String,
    // 保存确认密码字段。
    confirmation: String,
}

// 构建已经绑定真实字段 View 的测试表单。
fn bound_form(
    // 接收业务模型状态。
    model: &State<PasswordForm>,
    // 接收全模型规则。
    rule: impl Fn(&PasswordForm) -> Result<(), String> + 'static,
) -> ModelForm<PasswordForm> {
    // 构建两个类型化文本字段。
    let form = Form::model(model)
        // 投影密码字段。
        .field(
            "password",
            |value| &mut value.password,
            FormInputItem::new("password"),
        )
        // 投影确认密码字段。
        .field(
            "confirmation",
            |value| &mut value.confirmation,
            FormInputItem::new("confirmation"),
        )
        // 登记整表规则。
        .model_rule(rule)
        // 完成表单构建。
        .build();
    // 物化字段 View 以建立受控字段绑定。
    let _view = form.view();
    // 返回可提交表单句柄。
    form
}

// 验证全模型规则失败不会写回状态或调用提交回调。
#[test]
fn model_rule_failure_keeps_model_and_skips_submit_callback() {
    // 创建确认密码不一致的原始模型。
    let initial = PasswordForm {
        // 设置主密码。
        password: "secret".to_string(),
        // 设置不一致确认值。
        confirmation: "different".to_string(),
    };
    // 保存可核对原子性的模型状态。
    let model = State::new(initial.clone());
    // 保存提交回调是否执行的事实。
    let submitted = Rc::new(Cell::new(false));
    // 为回调克隆事实句柄。
    let submitted_for_callback = submitted.clone();
    // 构建带全模型规则与提交回调的表单。
    let form = Form::model(&model)
        // 投影密码字段。
        .field(
            "password",
            |value| &mut value.password,
            FormInputItem::new("password"),
        )
        // 投影确认密码字段。
        .field(
            "confirmation",
            |value| &mut value.confirmation,
            FormInputItem::new("confirmation"),
        )
        // 拒绝两个字段不一致的候选模型。
        .model_rule(|value| {
            // 返回稳定用户可见错误。
            (value.password == value.confirmation)
                .then_some(())
                .ok_or_else(|| "两次密码不一致".to_string())
        })
        // 登记成功提交回调。
        .on_submit_typed(move |_| {
            // 记录回调执行事实。
            submitted_for_callback.set(true);
            // 返回成功结果。
            Ok(())
        })
        // 完成表单构建。
        .build();
    // 物化字段 View 以建立受控字段绑定。
    let _view = form.view();
    // 全模型规则必须拒绝提交。
    let error = form.submit_typed().expect_err("不一致密码必须失败");
    // 返回首个规则错误文本。
    assert_eq!(error, "两次密码不一致");
    // 失败不得覆盖原始模型状态。
    assert_eq!(model.get(), initial);
    // 失败不得调用类型化提交回调。
    assert!(!submitted.get());
}

// 验证全模型规则按声明顺序先于提交回调执行。
#[test]
fn model_rules_run_in_declaration_order_before_submit_callback() {
    // 创建通过规则的业务模型。
    let model = State::new(PasswordForm {
        // 设置主密码。
        password: "secret".to_string(),
        // 设置一致确认值。
        confirmation: "secret".to_string(),
    });
    // 保存运行时执行顺序。
    let order = Rc::new(RefCell::new(Vec::new()));
    // 为第一条规则克隆顺序句柄。
    let first_order = order.clone();
    // 构建并绑定第一条规则。
    let form = bound_form(&model, move |_| {
        // 记录第一条规则。
        first_order.borrow_mut().push("rule-1");
        // 允许继续提交。
        Ok(())
    });
    // 当前帮助构造器已完成，另建完整链验证多规则与回调顺序。
    let second_order = order.clone();
    // 给已构建句柄无法追加规则，因此先验证单规则提交成功。
    let submitted = form.submit().expect("首条规则应通过");
    // 候选模型必须保持完整值。
    assert_eq!(submitted.password, submitted.confirmation);
    // 再构建带两条规则与提交回调的完整表单。
    let callback_order = order.clone();
    // 创建第二个模型句柄。
    let second_model = State::new(submitted);
    // 构建字段与有序规则。
    let second_form = Form::model(&second_model)
        // 投影密码字段。
        .field(
            "password",
            |value| &mut value.password,
            FormInputItem::new("password"),
        )
        // 投影确认密码字段。
        .field(
            "confirmation",
            |value| &mut value.confirmation,
            FormInputItem::new("confirmation"),
        )
        // 登记第二阶段第一条规则。
        .model_rule(move |_| {
            // 记录规则执行。
            second_order.borrow_mut().push("rule-2");
            // 允许继续执行。
            Ok(())
        })
        // 登记第二阶段第二条规则。
        .model_rule({
            // 为闭包克隆顺序句柄。
            let order = order.clone();
            // 返回记录规则的闭包。
            move |_| {
                // 记录第二条规则。
                order.borrow_mut().push("rule-3");
                // 允许提交回调执行。
                Ok(())
            }
        })
        // 登记最终提交回调。
        .on_submit_typed(move |_| {
            // 记录回调晚于全部规则。
            callback_order.borrow_mut().push("submit");
            // 返回成功状态。
            Ok(())
        })
        // 完成表单构建。
        .build();
    // 物化第二个表单字段绑定。
    let _view = second_form.view();
    // 规则与回调全部通过。
    second_form.submit_typed().expect("规则与回调应通过");
    // 核对全部执行顺序。
    assert_eq!(
        order.borrow().as_slice(),
        &["rule-1", "rule-2", "rule-3", "submit"]
    );
}
