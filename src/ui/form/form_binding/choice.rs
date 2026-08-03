use std::ops::RangeInclusive;

use crate::native::windowing::input::ControlSize;
use crate::ui::form::form_validation::FormModel;
use crate::ui::view::{View, ViewNode};
use crate::ui::widgets::input::{Checkbox, Radio, Rate, Segmented, Slider, Switch};
use crate::ui::{FocusHandle, State};

use super::{bind_typed_control, form_item_shell};



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

/// 一个已登记字段的声明式日期输入项。

impl FormModel {

    pub fn checkbox_item(
        &self,
        field: impl AsRef<str>,
        value: &State<bool>,
    ) -> Option<FormCheckboxItem> {
        let field = field.as_ref();
        let focus_handle = self.focus_handle_for(field)?;
        self.register_reset_state(field, value);
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
        self.register_reset_state(field, value);
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
        self.register_reset_state(field, value);
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
        self.register_reset_state(field, value);
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
        self.register_reset_state(field, value);
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
        self.register_reset_state(field, value);
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
