//! `FormModel` 与声明式输入 View 的绑定。

use crate::native::traits::input::ControlSize;
use crate::ui::view::{input, EventExt, View, ViewNode};
use crate::ui::{EventResult, FocusHandle, SemanticKind, State, SystemEvent};

use super::{
    FormItem, FormModel, InputNumber, InputNumberValue, IntoFormValue, OptGroup, Select,
    ValidateStatus,
};

/// 一个已登记字段的声明式文本输入项。
pub struct FormInputItem {
    model: FormModel,
    field: String,
    value: State<String>,
    focus_handle: FocusHandle,
    placeholder: String,
    show_error: bool,
}

impl FormInputItem {
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
}

impl View for FormInputItem {
    fn build(self) -> ViewNode {
        let current = self.value.get();
        self.model.sync_text_value(&self.field, &current);

        let change_model = self.model.clone();
        let change_field = self.field.clone();
        let blur_model = self.model.clone();
        let blur_field = self.field.clone();
        let blur_value = self.value.clone();
        let input = input()
            .placeholder(self.placeholder)
            .value(&self.value)
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
            .focus_handle(&self.focus_handle);

        form_item_shell(&self.model, &self.field, self.show_error, input)
    }
}

/// 一个已登记字段的声明式数值输入项。
pub struct FormInputNumberItem<T>
where
    T: InputNumberValue + IntoFormValue<Stored = T>,
{
    model: FormModel,
    field: String,
    value: State<T>,
    focus_handle: FocusHandle,
    input_number: InputNumber,
    show_error: bool,
}

impl<T> FormInputNumberItem<T>
where
    T: InputNumberValue + IntoFormValue<Stored = T>,
{
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
        let current = self.value.get();
        self.model.sync_typed_value(&self.field, &current);

        let change_model = self.model.clone();
        let change_field = self.field.clone();
        let change_value = self.value.clone();
        let blur_model = self.model.clone();
        let blur_field = self.field.clone();
        let input = ViewNode::leaf(self.input_number.value(&self.value));
        let input = input
            .on_semantic(SemanticKind::Change, move |_| {
                let value = change_value.get();
                change_model.sync_typed_value(&change_field, &value);
            })
            .on_focus(move |event| {
                if matches!(event, SystemEvent::FocusOut) {
                    blur_model.blur(&blur_field);
                }
                EventResult::NotHandled
            })
            .focus_handle(&self.focus_handle);

        form_item_shell(&self.model, &self.field, self.show_error, input)
    }
}

/// 一个已登记字段的声明式单选输入项。
pub struct FormSelectItem {
    model: FormModel,
    field: String,
    value: State<String>,
    focus_handle: FocusHandle,
    options: Vec<String>,
    optgroups: Vec<OptGroup>,
    placeholder: String,
    searchable: bool,
    disabled: Option<bool>,
    size: Option<ControlSize>,
    show_error: bool,
}

impl FormSelectItem {
    /// 设置平铺选项。
    pub fn options<I, S>(mut self, options: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.options = options
            .into_iter()
            .map(|option| option.as_ref().to_string())
            .collect();
        self
    }

    /// 设置分组选项。
    pub fn optgroups(mut self, groups: Vec<OptGroup>) -> Self {
        self.optgroups = groups;
        self
    }

    /// 启用可搜索单选模式。
    pub fn searchable(mut self) -> Self {
        self.searchable = true;
        self
    }

    /// 设置无选中项时的占位文本。
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// 设置是否禁用输入。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = Some(disabled);
        self
    }

    /// 设置输入控件尺寸。
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = Some(size);
        self
    }

    /// 控制是否在字段下方显示首条错误文本；错误状态仍会保留。
    pub fn show_error(mut self, show_error: bool) -> Self {
        self.show_error = show_error;
        self
    }
}

impl View for FormSelectItem {
    fn build(self) -> ViewNode {
        let current = self.value.get();
        self.model.sync_text_value(&self.field, &current);

        let mut select = if self.searchable {
            Select::searchable()
        } else {
            Select::new()
        }
        .options(self.options)
        .optgroups(self.optgroups)
        .placeholder(self.placeholder)
        .value(&self.value);
        if let Some(disabled) = self.disabled {
            select = select.disabled(disabled);
        }
        if let Some(size) = self.size {
            select = select.size(size);
        }

        let change_model = self.model.clone();
        let change_field = self.field.clone();
        let change_value = self.value.clone();
        let blur_model = self.model.clone();
        let blur_field = self.field.clone();
        let input = ViewNode::leaf(select)
            .on_semantic(SemanticKind::Change, move |_| {
                let value = change_value.get();
                change_model.sync_text_value(&change_field, &value);
            })
            .on_focus(move |event| {
                if matches!(event, SystemEvent::FocusOut) {
                    blur_model.blur(&blur_field);
                }
                EventResult::NotHandled
            })
            .focus_handle(&self.focus_handle);

        form_item_shell(&self.model, &self.field, self.show_error, input)
    }
}

fn form_item_shell(model: &FormModel, field: &str, show_error: bool, input: ViewNode) -> ViewNode {
    let error = model.field_error(field);
    let status = if error.is_some() {
        ValidateStatus::Error
    } else {
        ValidateStatus::None
    };
    let help = error
        .as_ref()
        .filter(|_| show_error)
        .map_or_else(String::new, |error| error.message().to_string());
    let label = model
        .field_label(field)
        .map_or_else(String::new, str::to_string);

    ViewNode::new(
        FormItem::new(&label)
            .name(field.to_string())
            .required(model.field_is_required(field))
            .controlled_status(status)
            .help(&help)
            .label_width(model.layout().item_label_width())
            .layout(model.layout().item_layout()),
        vec![input],
    )
}

impl FormModel {
    /// 把已声明的字符串字段绑定为 `FormItem + Input` View。
    ///
    /// 字段不存在时返回 `None`；`Option<View>` 可直接放入 `column` / `row`。
    pub fn input_item(
        &self,
        field: impl AsRef<str>,
        value: &State<String>,
    ) -> Option<FormInputItem> {
        let field = field.as_ref();
        let focus_handle = self.focus_handle_for(field)?;
        Some(FormInputItem {
            model: self.clone(),
            field: field.to_string(),
            value: value.clone(),
            focus_handle,
            placeholder: String::new(),
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
        Some(FormInputNumberItem {
            model: self.clone(),
            field: field.to_string(),
            value: value.clone(),
            focus_handle,
            input_number: InputNumber::new(),
            show_error: true,
        })
    }

    /// 把已声明的字符串字段绑定为 `FormItem + Select` 单选 View。
    ///
    /// 字段不存在时返回 `None`；选项文本同时作为 Select 与表单的 typed `String` 真值。
    pub fn select_item(
        &self,
        field: impl AsRef<str>,
        value: &State<String>,
    ) -> Option<FormSelectItem> {
        let field = field.as_ref();
        let focus_handle = self.focus_handle_for(field)?;
        Some(FormSelectItem {
            model: self.clone(),
            field: field.to_string(),
            value: value.clone(),
            focus_handle,
            options: Vec::new(),
            optgroups: Vec::new(),
            placeholder: String::new(),
            searchable: false,
            disabled: None,
            size: None,
            show_error: true,
        })
    }
}
