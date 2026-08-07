use crate::native::windowing::input::ControlSize;
use crate::ui::form::form_validation::{FormModel, IntoFormValue};
use crate::ui::view::{EventExt, View, ViewNode};
use crate::ui::widgets::combinators::input;
use crate::ui::widgets::input::{InputNumber, InputNumberValue};
use crate::ui::{EventResult, FocusHandle, State, SystemEvent};

use super::{bind_typed_control, bound_required, form_item_shell};


/// 一个已登记字段的声明式文本输入项。
///
/// 经 `Form::model` 字段投影（裸配置）或 `FormModel::input_item`（已绑定）创建；
/// 直接作为 View 使用前必须完成绑定，否则构建时 panic（带明确提示）。
pub struct FormInputItem {
    pub(crate) model: Option<FormModel>,
    pub(crate) field: String,
    pub(crate) value: Option<State<String>>,
    pub(crate) focus_handle: Option<FocusHandle>,
    pub(crate) placeholder: String,
    pub(crate) required: bool,
    pub(crate) show_error: bool,
}

impl FormInputItem {
    /// 声明式裸配置：字段名即标签；经 `Form::model(...).field(...)` 投影绑定时使用。
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            model: None,
            field: name.into(),
            value: None,
            focus_handle: None,
            placeholder: String::new(),
            required: false,
            show_error: true,
        }
    }

    /// 声明字段为必填（`Form::model` 构建时登记校验规则）。
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// 设置输入占位文本。
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// 控制是否在字段下方显示首条错误文本；错误状态仍会保留。
    pub fn show_error(mut self, show_error: bool) -> Self {
        self.show_error = show_error;
        self
    }

    /// 绑定模型、值 State 与焦点句柄（由 `FormModel::input_item` 或 `Form::model` 内部调用）。
    // 保留内部绑定阶段入口，公开构造器由 FormModel 负责调用。
    #[allow(dead_code)]
    pub(crate) fn bind(
        self,
        model: FormModel,
        value: State<String>,
        focus_handle: FocusHandle,
    ) -> Self {
        Self {
            model: Some(model),
            value: Some(value),
            focus_handle: Some(focus_handle),
            ..self
        }
    }
}

impl View for FormInputItem {
    fn build(self) -> ViewNode {
        let model = bound_required(
            self.model.clone(),
            "FormInputItem 未绑定：请经 Form::model 字段投影或 FormModel::input_item 创建",
        );
        let value = bound_required(self.value.clone(), "FormInputItem 未绑定值 State");
        let focus_handle = bound_required(self.focus_handle.clone(), "FormInputItem 未绑定焦点句柄");
        let current = value.get();
        model.sync_text_value(&self.field, &current);

        let change_model = model.clone();
        let change_field = self.field.clone();
        let blur_model = model.clone();
        let blur_field = self.field.clone();
        let blur_value = value.clone();
        let input = input()
            .placeholder(self.placeholder)
            .value(&value)
            .on_change(move |value| {
                change_model.sync_text_value(&change_field, value);
            })
            .on_focus(move |event| {
                if matches!(event, SystemEvent::FocusOut) {
                    let value = blur_value.get();
                    blur_model.sync_text_value(&blur_field, &value);
                    blur_model.blur(&blur_field);
                }
                EventResult::NotHandled
            })
            .focus_handle(&focus_handle);

        form_item_shell(&model, &self.field, self.show_error, input)
    }
}

/// 一个已登记字段的声明式数值输入项。
/// 一个已登记字段的声明式数值输入项。
pub struct FormInputNumberItem<T>
where
    T: InputNumberValue + IntoFormValue<Stored = T>,
{
    pub(crate) model: Option<FormModel>,
    pub(crate) field: String,
    pub(crate) value: Option<State<T>>,
    pub(crate) focus_handle: Option<FocusHandle>,
    pub(crate) input_number: InputNumber,
    pub(crate) required: bool,
    pub(crate) show_error: bool,
}

