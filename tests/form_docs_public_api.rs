// 声明本文件只编译表单使用文档，不执行提交或原生资源操作。
#![allow(dead_code)]

// 隔离 form-basic 围栏中的声明式表单树。
mod form_basic {
    // 引入文档承诺的公开表单、输入与树宏 prelude。
    use uix::prelude::*;

    // 编译垂直布局下的用户名与密码字段声明。
    fn compile_example() {
        // 构造由 Form 拥有布局、FormItem 拥有字段契约的声明树。
        let _form = embed(tree! {
            // 创建带统一标签宽度和间距的垂直表单。
            Form::new()
                // 声明统一标签宽度。
                .label_width(86.0)
                // 声明字段间距。
                .gap(8.0)
                // 声明垂直字段布局。
                .layout(FormLayout::Vertical) => [
                    // 声明用户名字段与其唯一输入子节点。
                    tree! {
                        // 创建带稳定名称和必填帮助文本的 FormItem。
                        FormItem::new("用户名")
                            // 声明稳定字段 key。
                            .name("user")
                            // 声明必填规则。
                            .required(true)
                            // 声明字段帮助文本。
                            .help("必填") => [
                                // 创建用户名输入节点。
                                Input::new("请输入用户名").into_node(),
                            ]
                    },
                    // 声明密码字段与其唯一输入子节点。
                    tree! {
                        // 创建带稳定名称和长度提示的密码 FormItem。
                        FormItem::new("密码")
                            // 声明稳定字段 key。
                            .name("password")
                            // 声明必填规则。
                            .required(true)
                            // 声明密码长度帮助文本。
                            .help("至少 8 位") => [
                                // 创建密码输入节点。
                                Input::password().into_node(),
                            ]
                    },
                ]
        });
    }
}

// 隔离 form-model 围栏中的类型化业务模型与字段投影。
mod form_model {
    // 引入文档承诺的公开类型化表单与状态 prelude。
    use uix::prelude::*;

    // 派生类型化表单构建所需的克隆与默认值能力。
    #[derive(Clone, Default)]
    // 声明业务拥有的用户资料模型。
    struct ProfileForm {
        // 保存姓名字段。
        name: String,
        // 保存年龄字段。
        age: u8,
        // 保存等级字段。
        level: String,
        // 保存协议确认字段。
        accepted: bool,
        // 保存通知渠道字段。
        channel: String,
        // 保存通知开关字段。
        notifications: bool,
        // 保存音量字段。
        volume: f64,
    }

    // 校验字段之间的完整业务约束。
    fn validate_profile(profile: &ProfileForm) -> Result<(), String> {
        // 要求启用通知前先接受协议。
        (!profile.notifications || profile.accepted)
            // 合法候选转换为成功结果。
            .then_some(())
            // 非法候选转换为用户可见错误。
            .ok_or_else(|| "启用通知前必须接受协议".to_string())
    }

    // 编译类型化字段投影、整表规则、提交回调与 View 组合。
    fn profile_form_view() -> ViewNode {
        // 创建由业务作用域持有的已提交模型状态。
        let model = State::new(ProfileForm::default());
        // 构造由 Form Module 拥有字段与校验生命周期的句柄。
        let form = Form::model(&model)
            // 把姓名字段投影到必填文本输入项。
            .field(
                // 声明稳定字段 key。
                "name",
                // 返回模型中的姓名可变引用。
                |value| &mut value.name,
                // 创建必填文本字段声明。
                FormInputItem::new("name").required(true),
            )
            // 把年龄字段投影到零至一百二十的数值输入项。
            .field(
                // 声明稳定字段 key。
                "age",
                // 返回模型中的年龄可变引用。
                |value| &mut value.age,
                // 创建带范围约束的数值字段声明。
                FormInputNumberItem::new("age").min(0.0).max(120.0),
            )
            // 把等级字段投影到必填选择项。
            .field(
                // 声明稳定字段 key。
                "level",
                // 返回模型中的等级可变引用。
                |value| &mut value.level,
                // 创建带标签、选项和必填规则的选择字段。
                FormSelectItem::new("level")
                    // 声明用户可见标签。
                    .label("等级")
                    // 声明稳定选项集合。
                    .options(["初级", "中级", "高级"])
                    // 声明必填规则。
                    .required(true),
            )
            // 把协议字段投影到必选复选框项。
            .field(
                // 声明稳定字段 key。
                "accepted",
                // 返回模型中的协议布尔引用。
                |value| &mut value.accepted,
                // 创建区分字段标签与控件说明的复选框字段。
                FormCheckboxItem::new("accepted")
                    // 声明字段壳标签。
                    .field_label("协议确认")
                    // 声明复选框自身说明。
                    .label("我已阅读并同意")
                    // 要求值为 true。
                    .required(true),
            )
            // 把渠道字段投影到必填单选项。
            .field(
                // 声明稳定字段 key。
                "channel",
                // 返回模型中的渠道可变引用。
                |value| &mut value.channel,
                // 创建带标签、候选和必填规则的单选字段。
                FormRadioItem::new("channel")
                    // 声明用户可见字段标签。
                    .label("通知渠道")
                    // 声明邮件与短信候选。
                    .options(["邮件", "短信"])
                    // 声明必填规则。
                    .required(true),
            )
            // 把通知字段投影到必选开关项。
            .field(
                // 声明稳定字段 key。
                "notifications",
                // 返回模型中的通知布尔引用。
                |value| &mut value.notifications,
                // 创建带字段标签和必选规则的开关字段。
                FormSwitchItem::new("notifications")
                    // 声明用户可见字段标签。
                    .label("启用通知")
                    // 要求值为 true。
                    .required(true),
            )
            // 把音量字段投影到零至一百的滑块项。
            .field(
                // 声明稳定字段 key。
                "volume",
                // 返回模型中的音量可变引用。
                |value| &mut value.volume,
                // 创建带范围、标签和步长的滑块字段。
                FormSliderItem::new("volume", 0.0..=100.0)
                    // 声明用户可见字段标签。
                    .label("音量")
                    // 声明五单位步长。
                    .step(5.0),
            )
            // 在全部字段规则后登记跨字段业务约束。
            .model_rule(validate_profile)
            // 登记接收类型化候选模型的提交回调。
            .on_submit_typed(|profile: ProfileForm| {
                // 保留文档中的类型化业务消费示例。
                println!("提交：{} / {}", profile.name, profile.age);
                // 返回成功而不引入额外副作用。
                Ok(())
            })
            // 完成类型化表单句柄构建。
            .build();

        // 在同一声明树中组合表单 View 与提交按钮。
        embed(column((
            // 物化由 Form Module 持有字段状态的表单 View。
            form.view(),
            // 构造捕获表单句柄的提交按钮。
            button("提交").on_click_fn(move || {
                // 触发类型化提交并由应用决定如何展示错误。
                let _result = form.submit_typed();
            }),
        )))
    }
}
