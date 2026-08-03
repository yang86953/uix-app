use std::sync::Arc;

use crate::draw::Color;
use crate::native::windowing::input::ControlSize;
use crate::ui::form::form::{FormItem, ValidateStatus};
use crate::ui::form::form_validation::{FormModel, IntoFormValue};
use crate::ui::view::{View, ViewNode};
use crate::ui::widgets::input::{
    ColorPicker, Date, DatePicker, DateRangePicker, PickerMode, PresetDate, Time, TimePicker,
};
use crate::ui::{EventResult, FocusHandle, SemanticKind, State, SystemEvent};



/// 一个已登记字段的声明式日期输入项。
pub struct FormDatePickerItem {
    model: FormModel,
    field: String,
    value: State<Date>,
    focus_handle: FocusHandle,
    placeholder: String,
    mode: PickerMode,
    disabled_date: Option<Arc<dyn Fn(Date) -> bool + Send + Sync>>,
    size: Option<ControlSize>,
    show_error: bool,
}

impl FormDatePickerItem {
    /// 设置占位文本。
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// 设置日期选择粒度。
    pub fn mode(mut self, mode: PickerMode) -> Self {
        self.mode = mode;
        self
    }

    /// 禁止选择满足谓词的日期。
    pub fn disabled_date(
        mut self,
        predicate: impl Fn(Date) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.disabled_date = Some(Arc::new(predicate));
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

impl View for FormDatePickerItem {
    fn build(self) -> ViewNode {
        let mut picker = DatePicker::new()
            .value(&self.value)
            .placeholder(self.placeholder)
            .mode(self.mode);
        if let Some(predicate) = self.disabled_date {
            picker = picker.disabled_date(move |date| predicate(date));
        }
        if let Some(size) = self.size {
            picker = picker.size(size);
        }
        let input = bind_typed_control(
            &self.model,
            &self.field,
            &self.value,
            &self.focus_handle,
            ViewNode::leaf(picker),
        );
        form_item_shell(&self.model, &self.field, self.show_error, input)
    }
}

/// 一个已登记字段的声明式时间输入项。
pub struct FormTimePickerItem {
    model: FormModel,
    field: String,
    value: State<Time>,
    focus_handle: FocusHandle,
    placeholder: String,
    size: Option<ControlSize>,
    show_error: bool,
}

impl FormTimePickerItem {
    /// 设置占位文本。
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
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

impl View for FormTimePickerItem {
    fn build(self) -> ViewNode {
        let mut picker = TimePicker::new()
            .value(&self.value)
            .placeholder(self.placeholder);
        if let Some(size) = self.size {
            picker = picker.size(size);
        }
        let input = bind_typed_control(
            &self.model,
            &self.field,
            &self.value,
            &self.focus_handle,
            ViewNode::leaf(picker),
        );
        form_item_shell(&self.model, &self.field, self.show_error, input)
    }
}

/// 一个已登记字段的声明式颜色输入项。
pub struct FormColorPickerItem {
    model: FormModel,
    field: String,
    value: State<Color>,
    focus_handle: FocusHandle,
    size: Option<ControlSize>,
    show_error: bool,
}

impl FormColorPickerItem {
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

impl View for FormColorPickerItem {
    fn build(self) -> ViewNode {
        let mut picker = ColorPicker::new().value(&self.value);
        if let Some(size) = self.size {
            picker = picker.size(size);
        }
        let input = bind_typed_control(
            &self.model,
            &self.field,
            &self.value,
            &self.focus_handle,
            ViewNode::leaf(picker),
        );
        form_item_shell(&self.model, &self.field, self.show_error, input)
    }
}

/// 两个已登记日期字段共享的声明式范围输入项。
pub struct FormDateRangePickerItem {
    model: FormModel,
    start_field: String,
    end_field: String,
    start: State<Date>,
    end: State<Date>,
    focus_handle: FocusHandle,
    label: Option<String>,
    placeholder: String,
    presets: Vec<(String, PresetDate)>,
    disabled_date: Option<Arc<dyn Fn(Date) -> bool + Send + Sync>>,
    size: Option<ControlSize>,
    show_error: bool,
}

impl FormDateRangePickerItem {
    /// 覆盖由起止字段标签组合出的表单项标签。
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// 设置占位文本。
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// 设置具名范围预设。
    pub fn presets<I, L>(mut self, presets: I) -> Self
    where
        I: IntoIterator<Item = (L, PresetDate)>,
        L: Into<String>,
    {
        self.presets = presets
            .into_iter()
            .map(|(label, preset)| (label.into(), preset))
            .collect();
        self
    }

    /// 禁止选择满足谓词的日期。
    pub fn disabled_date(
        mut self,
        predicate: impl Fn(Date) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.disabled_date = Some(Arc::new(predicate));
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

impl View for FormDateRangePickerItem {
    fn build(self) -> ViewNode {
        let start = self.start.get();
        let end = self.end.get();
        self.model.sync_typed_value(&self.start_field, &start);
        self.model.sync_typed_value(&self.end_field, &end);

        let mut picker = DateRangePicker::new()
            .start(&self.start)
            .end(&self.end)
            .placeholder(self.placeholder)
            .presets(self.presets);
        if let Some(predicate) = self.disabled_date {
            picker = picker.disabled_date(move |date| predicate(date));
        }
        if let Some(size) = self.size {
            picker = picker.size(size);
        }

        let change_model = self.model.clone();
        let change_start_field = self.start_field.clone();
        let change_end_field = self.end_field.clone();
        let change_start = self.start.clone();
        let change_end = self.end.clone();
        let blur_model = self.model.clone();
        let blur_start_field = self.start_field.clone();
        let blur_end_field = self.end_field.clone();
        let input = ViewNode::leaf(picker)
            .on_semantic(SemanticKind::Change, move |_| {
                let start = change_start.get();
                let end = change_end.get();
                change_model.sync_typed_value(&change_start_field, &start);
                change_model.sync_typed_value(&change_end_field, &end);
            })
            .on_focus(move |event| {
                if matches!(event, SystemEvent::FocusOut) {
                    blur_model.blur(&blur_start_field);
                    blur_model.blur(&blur_end_field);
                }
                EventResult::NotHandled
            })
            .focus_handle(&self.focus_handle);

        form_range_item_shell(
            &self.model,
            &self.start_field,
            &self.end_field,
            self.label.as_deref(),
            self.show_error,
            input,
        )
    }
}

pub(crate) fn bind_typed_control<T>(
    model: &FormModel,
    field: &str,
    value: &State<T>,
    focus_handle: &FocusHandle,
    input: ViewNode,
) -> ViewNode
where
    T: Clone + PartialEq + Send + Sync + IntoFormValue<Stored = T> + 'static,
{
    let current = value.get();
    model.sync_typed_value(field, &current);

    let change_model = model.clone();
    let change_field = field.to_string();
    let change_value = value.clone();
    let blur_model = model.clone();
    let blur_field = field.to_string();
    input
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
        .focus_handle(focus_handle)
}

pub(crate) fn form_item_shell(model: &FormModel, field: &str, show_error: bool, input: ViewNode) -> ViewNode {
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

fn form_range_item_shell(
    model: &FormModel,
    start_field: &str,
    end_field: &str,
    label: Option<&str>,
    show_error: bool,
    input: ViewNode,
) -> ViewNode {
    let error = model
        .field_error(start_field)
        .or_else(|| model.field_error(end_field));
    let status = if error.is_some() {
        ValidateStatus::Error
    } else {
        ValidateStatus::None
    };
    let help = error
        .as_ref()
        .filter(|_| show_error)
        .map_or_else(String::new, |error| error.message().to_string());
    let default_label = || {
        let start = model.field_label(start_field).unwrap_or(start_field);
        let end = model.field_label(end_field).unwrap_or(end_field);
        if start == end {
            start.to_string()
        } else {
            format!("{start} – {end}")
        }
    };
    let label = label.map_or_else(default_label, str::to_string);

    ViewNode::new(
        FormItem::new(&label)
            .name(format!("{start_field}:{end_field}"))
            .required(model.field_is_required(start_field) || model.field_is_required(end_field))
            .controlled_status(status)
            .help(&help)
            .label_width(model.layout().item_label_width())
            .layout(model.layout().item_layout()),
        vec![input],
    )
}


impl FormModel {

    /// 把已声明的 `Date` 字段绑定为 `FormItem + DatePicker` View。
    pub fn date_picker_item(
        &self,
        field: impl AsRef<str>,
        value: &State<Date>,
    ) -> Option<FormDatePickerItem> {
        let field = field.as_ref();
        let focus_handle = self.focus_handle_for(field)?;
        self.register_reset_state(field, value);
        Some(FormDatePickerItem {
            model: self.clone(),
            field: field.to_string(),
            value: value.clone(),
            focus_handle,
            placeholder: String::new(),
            mode: PickerMode::Date,
            disabled_date: None,
            size: None,
            show_error: true,
        })
    }

    /// 把两个已声明的 `Date` 字段绑定为一个 `FormItem + DateRangePicker` View。
    ///
    /// 起止字段必须不同且都已登记；两者各自保留 typed `Date` 值、校验规则与提交错误。
    pub fn date_range_picker_item(
        &self,
        start_field: impl AsRef<str>,
        start: &State<Date>,
        end_field: impl AsRef<str>,
        end: &State<Date>,
    ) -> Option<FormDateRangePickerItem> {
        let start_field = start_field.as_ref();
        let end_field = end_field.as_ref();
        if start_field == end_field {
            return None;
        }
        let focus_handle = self.shared_focus_handle_for(&[start_field, end_field])?;
        self.register_reset_state(start_field, start);
        self.register_reset_state(end_field, end);
        Some(FormDateRangePickerItem {
            model: self.clone(),
            start_field: start_field.to_string(),
            end_field: end_field.to_string(),
            start: start.clone(),
            end: end.clone(),
            focus_handle,
            label: None,
            placeholder: String::new(),
            presets: Vec::new(),
            disabled_date: None,
            size: None,
            show_error: true,
        })
    }

    /// 把已声明的 `Time` 字段绑定为 `FormItem + TimePicker` View。
    pub fn time_picker_item(
        &self,
        field: impl AsRef<str>,
        value: &State<Time>,
    ) -> Option<FormTimePickerItem> {
        let field = field.as_ref();
        let focus_handle = self.focus_handle_for(field)?;
        self.register_reset_state(field, value);
        Some(FormTimePickerItem {
            model: self.clone(),
            field: field.to_string(),
            value: value.clone(),
            focus_handle,
            placeholder: String::new(),
            size: None,
            show_error: true,
        })
    }

    /// 把已声明的 `Color` 字段绑定为 `FormItem + ColorPicker` View。
    pub fn color_picker_item(
        &self,
        field: impl AsRef<str>,
        value: &State<Color>,
    ) -> Option<FormColorPickerItem> {
        let field = field.as_ref();
        let focus_handle = self.focus_handle_for(field)?;
        self.register_reset_state(field, value);
        Some(FormColorPickerItem {
            model: self.clone(),
            field: field.to_string(),
            value: value.clone(),
            focus_handle,
            size: None,
            show_error: true,
        })
    }

}