impl<T> FormInputNumberItem<T>
where
    T: InputNumberValue + IntoFormValue<Stored = T>,
{
    /// 声明式裸配置：字段名即标签；经 `Form::model(...).field(...)` 投影绑定时使用。
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            model: None,
            field: name.into(),
            value: None,
            focus_handle: None,
            input_number: InputNumber::new(),
            required: false,
            show_error: true,
        }
    }

    /// 声明字段为必填（`Form::model` 构建时登记校验规则）。
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// 绑定模型、值 State 与焦点句柄（由 `FormModel::input_number_item` 或 `Form::model` 内部调用）。
    // 保留数值控件的内部绑定阶段入口，公开构造器由 FormModel 负责调用。
    #[allow(dead_code)]
    pub(crate) fn bind(self, model: FormModel, value: State<T>, focus_handle: FocusHandle) -> Self {
        Self {
            model: Some(model),
            value: Some(value),
            focus_handle: Some(focus_handle),
            ..self
        }
    }

    /// 设置输入占位文本。
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.input_number = self.input_number.placeholder(placeholder);
        self
    }

    /// 设置允许的最小值。
    pub fn min(mut self, min: f64) -> Self {
        self.input_number = self.input_number.min(min);
        self
    }

    /// 设置允许的最大值。
    pub fn max(mut self, max: f64) -> Self {
        self.input_number = self.input_number.max(max);
        self
    }

    /// 设置步进值。
    pub fn step(mut self, step: f64) -> Self {
        self.input_number = self.input_number.step(step);
        self
    }

    /// 设置输入控件尺寸。
    pub fn size(mut self, size: ControlSize) -> Self {
        self.input_number = self.input_number.size(size);
        self
    }

    /// 设置是否禁用输入。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.input_number = self.input_number.disabled(disabled);
        self
    }

    /// 控制是否在字段下方显示首条错误文本；错误状态仍会保留。
    pub fn show_error(mut self, show_error: bool) -> Self {
        self.show_error = show_error;
        self
    }
}

impl<T> View for FormInputNumberItem<T>
where
    T: InputNumberValue + IntoFormValue<Stored = T>,
{
    fn build(self) -> ViewNode {
        let model = bound_required(
            self.model.clone(),
            "FormInputNumberItem 未绑定：请经 Form::model 字段投影或 FormModel::input_number_item 创建",
        );
        let value = bound_required(
            self.value.clone(),
            "FormInputNumberItem 未绑定值 State",
        );
        let focus_handle = bound_required(
            self.focus_handle.clone(),
            "FormInputNumberItem 未绑定焦点句柄",
        );
        let input = ViewNode::leaf(self.input_number.value(&value));
        let input = bind_typed_control(&model, &self.field, &value, &focus_handle, input);

        form_item_shell(&model, &self.field, self.show_error, input)
    }
}


impl FormModel {

    pub fn input_item(
        &self,
        field: impl AsRef<str>,
        value: &State<String>,
    ) -> Option<FormInputItem> {
        let field = field.as_ref();
        let focus_handle = self.focus_handle_for(field)?;
        self.register_reset_state(field, value);
        Some(FormInputItem {
            model: Some(self.clone()),
            field: field.to_string(),
            value: Some(value.clone()),
            focus_handle: Some(focus_handle),
            placeholder: String::new(),
            required: false,
            show_error: true,
        })
    }

    /// 把已声明的数值字段绑定为 `FormItem + InputNumber` View。
    ///
    /// 字段不存在时返回 `None`；支持 `InputNumberValue` 已覆盖的标准整数和浮点数。
    pub fn input_number_item<T>(
        &self,
        field: impl AsRef<str>,
        value: &State<T>,
    ) -> Option<FormInputNumberItem<T>>
    where
        T: InputNumberValue + IntoFormValue<Stored = T>,
    {
        let field = field.as_ref();
        let focus_handle = self.focus_handle_for(field)?;
        self.register_reset_state(field, value);
        Some(FormInputNumberItem {
            model: Some(self.clone()),
            field: field.to_string(),
            value: Some(value.clone()),
            focus_handle: Some(focus_handle),
            input_number: InputNumber::new(),
            required: false,
            show_error: true,
        })
    }

}
