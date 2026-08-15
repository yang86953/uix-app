use std::ops::RangeInclusive;

use crate::platform::windowing::ControlSize;
use crate::ui::form::form_validation::FormModel;
use crate::ui::view::{View, ViewNode};
use crate::ui::widgets::input::{Checkbox, Radio, Rate, Segmented, Slider, Switch};
use crate::ui::{FocusHandle, State};

use super::{bind_typed_control, bound_required, form_item_shell};

/// 一个已登记字段的声明式复选输入项。
pub struct FormCheckboxItem {
    // 保存统一表单模型；声明阶段允许尚未绑定。
    pub(crate) model: Option<FormModel>,
    // 保存稳定业务字段 key。
    pub(crate) field: String,
    // 保存布尔字段状态；声明阶段允许尚未绑定。
    pub(crate) value: Option<State<bool>>,
    // 保存字段焦点句柄；声明阶段允许尚未绑定。
    pub(crate) focus_handle: Option<FocusHandle>,
    // 保存 FormItem 独立展示标签。
    pub(crate) field_label: Option<String>,
    // 保存复选框自身说明文本。
    pub(crate) label: String,
    // 保存可选禁用状态。
    pub(crate) disabled: Option<bool>,
    // 保存可选控件尺寸。
    pub(crate) size: Option<ControlSize>,
    // 保存必须勾选校验开关。
    pub(crate) required: bool,
    // 保存错误文本显示策略。
    pub(crate) show_error: bool,
}

impl FormCheckboxItem {
    /// 声明式裸配置；经 `Form::model(...).field(...)` 投影绑定时使用。
    pub fn new(field: impl Into<String>) -> Self {
        // 创建尚未绑定运行时模型的字段声明。
        Self {
            // 绑定阶段由 ModelForm 注入统一模型。
            model: None,
            // 保存稳定业务字段 key。
            field: field.into(),
            // 绑定阶段由 ModelForm 注入字段值状态。
            value: None,
            // 绑定阶段由 ModelForm 注入焦点句柄。
            focus_handle: None,
            // 默认沿用字段 key 作为表单标签。
            field_label: None,
            // 默认不显示复选框自身说明文本。
            label: String::new(),
            // 默认启用输入。
            disabled: None,
            // 默认沿用控件尺寸。
            size: None,
            // 默认不要求勾选。
            required: false,
            // 默认显示首条错误文本。
            show_error: true,
        }
    }

    /// 设置独立于字段 key 的表单展示标签。
    pub fn field_label(mut self, label: impl Into<String>) -> Self {
        // 保存 FormItem 面向用户的展示文本。
        self.field_label = Some(label.into());
        // 返回配置后的字段声明。
        self
    }

    /// 设置复选框自身的说明文本。
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// 声明布尔字段必须勾选后才能提交。
    pub fn required(mut self, required: bool) -> Self {
        // 保存运行时校验开关。
        self.required = required;
        // 返回配置后的字段声明。
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
        // 断言字段已由类型化表单或低层 FormModel 绑定。
        let model = bound_required(
            self.model.clone(),
            "FormCheckboxItem 未绑定：请经 Form::model 字段投影或 FormModel::checkbox_item 创建",
        );
        // 断言布尔值状态已注入。
        let value = bound_required(self.value.clone(), "FormCheckboxItem 未绑定值 State");
        // 断言焦点句柄已注入。
        let focus_handle =
            bound_required(self.focus_handle.clone(), "FormCheckboxItem 未绑定焦点句柄");
        let mut checkbox = Checkbox::new(self.label).checked(&value);
        if let Some(disabled) = self.disabled {
            checkbox = checkbox.disabled(disabled);
        }
        if let Some(size) = self.size {
            checkbox = checkbox.size(size);
        }
        let input = bind_typed_control(
            &model,
            &self.field,
            &value,
            &focus_handle,
            ViewNode::leaf(checkbox),
        );
        form_item_shell(&model, &self.field, self.show_error, input)
    }
}

