//! `FormModel` 与声明式文本输入 View 的绑定。

use crate::ui::view::{input, EventExt, View, ViewNode};
use crate::ui::{EventResult, FocusHandle, State, SystemEvent};

use super::{FormItem, FormModel, ValidateStatus};

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

        let error = self.model.field_error(&self.field);
        let status = if error.is_some() {
            ValidateStatus::Error
        } else {
            ValidateStatus::None
        };
        let help = error
            .as_ref()
            .filter(|_| self.show_error)
            .map_or_else(String::new, |error| error.message().to_string());
        let label = self
            .model
            .field_label(&self.field)
            .map_or_else(String::new, str::to_string);

        let change_model = self.model.clone();
        let change_field = self.field.clone();
        let blur_model = self.model.clone();
        let blur_field = self.field.clone();
        let input = input()
            .placeholder(self.placeholder)
            .value(&self.value)
            .on_change(move |value| {
                change_model.set_value(&change_field, value.to_string());
            })
            .on_focus(move |event| {
                if matches!(event, SystemEvent::FocusOut) {
                    blur_model.blur(&blur_field);
                }
                EventResult::NotHandled
            })
            .focus_handle(&self.focus_handle);

        ViewNode::new(
            FormItem::new(&label)
                .name(self.field.clone())
                .required(self.model.field_is_required(&self.field))
                .controlled_status(status)
                .help(&help)
                .label_width(self.model.layout().item_label_width())
                .layout(self.model.layout().item_layout()),
            vec![input],
        )
    }
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
}
