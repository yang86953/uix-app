//! `FormModel` 与声明式输入 View 的绑定。

use std::ops::RangeInclusive;

use crate::native::traits::input::ControlSize;
use crate::ui::view::{input, EventExt, View, ViewNode};
use crate::ui::{EventResult, FocusHandle, SemanticKind, State, SystemEvent};

use super::{
    Checkbox, FormItem, FormModel, InputNumber, InputNumberValue, IntoFormValue, OptGroup, Radio,
    Rate, Segmented, Select, SelectValue, Slider, Switch, ValidateStatus,
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
        let input = ViewNode::leaf(self.input_number.value(&self.value));
        let input = bind_typed_control(
            &self.model,
            &self.field,
            &self.value,
            &self.focus_handle,
            input,
        );

        form_item_shell(&self.model, &self.field, self.show_error, input)
    }
}

/// 一个已登记字段的声明式单选输入项。
pub struct FormSelectItem<T = String>
where
    T: SelectValue + IntoFormValue<Stored = T>,
{
    model: FormModel,
    field: String,
    value: State<T>,
    focus_handle: FocusHandle,
    options: Vec<String>,
    optgroups: Vec<OptGroup>,
    placeholder: String,
    searchable: bool,
    disabled: Option<bool>,
    size: Option<ControlSize>,
    show_error: bool,
}