/// 一个已登记字段的声明式开关输入项。
pub struct FormSwitchItem {
    // 保存统一表单模型；声明阶段允许尚未绑定。
    pub(crate) model: Option<FormModel>,
    // 保存稳定业务字段 key。
    pub(crate) field: String,
    // 保存布尔字段状态；声明阶段允许尚未绑定。
    pub(crate) value: Option<State<bool>>,
    // 保存字段焦点句柄；声明阶段允许尚未绑定。
    pub(crate) focus_handle: Option<FocusHandle>,
    // 保存 FormItem 独立展示标签。
    pub(crate) label: Option<String>,
    // 保存可选禁用状态。
    pub(crate) disabled: Option<bool>,
    // 保存可选控件尺寸。
    pub(crate) size: Option<ControlSize>,
    // 保存必须开启校验开关。
    pub(crate) required: bool,
    // 保存错误文本显示策略。
    pub(crate) show_error: bool,
}

impl FormSwitchItem {
    /// 声明式裸配置；经 `Form::model(...).field(...)` 投影绑定时使用。
    pub fn new(field: impl Into<String>) -> Self {
        // 创建尚未绑定运行时模型的字段声明。
        Self {
            // 绑定阶段由 ModelForm 注入统一模型。
            model: None,
            // 保存稳定业务字段 key。
            field: field.into(),
            // 绑定阶段由 ModelForm 注入字段值状态。
            value: None,
            // 绑定阶段由 ModelForm 注入焦点句柄。
            focus_handle: None,
            // 默认沿用字段 key 作为表单标签。
            label: None,
            // 默认启用输入。
            disabled: None,
            // 默认沿用控件尺寸。
            size: None,
            // 默认不要求开关处于开启态。
            required: false,
            // 默认显示首条错误文本。
            show_error: true,
        }
    }

    /// 设置独立于字段 key 的表单展示标签。
    pub fn label(mut self, label: impl Into<String>) -> Self {
        // 保存 FormItem 面向用户的展示文本。
        self.label = Some(label.into());
        // 返回配置后的字段声明。
        self
    }

