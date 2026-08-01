//! E-01 类型化表单模型 `Form::model` 行为契约。
//!
//! 覆盖：State<M> 绑定、accessor 投影、submit 组装 M、submit_typed 回调、
//! 校验失败不覆盖模型与已输入内容。

use uix::ui::{Form, FormInputItem, FormInputNumberItem, State};

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProfileForm {
    name: String,
    age: u8,
}

impl Default for ProfileForm {
    fn default() -> Self {
        Self {
            name: String::new(),
            age: 20,
        }
    }
}

#[test]
fn typed_form_builds_view_from_bound_model() {
    let model = State::new(ProfileForm {
        name: "初始".to_string(),
        age: 20,
    });
    let form = Form::model(&model)
        .field(
            "name",
            |m| &mut m.name,
            FormInputItem::new("name").required(true),
        )
        .field(
            "age",
            |m| &mut m.age,
            FormInputNumberItem::new("age").min(0.0).max(120.0),
        )
        .build();

    // view() 可重复构建（值 State 缓存复用，不重置输入）。
    let _view = form.view();
    let _view_again = form.view();
    let _ = _view_again;
}

#[test]
fn typed_form_submit_assembles_model_from_projection() {
    let model = State::new(ProfileForm {
        name: "初始".to_string(),
        age: 20,
    });
    let form = Form::model(&model)
        .field("name", |m| &mut m.name, FormInputItem::new("name"))
        .field("age", |m| &mut m.age, FormInputNumberItem::new("age"))
        .build();

    let submitted = form.submit().expect("校验通过");
    assert_eq!(
        submitted,
        ProfileForm {
            name: "初始".to_string(),
            age: 20,
        }
    );
}

#[test]
fn typed_form_submit_typed_writes_model_and_invokes_callback() {
    let model = State::new(ProfileForm {
        name: "初始".to_string(),
        age: 20,
    });
    let received = std::sync::Arc::new(std::sync::Mutex::new(None::<ProfileForm>));
    let hook = received.clone();
    let form = Form::model(&model)
        .field("name", |m| &mut m.name, FormInputItem::new("name"))
        .on_submit_typed(move |profile: ProfileForm| {
            *hook.lock().unwrap() = Some(profile);
            Ok(())
        })
        .build();

    form.submit_typed().expect("提交成功");
    assert_eq!(model.get().name, "初始");
    assert_eq!(received.lock().unwrap().as_ref().unwrap().age, 20);
}

#[test]
fn typed_form_callback_error_propagates() {
    let model = State::new(ProfileForm::default());
    let form = Form::model(&model)
        .field("name", |m| &mut m.name, FormInputItem::new("name"))
        .on_submit_typed(|_| Err("业务拒绝".to_string()))
        .build();

    let err = form.submit_typed().expect_err("回调错误应上抛");
    assert_eq!(err, "业务拒绝");
}

#[test]
fn typed_form_validation_failure_does_not_overwrite_model() {
    let model = State::new(ProfileForm {
        name: String::new(),
        age: 20,
    });
    let form = Form::model(&model)
        .field(
            "name",
            |m| &mut m.name,
            FormInputItem::new("name").required(true),
        )
        .field("age", |m| &mut m.age, FormInputNumberItem::new("age"))
        .build();

    let _view = form.view();
    let result = form.submit();
    assert!(result.is_err(), "必填字段为空应校验失败");
    assert!(form.submit_typed().is_err());

    // 校验失败不覆盖模型与已输入内容。
    assert_eq!(model.get().name, String::new());
    assert_eq!(model.get().age, 20);
}

#[test]
fn typed_form_reset_restores_model_values() {
    let model = State::new(ProfileForm {
        name: "保留".to_string(),
        age: 42,
    });
    let form = Form::model(&model)
        .field("name", |m| &mut m.name, FormInputItem::new("name"))
        .field("age", |m| &mut m.age, FormInputNumberItem::new("age"))
        .build();

    let _view = form.view();
    form.reset();
    let submitted = form.submit().expect("校验通过");
    assert_eq!(submitted.name, "保留");
    assert_eq!(submitted.age, 42);
}