impl<T> FormSelectItem<T>
where
    T: SelectValue + IntoFormValue<Stored = T>,
{
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

impl<T> View for FormSelectItem<T>
where
    T: SelectValue + IntoFormValue<Stored = T>,
{
    fn build(self) -> ViewNode {
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

        let input = bind_typed_control(
            &self.model,
            &self.field,
            &self.value,
            &self.focus_handle,
            ViewNode::leaf(select),
        );

        form_item_shell(&self.model, &self.field, self.show_error, input)
    }
}

/// 一个已登记字段的声明式复选输入项。
pub struct FormCheckboxItem {
    model: FormModel,
    field: String,
    value: State<bool>,
    focus_handle: FocusHandle,
    label: String,
    disabled: Option<bool>,
    size: Option<ControlSize>,
    show_error: bool,
}

impl FormCheckboxItem {
    /// 设置复选框自身的说明文本。
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
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

impl View for FormCheckboxItem {
    fn build(self) -> ViewNode {
        let mut checkbox = Checkbox::new(self.label).checked(&self.value);
        if let Some(disabled) = self.disabled {
            checkbox = checkbox.disabled(disabled);
        }
        if let Some(size) = self.size {
            checkbox = checkbox.size(size);
        }
        let input = bind_typed_control(
            &self.model,
            &self.field,
            &self.value,
            &self.focus_handle,
            ViewNode::leaf(checkbox),
        );
        form_item_shell(&self.model, &self.field, self.show_error, input)
    }
}

/// 一个已登记字段的声明式开关输入项。
pub struct FormSwitchItem {
    model: FormModel,
    field: String,
    value: State<bool>,
    focus_handle: FocusHandle,
    disabled: Option<bool>,
    size: Option<ControlSize>,
    show_error: bool,
}

impl FormSwitchItem {
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

impl View for FormSwitchItem {
    fn build(self) -> ViewNode {
        let mut switch = Switch::new().checked(&self.value);
        if let Some(disabled) = self.disabled {
            switch = switch.disabled(disabled);
        }
        if let Some(size) = self.size {
            switch = switch.size(size);
        }
        let input = bind_typed_control(
            &self.model,
            &self.field,
            &self.value,
            &self.focus_handle,
            ViewNode::leaf(switch),
        );
        form_item_shell(&self.model, &self.field, self.show_error, input)
    }
}

/// 一个已登记字段的声明式滑块输入项。
pub struct FormSliderItem {
    model: FormModel,
    field: String,
    value: State<f64>,
    focus_handle: FocusHandle,
    range: RangeInclusive<f64>,
    step: f64,
    size: Option<ControlSize>,
    show_error: bool,
}

impl FormSliderItem {
    /// 设置滑块步进值。
    pub fn step(mut self, step: f64) -> Self {
        self.step = step;
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

impl View for FormSliderItem {
    fn build(self) -> ViewNode {
        let mut slider = Slider::new(self.range).step(self.step).value(&self.value);
        if let Some(size) = self.size {
            slider = slider.size(size);
        }
        let input = bind_typed_control(
            &self.model,
            &self.field,
            &self.value,
            &self.focus_handle,
            ViewNode::leaf(slider),
        );
        form_item_shell(&self.model, &self.field, self.show_error, input)
    }
}

/// 一个已登记字段的声明式评分输入项。
pub struct FormRateItem {
    model: FormModel,
    field: String,
    value: State<u32>,
    focus_handle: FocusHandle,
    count: usize,
    allow_half: bool,
    disabled: Option<bool>,
    clearable: bool,
    character: String,
    size: Option<ControlSize>,
    show_error: bool,
}

impl FormRateItem {
    /// 设置评分项数量。
    pub fn count(mut self, count: usize) -> Self {
        self.count = count;
        self
    }

    /// 启用半星值；State 中一个单位表示半星。
    pub fn allow_half(mut self) -> Self {
        self.allow_half = true;
        self
    }

    /// 设置是否禁用输入。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = Some(disabled);
        self
    }

    /// 允许再次选择当前值时清零。
    pub fn clearable(mut self) -> Self {
        self.clearable = true;
        self
    }

    /// 设置评分字符。
    pub fn character(mut self, character: impl Into<String>) -> Self {
        self.character = character.into();
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

impl View for FormRateItem {
    fn build(self) -> ViewNode {
        let mut rate = Rate::new().count(self.count);
        if self.allow_half {
            rate = rate.allow_half();
        }
        if let Some(disabled) = self.disabled {
            rate = rate.disabled(disabled);
        }
        if self.clearable {
            rate = rate.clearable();
        }
        if !self.character.is_empty() {
            rate = rate.character(self.character);
        }
        if let Some(size) = self.size {
            rate = rate.size(size);
        }
        let input = bind_typed_control(
            &self.model,
            &self.field,
            &self.value,
            &self.focus_handle,
            ViewNode::leaf(rate.value(&self.value)),
        );
        form_item_shell(&self.model, &self.field, self.show_error, input)
    }
}

/// 一个已登记字段的声明式单选组输入项。
pub struct FormRadioItem {
    model: FormModel,
    field: String,
    value: State<String>,
    focus_handle: FocusHandle,
    group_name: String,
    options: Vec<String>,
    disabled: Option<bool>,
    size: Option<ControlSize>,
    vertical: bool,
    show_error: bool,
}

impl FormRadioItem {
    /// 设置单选组名。
    pub fn group_name(mut self, group_name: impl Into<String>) -> Self {
        self.group_name = group_name.into();
        self
    }

    /// 设置选项文本。
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

    /// 使用纵向选项布局。
    pub fn vertical(mut self) -> Self {
        self.vertical = true;
        self
    }

    /// 控制是否在字段下方显示首条错误文本；错误状态仍会保留。
    pub fn show_error(mut self, show_error: bool) -> Self {
        self.show_error = show_error;
        self
    }
}

impl View for FormRadioItem {
    fn build(self) -> ViewNode {
        let mut radio = Radio::group(self.group_name, self.options, &self.value);
        if let Some(disabled) = self.disabled {
            radio = radio.disabled(disabled);
        }
        if let Some(size) = self.size {
            radio = radio.size(size);
        }
        if self.vertical {
            radio = radio.vertical();
        }
        let input = bind_typed_control(
            &self.model,
            &self.field,
            &self.value,
            &self.focus_handle,
            ViewNode::leaf(radio),
        );
        form_item_shell(&self.model, &self.field, self.show_error, input)
    }
}

/// 一个已登记字段的声明式分段选择输入项。
pub struct FormSegmentedItem {
    model: FormModel,
    field: String,
    value: State<String>,
    focus_handle: FocusHandle,
    options: Vec<String>,
    disabled: Option<bool>,
    disabled_options: Vec<usize>,
    size: Option<ControlSize>,
    show_error: bool,
}

impl FormSegmentedItem {
    /// 设置选项文本。
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

    /// 设置是否禁用输入。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = Some(disabled);
        self
    }

    /// 禁用一个选项索引；键盘与指针选择均会跳过它。
    pub fn disable_option(mut self, index: usize) -> Self {
        if !self.disabled_options.contains(&index) {
            self.disabled_options.push(index);
        }
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

impl View for FormSegmentedItem {
    fn build(self) -> ViewNode {
        let mut segmented = Segmented::new(self.options).value(&self.value);
        if let Some(disabled) = self.disabled {
            segmented = segmented.disabled(disabled);
        }
        for index in self.disabled_options {
            segmented = segmented.disable_option(index);
        }
        if let Some(size) = self.size {
            segmented = segmented.size(size);
        }
        let input = bind_typed_control(
            &self.model,
            &self.field,
            &self.value,
            &self.focus_handle,
            ViewNode::leaf(segmented),
        );
        form_item_shell(&self.model, &self.field, self.show_error, input)
    }
}

fn bind_typed_control<T>(
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

    /// 把已声明的选项字段绑定为 `FormItem + Select` View。
    ///
    /// 字段不存在时返回 `None`；支持单选 `String` 与多选 `HashSet<String>` typed 真值。
    pub fn select_item<T>(
        &self,
        field: impl AsRef<str>,
        value: &State<T>,
    ) -> Option<FormSelectItem<T>>
    where
        T: SelectValue + IntoFormValue<Stored = T>,
    {
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

    /// 把已声明的布尔字段绑定为 `FormItem + Checkbox` View。
    pub fn checkbox_item(
        &self,
        field: impl AsRef<str>,
        value: &State<bool>,
    ) -> Option<FormCheckboxItem> {
        let field = field.as_ref();
        let focus_handle = self.focus_handle_for(field)?;
        Some(FormCheckboxItem {
            model: self.clone(),
            field: field.to_string(),
            value: value.clone(),
            focus_handle,
            label: String::new(),
            disabled: None,
            size: None,
            show_error: true,
        })
    }

    /// 把已声明的布尔字段绑定为 `FormItem + Switch` View。
    pub fn switch_item(
        &self,
        field: impl AsRef<str>,
        value: &State<bool>,
    ) -> Option<FormSwitchItem> {
        let field = field.as_ref();
        let focus_handle = self.focus_handle_for(field)?;
        Some(FormSwitchItem {
            model: self.clone(),
            field: field.to_string(),
            value: value.clone(),
            focus_handle,
            disabled: None,
            size: None,
            show_error: true,
        })
    }

    /// 把已声明的 `f64` 字段绑定为 `FormItem + Slider` View。
    pub fn slider_item(
        &self,
        field: impl AsRef<str>,
        value: &State<f64>,
        range: RangeInclusive<f64>,
    ) -> Option<FormSliderItem> {
        let field = field.as_ref();
        let focus_handle = self.focus_handle_for(field)?;
        Some(FormSliderItem {
            model: self.clone(),
            field: field.to_string(),
            value: value.clone(),
            focus_handle,
            range,
            step: 1.0,
            size: None,
            show_error: true,
        })
    }

    /// 把已声明的 `u32` 字段绑定为 `FormItem + Rate` View。
    pub fn rate_item(&self, field: impl AsRef<str>, value: &State<u32>) -> Option<FormRateItem> {
        let field = field.as_ref();
        let focus_handle = self.focus_handle_for(field)?;
        Some(FormRateItem {
            model: self.clone(),
            field: field.to_string(),
            value: value.clone(),
            focus_handle,
            count: 5,
            allow_half: false,
            disabled: None,
            clearable: false,
            character: String::new(),
            size: None,
            show_error: true,
        })
    }

    /// 把已声明的字符串字段绑定为 `FormItem + Radio` View。
    pub fn radio_item(
        &self,
        field: impl AsRef<str>,
        value: &State<String>,
    ) -> Option<FormRadioItem> {
        let field = field.as_ref();
        let focus_handle = self.focus_handle_for(field)?;
        Some(FormRadioItem {
            model: self.clone(),
            field: field.to_string(),
            value: value.clone(),
            focus_handle,
            group_name: field.to_string(),
            options: Vec::new(),
            disabled: None,
            size: None,
            vertical: false,
            show_error: true,
        })
    }

    /// 把已声明的字符串字段绑定为 `FormItem + Segmented` View。
    pub fn segmented_item(
        &self,
        field: impl AsRef<str>,
        value: &State<String>,
    ) -> Option<FormSegmentedItem> {
        let field = field.as_ref();
        let focus_handle = self.focus_handle_for(field)?;
        Some(FormSegmentedItem {
            model: self.clone(),
            field: field.to_string(),
            value: value.clone(),
            focus_handle,
            options: Vec::new(),
            disabled: None,
            disabled_options: Vec::new(),
            size: None,
            show_error: true,
        })
    }
}