    /// 声明布尔字段必须开启后才能提交。
    pub fn required(mut self, required: bool) -> Self {
        // 保存运行时校验开关。
        self.required = required;
        // 返回配置后的字段声明。
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

impl View for FormSwitchItem {
    fn build(self) -> ViewNode {
        // 断言字段已由类型化表单或低层 FormModel 绑定。
        let model = bound_required(
            self.model.clone(),
            "FormSwitchItem 未绑定：请经 Form::model 字段投影或 FormModel::switch_item 创建",
        );
        // 断言布尔值状态已注入。
        let value = bound_required(self.value.clone(), "FormSwitchItem 未绑定值 State");
        // 断言焦点句柄已注入。
        let focus_handle =
            bound_required(self.focus_handle.clone(), "FormSwitchItem 未绑定焦点句柄");
        // 构造受控开关并绑定统一布尔状态。
        let mut switch = Switch::new().checked(&value);
        if let Some(disabled) = self.disabled {
            switch = switch.disabled(disabled);
        }
        if let Some(size) = self.size {
            switch = switch.size(size);
        }
        let input = bind_typed_control(
            &model,
            &self.field,
            &value,
            &focus_handle,
            ViewNode::leaf(switch),
        );
        form_item_shell(&model, &self.field, self.show_error, input)
    }
}

/// 一个已登记字段的声明式滑块输入项。
pub struct FormSliderItem {
    // 保存统一表单模型；声明阶段允许尚未绑定。
    pub(crate) model: Option<FormModel>,
    // 保存稳定业务字段 key。
    pub(crate) field: String,
    // 保存数值字段状态；声明阶段允许尚未绑定。
    pub(crate) value: Option<State<f64>>,
    // 保存字段焦点句柄；声明阶段允许尚未绑定。
    pub(crate) focus_handle: Option<FocusHandle>,
    // 保存 FormItem 独立展示标签。
    pub(crate) label: Option<String>,
    // 保存滑块的闭区间约束。
    pub(crate) range: RangeInclusive<f64>,
    // 保存滑块步进约束。
    pub(crate) step: f64,
    // 保存可选控件尺寸。
    pub(crate) size: Option<ControlSize>,
    // 保存错误文本显示策略。
    pub(crate) show_error: bool,
}

impl FormSliderItem {
    /// 声明式裸配置；经 `Form::model(...).field(...)` 投影绑定时使用。
    pub fn new(field: impl Into<String>, range: RangeInclusive<f64>) -> Self {
        // 创建尚未绑定运行时模型的字段声明。
        Self {
            // 绑定阶段由 ModelForm 注入统一模型。
            model: None,
            // 保存稳定业务字段 key。
            field: field.into(),
            // 绑定阶段由 ModelForm 注入字段值状态。
            value: None,
            // 绑定阶段由 ModelForm 注入焦点句柄。
            focus_handle: None,
            // 默认沿用字段 key 作为表单标签。
            label: None,
            // 保存调用方显式声明的范围。
            range,
            // 默认使用 Slider 的单位步长。
            step: 1.0,
            // 默认沿用控件尺寸。
            size: None,
            // 默认显示首条错误文本。
            show_error: true,
        }
    }

    /// 设置独立于字段 key 的表单展示标签。
    pub fn label(mut self, label: impl Into<String>) -> Self {
        // 保存 FormItem 面向用户的展示文本。
        self.label = Some(label.into());
        // 返回配置后的字段声明。
        self
    }

    /// 设置滑块步进值。
    pub fn step(mut self, step: f64) -> Self {
        // 保存步长并由底层 Slider 执行最终有限正数归一化。
        self.step = step;
        // 返回配置后的字段声明。
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
        // 断言字段已由类型化表单或低层 FormModel 绑定。
        let model = bound_required(
            self.model.clone(),
            "FormSliderItem 未绑定：请经 Form::model 字段投影或 FormModel::slider_item 创建",
        );
        // 断言 f64 值状态已注入。
        let value = bound_required(self.value.clone(), "FormSliderItem 未绑定值 State");
        // 断言焦点句柄已注入。
        let focus_handle =
            bound_required(self.focus_handle.clone(), "FormSliderItem 未绑定焦点句柄");
        // 构造受控滑块并绑定统一 f64 状态。
        let mut slider = Slider::new(self.range).step(self.step).value(&value);
        if let Some(size) = self.size {
            slider = slider.size(size);
        }
        let input = bind_typed_control(
            &model,
            &self.field,
            &value,
            &focus_handle,
            ViewNode::leaf(slider),
        );
        form_item_shell(&model, &self.field, self.show_error, input)
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
    // 保存统一表单模型；声明阶段允许尚未绑定。
    pub(crate) model: Option<FormModel>,
    // 保存稳定业务字段 key。
    pub(crate) field: String,
    // 保存字符串字段状态；声明阶段允许尚未绑定。
    pub(crate) value: Option<State<String>>,
    // 保存字段焦点句柄；声明阶段允许尚未绑定。
    pub(crate) focus_handle: Option<FocusHandle>,
    // 保存独立 FormItem 标签。
    pub(crate) label: Option<String>,
    // 保存单选组名称。
    pub(crate) group_name: String,
    // 保存按源码顺序排列的选项文本。
    pub(crate) options: Vec<String>,
    // 保存可选禁用状态。
    pub(crate) disabled: Option<bool>,
    // 保存可选控件尺寸。
    pub(crate) size: Option<ControlSize>,
    // 保存纵向布局开关。
    pub(crate) vertical: bool,
    // 保存必填校验开关。
    pub(crate) required: bool,
    // 保存错误文本显示策略。
    pub(crate) show_error: bool,
}

impl FormRadioItem {
    /// 声明式裸配置；经 `Form::model(...).field(...)` 投影绑定时使用。
    pub fn new(field: impl Into<String>) -> Self {
        // 只求值一次并复用为默认分组名。
        let field = field.into();
        // 创建尚未绑定运行时模型的字段声明。
        Self {
            // 绑定阶段由 ModelForm 注入统一模型。
            model: None,
            // 保存稳定业务字段 key。
            field: field.clone(),
            // 绑定阶段由 ModelForm 注入字段值状态。
            value: None,
            // 绑定阶段由 ModelForm 注入焦点句柄。
            focus_handle: None,
            // 默认沿用字段 key 作为展示标签。
            label: None,
            // 默认使用字段 key 隔离同组单选项。
            group_name: field,
            // 默认候选集合为空。
            options: Vec::new(),
            // 默认启用输入。
            disabled: None,
            // 默认沿用控件尺寸。
            size: None,
            // 默认使用横向布局。
            vertical: false,
            // 默认不启用必填规则。
            required: false,
            // 默认显示首条错误文本。
            show_error: true,
        }
    }

    /// 设置独立于稳定字段 key 的表单展示标签。
    pub fn label(mut self, label: impl Into<String>) -> Self {
        // 保存 FormItem 面向用户的展示文本。
        self.label = Some(label.into());
        // 返回配置后的字段声明。
        self
    }

    /// 声明字段必须选择非空选项后才能提交。
    pub fn required(mut self, required: bool) -> Self {
        // 保存运行时必填校验开关。
        self.required = required;
        // 返回配置后的字段声明。
        self
    }

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
        // 断言字段已由类型化表单或低层 FormModel 绑定。
        let model = bound_required(
            self.model.clone(),
            "FormRadioItem 未绑定：请经 Form::model 字段投影或 FormModel::radio_item 创建",
        );
        // 断言字符串值状态已注入。
        let value = bound_required(self.value.clone(), "FormRadioItem 未绑定值 State");
        // 断言焦点句柄已注入。
        let focus_handle =
            bound_required(self.focus_handle.clone(), "FormRadioItem 未绑定焦点句柄");
        let mut radio = Radio::group(self.group_name, self.options, &value);
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
            &model,
            &self.field,
            &value,
            &focus_handle,
            ViewNode::leaf(radio),
        );
        form_item_shell(&model, &self.field, self.show_error, input)
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

/// 为已登记字段创建双向绑定布尔状态的声明式复选框项。
impl FormModel {
    /// 使用指定字段和值状态创建复选框项；字段未登记时返回 `None`。
    pub fn checkbox_item(
        &self,
        field: impl AsRef<str>,
        value: &State<bool>,
    ) -> Option<FormCheckboxItem> {
        let field = field.as_ref();
        let focus_handle = self.focus_handle_for(field)?;
        self.register_reset_state(field, value);
        Some(FormCheckboxItem {
            model: Some(self.clone()),
            field: field.to_string(),
            value: Some(value.clone()),
            focus_handle: Some(focus_handle),
            field_label: None,
            label: String::new(),
            disabled: None,
            size: None,
            required: false,
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
            // 注入统一表单模型。
            model: Some(self.clone()),
            // 保存稳定字段 key。
            field: field.to_string(),
            // 注入布尔字段状态。
            value: Some(value.clone()),
            // 注入已登记焦点句柄。
            focus_handle: Some(focus_handle),
            // 低层入口默认沿用字段元数据标签。
            label: None,
            // 默认启用开关。
            disabled: None,
            // 默认沿用控件尺寸。
            size: None,
            // 低层入口默认不要求开启。
            required: false,
            // 默认显示首条错误文本。
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
            // 注入统一表单模型。
            model: Some(self.clone()),
            // 保存稳定字段 key。
            field: field.to_string(),
            // 注入 f64 字段状态。
            value: Some(value.clone()),
            // 注入已登记焦点句柄。
            focus_handle: Some(focus_handle),
            // 低层入口默认沿用字段元数据标签。
            label: None,
            // 保存调用方声明的闭区间。
            range,
            // 默认使用单位步长。
            step: 1.0,
            // 默认沿用控件尺寸。
            size: None,
            // 默认显示首条错误文本。
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
            model: Some(self.clone()),
            field: field.to_string(),
            value: Some(value.clone()),
            focus_handle: Some(focus_handle),
            label: None,
            group_name: field.to_string(),
            options: Vec::new(),
            disabled: None,
            size: None,
            vertical: false,
            required: false,
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
