use crate::native::windowing::input::ControlSize;
use crate::ui::form::form_validation::{FormModel, IntoFormValue};
use crate::ui::view::{View, ViewNode};
use crate::ui::widgets::input::{OptGroup, Select, SelectValue};
use crate::ui::{FocusHandle, State};

use super::{bind_typed_control, bound_required, form_item_shell};


/// 一个已登记字段的声明式单选输入项。
/// 一个已登记字段的声明式单选输入项。
pub struct FormSelectItem<T = String>
where
    T: SelectValue + IntoFormValue<Stored = T>,
{
    pub(crate) model: Option<FormModel>,
    pub(crate) field: String,
    pub(crate) value: Option<State<T>>,
    pub(crate) focus_handle: Option<FocusHandle>,
    pub(crate) options: Vec<String>,
    pub(crate) optgroups: Vec<OptGroup>,
    pub(crate) placeholder: String,
    pub(crate) searchable: bool,
    pub(crate) disabled: Option<bool>,
    pub(crate) size: Option<ControlSize>,
    pub(crate) required: bool,
    pub(crate) show_error: bool,
}

impl<T> FormSelectItem<T>
where
    T: SelectValue + IntoFormValue<Stored = T>,
{
    /// 声明式裸配置：字段名即标签；经 `Form::model(...).field(...)` 投影绑定时使用。
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            model: None,
            field: name.into(),
            value: None,
            focus_handle: None,
            options: Vec::new(),
            optgroups: Vec::new(),
            placeholder: String::new(),
            searchable: false,
            disabled: None,
            size: None,
            required: false,
            show_error: true,
        }
    }

    /// 声明字段为必填（`Form::model` 构建时登记校验规则）。
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// 绑定模型、值 State 与焦点句柄（由 `FormModel::select_item` 或 `Form::model` 内部调用）。
    // 保留选择控件的内部绑定阶段入口，公开构造器由 FormModel 负责调用。
    #[allow(dead_code)]
    pub(crate) fn bind(self, model: FormModel, value: State<T>, focus_handle: FocusHandle) -> Self {
        Self {
            model: Some(model),
            value: Some(value),
            focus_handle: Some(focus_handle),
            ..self
        }
    }

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
        let model = bound_required(
            self.model.clone(),
            "FormSelectItem 未绑定：请经 Form::model 字段投影或 FormModel::select_item 创建",
        );
        let value = bound_required(self.value.clone(), "FormSelectItem 未绑定值 State");
        let focus_handle = bound_required(self.focus_handle.clone(), "FormSelectItem 未绑定焦点句柄");
        let mut select = if self.searchable {
            Select::searchable()
        } else {
            Select::new()
        }
        .options(self.options)
        .optgroups(self.optgroups)
        .placeholder(self.placeholder)
        .value(&value);
        if let Some(disabled) = self.disabled {
            select = select.disabled(disabled);
        }
        if let Some(size) = self.size {
            select = select.size(size);
        }

        let input = bind_typed_control(
            &model,
            &self.field,
            &value,
            &focus_handle,
            ViewNode::leaf(select),
        );

        form_item_shell(&model, &self.field, self.show_error, input)
    }
}

impl FormModel {

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
        self.register_reset_state(field, value);
        Some(FormSelectItem {
            model: Some(self.clone()),
            field: field.to_string(),
            value: Some(value.clone()),
            focus_handle: Some(focus_handle),
            options: Vec::new(),
            optgroups: Vec::new(),
            placeholder: String::new(),
            searchable: false,
            disabled: None,
            size: None,
            required: false,
            show_error: true,
        })
    }

}
