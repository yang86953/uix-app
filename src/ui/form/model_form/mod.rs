//! 类型化表单模型 — `Form::model` 双向绑定业务结构体（E-01）。
//!
//! 与字符串字段校验模型（`FormModel`）共存：字段经 accessor 投影绑定
//! `State<M>` 的字段值，提交时校验全部字段，通过后组装类型化结果 `M`
//! 并触发 `on_submit_typed`；校验失败不覆盖模型与已输入内容。
//!
//! # SMC 边界（SMC-04 / E-01）
//!
//! 本文件属于 form Module：消费 widgets（Input / InputNumber / Select）与
//! view DSL，不依赖兄弟 Module 的私有实现；仅新增公开 API，不改变既有
//! 字符串表单行为。

use std::cell::RefCell;
use std::rc::Rc;

// 定义类型化全模型规则的独立所有权与执行边界。
mod model_rules;
// 引入全模型规则存储与验证入口。
use model_rules::{ModelRule, validate_model_rules};

/// 断言字段已在内部模型登记：未登记属 `Form::model` 投影误用，给出
/// 带明确提示的可诊断 panic（开发者契约错误，非运行时失败）。
fn field_bound_or_panic<T>(value: Option<T>, contract: &str) -> T {
    match value {
        Some(v) => v,
        None => panic!("{contract}"),
    }
}

use crate::ui::State;
use crate::ui::form::form::{Form, FormLayout};
use crate::ui::form::form_binding::{
    FormCheckboxItem,
    FormInputItem,
    FormInputNumberItem,
    FormRadioItem,
    FormSelectItem,
    // 引入类型化开关字段声明。
    FormSwitchItem,
};
use crate::ui::form::form_validation::{FieldError, FormBuilder, FormModel, IntoFormValue};
use crate::ui::view::{View, ViewNode};
use crate::ui::widgets::input::input_number::InputNumberValue;
use crate::ui::widgets::input::select::SelectValue;

/// 类型化字段控件声明：`Form::model(...).field(name, accessor, item)` 的 item 契约。
///
/// 已支持 `FormInputItem`（文本）、`FormInputNumberItem<T>`（数值）、
/// `FormSelectItem<T>`（单选）、`FormCheckboxItem`（布尔）、
/// `FormRadioItem`（单选组）、`FormSwitchItem`（布尔开关）、
/// `FormSliderItem`（f64 滑块）；其余字段控件按需扩展。
pub trait FormItemSpec<F> {
    /// 绑定值 State 并构建字段 View（FormItem + 控件）。
    fn bind_view(&self, form: &FormModel, value: &State<F>) -> ViewNode;

    /// 字段是否必填（`Form::model` 构建时登记校验规则）。
    fn required(&self) -> bool {
        false
    }

    /// 返回字段展示标签；默认沿用稳定字段 key。
    fn label<'a>(&'a self, field: &'a str) -> &'a str {
        // 未声明独立标签时不改变既有行为。
        field
    }

    /// 把字段项拥有的校验规则登记到统一 FormBuilder。
    fn configure_rules(&self, builder: FormBuilder) -> FormBuilder {
        // 默认只投影所有现有字段项都支持的 required 规则。
        if self.required() {
            // 保留既有必填错误文案。
            builder.required("必填")
        } else {
            // 无必填规则时原样返回构建器。
            builder
        }
    }
}

impl FormItemSpec<String> for FormInputItem {
    fn bind_view(&self, form: &FormModel, value: &State<String>) -> ViewNode {
        // 字段未登记属开发者误用：panic 并携带底层错误详情。
        let mut bound = field_bound_or_panic(
            form.input_item(&self.field, value),
            "Form::model 字段未在内部模型登记",
        );
        bound.placeholder = self.placeholder.clone();
        bound.required = self.required;
        bound.show_error = self.show_error;
        bound.build()
    }

    fn required(&self) -> bool {
        self.required
    }

    fn label<'a>(&'a self, field: &'a str) -> &'a str {
        // 显式标签优先，否则保持字段 key 兼容行为。
        self.label.as_deref().unwrap_or(field)
    }

    fn configure_rules(&self, builder: FormBuilder) -> FormBuilder {
        // 先登记可选必填规则。
        let builder = if self.required {
            // 使用类型化表单既有必填文案。
            builder.required("必填")
        } else {
            // 未启用必填时保持构建器不变。
            builder
        };
        // 再登记可选邮箱格式规则。
        if self.email {
            // 使用稳定的内置邮箱错误文案。
            builder.validate_email("邮箱格式不正确")
        } else {
            // 未启用邮箱规则时返回当前构建器。
            builder
        }
    }
}

impl<T> FormItemSpec<T> for FormInputNumberItem<T>
where
    T: InputNumberValue + IntoFormValue<Stored = T> + Clone + Send + Sync + 'static,
{
    fn bind_view(&self, form: &FormModel, value: &State<T>) -> ViewNode {
        let mut bound = field_bound_or_panic(
            form.input_number_item(&self.field, value),
            "Form::model 字段未在内部模型登记",
        );
        bound.input_number = self.input_number.clone();
        bound.required = self.required;
        bound.show_error = self.show_error;
        bound.build()
    }

    fn required(&self) -> bool {
        self.required
    }
}

impl<T> FormItemSpec<T> for FormSelectItem<T>
where
    T: SelectValue + IntoFormValue<Stored = T> + Clone + Send + Sync + 'static,
{
    fn bind_view(&self, form: &FormModel, value: &State<T>) -> ViewNode {
        let mut bound = field_bound_or_panic(
            form.select_item(&self.field, value),
            "Form::model 字段未在内部模型登记",
        );
        // 把独立展示标签投影到绑定后的字段配置。
        bound.label = self.label.clone();
        bound.options = self.options.clone();
        bound.optgroups = self.optgroups.clone();
        bound.placeholder = self.placeholder.clone();
        bound.searchable = self.searchable;
        bound.disabled = self.disabled;
        bound.size = self.size;
        bound.required = self.required;
        bound.show_error = self.show_error;
        bound.build()
    }

    fn required(&self) -> bool {
        self.required
    }

    fn label<'a>(&'a self, field: &'a str) -> &'a str {
        // 显式标签优先，否则继续沿用稳定字段 key。
        self.label.as_deref().unwrap_or(field)
    }
}

impl FormItemSpec<bool> for FormCheckboxItem {
    fn bind_view(&self, form: &FormModel, value: &State<bool>) -> ViewNode {
        // 通过 FormModel 建立统一字段状态和焦点绑定。
        let mut bound = field_bound_or_panic(
            form.checkbox_item(&self.field, value),
            "Form::model 字段未在内部模型登记",
        );
        // 投影独立表单标签。
        bound.field_label = self.field_label.clone();
        // 保留复选框自身说明文字。
        bound.label = self.label.clone();
        // 投影禁用状态。
        bound.disabled = self.disabled;
        // 投影控件尺寸。
        bound.size = self.size;
        // 投影必须勾选规则标记。
        bound.required = self.required;
        // 投影错误文本显示策略。
        bound.show_error = self.show_error;
        // 构建绑定后的字段 View。
        bound.build()
    }

    fn required(&self) -> bool {
        // 向 FormItem 公开必选语义。
        self.required
    }

    fn label<'a>(&'a self, field: &'a str) -> &'a str {
        // 独立表单标签优先，否则沿用稳定字段 key。
        self.field_label.as_deref().unwrap_or(field)
    }

    fn configure_rules(&self, builder: FormBuilder) -> FormBuilder {
        // 非必选字段不登记额外布尔规则。
        if !self.required {
            // 原样返回统一表单构建器。
            return builder;
        }
        // 登记 required 元数据，使 FormItem 公开必选状态。
        builder.required("必须勾选").custom(|value| {
            // 使用 IntoFormValue 的稳定布尔文本投影。
            if value == "true" {
                // 已勾选时通过校验。
                Ok(())
            } else {
                // 未勾选时返回面向用户的明确错误。
                Err("必须勾选".to_string())
            }
        })
    }
}

// 把声明式开关字段接入类型化 bool 投影契约。
impl FormItemSpec<bool> for FormSwitchItem {
    // 绑定统一表单状态并构建开关字段 View。
    fn bind_view(&self, form: &FormModel, value: &State<bool>) -> ViewNode {
        // 通过 FormModel 建立统一字段状态和焦点绑定。
        let mut bound = field_bound_or_panic(
            // 请求既有低层开关适配器。
            form.switch_item(&self.field, value),
            // 字段未登记时给出稳定开发者诊断。
            "Form::model 字段未在内部模型登记",
        );
        // 投影独立 FormItem 标签。
        bound.label = self.label.clone();
        // 投影禁用状态。
        bound.disabled = self.disabled;
        // 投影控件尺寸。
        bound.size = self.size;
        // 投影必须开启规则标记。
        bound.required = self.required;
        // 投影错误文本显示策略。
        bound.show_error = self.show_error;
        // 构建绑定后的字段 View。
        bound.build()
    }

    // 向统一表单元数据公开必须开启语义。
    fn required(&self) -> bool {
        // 返回字段声明中的规则开关。
        self.required
    }

    // 返回面向用户的表单标签。
    fn label<'a>(&'a self, field: &'a str) -> &'a str {
        // 独立标签优先，否则沿用稳定字段 key。
        self.label.as_deref().unwrap_or(field)
    }

    // 把开关必选语义登记到统一 FormBuilder。
    fn configure_rules(&self, builder: FormBuilder) -> FormBuilder {
        // 非必选字段不登记额外布尔规则。
        if !self.required {
            // 原样返回统一表单构建器。
            return builder;
        }
        // 登记 required 元数据并要求布尔真值。
        builder.required("必须开启").custom(|value| {
            // 使用 IntoFormValue 的稳定布尔文本投影。
            if value == "true" {
                // 已开启时通过校验。
                Ok(())
            } else {
                // 未开启时返回面向用户的明确错误。
                Err("必须开启".to_string())
            }
        })
    }
}

impl FormItemSpec<String> for FormRadioItem {
    fn bind_view(&self, form: &FormModel, value: &State<String>) -> ViewNode {
        // 通过 FormModel 建立统一字段状态和焦点绑定。
        let mut bound = field_bound_or_panic(
            form.radio_item(&self.field, value),
            "Form::model 字段未在内部模型登记",
        );
        // 投影独立 FormItem 标签。
        bound.label = self.label.clone();
        // 投影单选组名称。
        bound.group_name = self.group_name.clone();
        // 投影候选集合。
        bound.options = self.options.clone();
        // 投影禁用状态。
        bound.disabled = self.disabled;
        // 投影控件尺寸。
        bound.size = self.size;
        // 投影纵向布局开关。
        bound.vertical = self.vertical;
        // 投影必填规则标记。
        bound.required = self.required;
        // 投影错误文本显示策略。
        bound.show_error = self.show_error;
        // 构建绑定后的字段 View。
        bound.build()
    }

    fn required(&self) -> bool {
        // 向统一 FormBuilder 公开必填语义。
        self.required
    }

    fn label<'a>(&'a self, field: &'a str) -> &'a str {
        // 独立标签优先，否则沿用稳定字段 key。
        self.label.as_deref().unwrap_or(field)
    }
}

/// 类型化字段声明：name + accessor 投影 + 控件配置 + 绑定后的值 State。
///
/// 值 State 在首次 `view()` 时从模型投影创建并缓存；`write_back` 提交时
/// 把最新输入写回模型，`reset_from` 把模型当前值回灌字段。
struct TypedField<M, F, I> {
    name: String,
    accessor: Box<dyn Fn(&mut M) -> &mut F>,
    item: I,
    value: RefCell<Option<State<F>>>,
}

trait ModelField<M> {
    fn name(&self) -> &str;
    // 返回当前字段面向用户的展示标签。
    fn label(&self) -> &str;
    // 把字段项规则配置到当前 FormBuilder 字段。
    fn configure_rules(&self, builder: FormBuilder) -> FormBuilder;
    fn bind_view(&self, model: &State<M>, form: &FormModel) -> ViewNode;
    fn write_back(&self, model: &mut M);
    fn reset_from(&self, model: &M);
}

impl<M, F, I> ModelField<M> for TypedField<M, F, I>
where
    M: Clone + Send + Sync + 'static,
    F: Clone + Send + Sync + 'static,
    I: FormItemSpec<F> + 'static,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn label(&self) -> &str {
        // 委托字段项解析显式标签或字段 key 回退。
        self.item.label(&self.name)
    }

    fn configure_rules(&self, builder: FormBuilder) -> FormBuilder {
        // 委托字段项登记其拥有的规则集合。
        self.item.configure_rules(builder)
    }

    fn bind_view(&self, model: &State<M>, form: &FormModel) -> ViewNode {
        let value = self.value.borrow().clone().unwrap_or_else(|| {
            let mut current = model.get();
            let initial = (self.accessor)(&mut current).clone();
            State::new(initial)
        });
        *self.value.borrow_mut() = Some(value.clone());
        self.item.bind_view(form, &value)
    }

    fn write_back(&self, model: &mut M) {
        let value = self.value.borrow();
        let Some(state) = value.as_ref() else { return };
        let current = state.get();
        *(self.accessor)(model) = current;
    }

    fn reset_from(&self, model: &M) {
        let value = self.value.borrow();
        let Some(state) = value.as_ref() else { return };
        let mut current = model.clone();
        let initial = (self.accessor)(&mut current).clone();
        state.set(initial);
    }
}

/// `Form::model` 返回的类型化表单构建器。
pub struct ModelFormBuilder<M> {
    model: State<M>,
    layout: Form,
    fields: Vec<Box<dyn ModelField<M>>>,
    // 保存按声明顺序执行的类型化全模型规则。
    model_rules: Vec<ModelRule<M>>,
    on_submit: Option<Box<dyn Fn(M) -> Result<(), String>>>,
}

impl<M: Clone + Send + Sync + 'static> ModelFormBuilder<M> {
    /// 声明字段：accessor 投影模型字段，item 为字段控件配置（name 即标签）。
    pub fn field<F, A, I>(mut self, name: impl Into<String>, accessor: A, item: I) -> Self
    where
        F: Clone + Send + Sync + 'static,
        A: Fn(&mut M) -> &mut F + 'static,
        I: FormItemSpec<F> + 'static,
    {
        self.fields.push(Box::new(TypedField {
            name: name.into(),
            accessor: Box::new(accessor),
            item,
            value: RefCell::new(None),
        }));
        self
    }

    /// 注册类型化提交回调：校验通过并组装 `M` 后调用，`Err(String)` 上抛。
    pub fn on_submit_typed<F>(mut self, f: F) -> Self
    where
        F: Fn(M) -> Result<(), String> + 'static,
    {
        self.on_submit = Some(Box::new(f));
        self
    }

    /// 设置字段标签宽度（透传 Form 布局）。
    pub fn label_width(mut self, w: f32) -> Self {
        self.layout = self.layout.label_width(w);
        self
    }

    /// 设置字段间距（透传 Form 布局）。
    pub fn gap(mut self, g: f32) -> Self {
        self.layout = self.layout.gap(g);
        self
    }

    /// 设置表单布局方向（透传 Form 布局）。
    pub fn layout(mut self, l: FormLayout) -> Self {
        self.layout = self.layout.layout(l);
        self
    }

    /// 构建类型化表单句柄：应用侧持有；`view()` 生成字段区域，`submit_typed()` 提交。
    pub fn build(self) -> ModelForm<M> {
        let mut form_builder: Option<FormBuilder> = None;
        for field in &self.fields {
            let fb = match form_builder.take() {
                // 后续字段保留稳定 key，并使用字段项提供的展示标签。
                Some(fb) => fb.field(field.name(), field.label()),
                // 首个字段从表单布局创建同样的 key/label 契约。
                None => self.layout.clone().field(field.name(), field.label()),
            };
            // 由字段项统一登记 required、email 等运行时规则。
            let fb = field.configure_rules(fb);
            form_builder = Some(fb);
        }
        let form = match form_builder {
            Some(fb) => fb.build(),
            None => FormModel::from_fields(self.layout.clone(), Vec::new()),
        };
        ModelForm {
            model: self.model,
            inner: Rc::new(ModelFormInner {
                form,
                fields: self.fields,
                // 把规则所有权交给共享表单句柄。
                model_rules: self.model_rules,
                on_submit: self.on_submit,
            }),
        }
    }
}

/// 类型化表单句柄：绑定业务模型 `State<M>` 的字段投影与类型化提交（E-01）。
///
/// 克隆后仍共享字段值、校验状态与提交回调。
pub struct ModelForm<M: Clone> {
    model: State<M>,
    inner: Rc<ModelFormInner<M>>,
}

impl<M: Clone + Send + Sync + 'static> Clone for ModelForm<M> {
    fn clone(&self) -> Self {
        Self {
            model: self.model.clone(),
            inner: self.inner.clone(),
        }
    }
}

struct ModelFormInner<M> {
    form: FormModel,
    fields: Vec<Box<dyn ModelField<M>>>,
    // 保存候选模型组装后执行的规则。
    model_rules: Vec<ModelRule<M>>,
    on_submit: Option<Box<dyn Fn(M) -> Result<(), String>>>,
}

impl<M: Clone + Send + Sync + 'static> ModelForm<M> {
    /// 生成字段区域 View（嵌入 `embed` / `column` 等容器）。
    pub fn view(&self) -> ViewNode {
        let children = self
            .inner
            .fields
            .iter()
            .map(|field| field.bind_view(&self.model, &self.inner.form))
            .collect();
        ViewNode::new(self.inner.form.layout().clone(), children)
    }

    /// 校验全部字段；通过时组装类型化模型（不写入共享 State，不触发回调）。
    pub fn submit(&self) -> Result<M, Vec<FieldError>> {
        self.inner.form.validate()?;
        let mut next = self.model.get();
        for field in &self.inner.fields {
            field.write_back(&mut next);
        }
        // 字段规则通过后再按声明顺序校验完整候选模型。
        validate_model_rules(&self.inner.model_rules, &next)?;
        Ok(next)
    }

    /// 校验并提交：通过后把组装结果写回共享 `State<M>`，再调用 `on_submit_typed`。
    ///
    /// 校验失败时返回首个错误文本，不覆盖模型与已输入内容。
    pub fn submit_typed(&self) -> Result<(), String> {
        let next = self.submit().map_err(|errors| {
            errors
                .first()
                .map_or_else(|| "表单校验失败".to_string(), |e| e.message().to_string())
        })?;
        self.model.set(next.clone());
        if let Some(callback) = &self.inner.on_submit {
            callback(next)?;
        }
        Ok(())
    }

    /// 恢复字段为模型当前值（丢弃未提交输入），并清除已激活错误。
    pub fn reset(&self) {
        self.inner.form.reset();
        for field in &self.inner.fields {
            field.reset_from(&self.model.get());
        }
    }

    /// 返回绑定的业务模型 State。
    pub fn model(&self) -> State<M> {
        self.model.clone()
    }
}

impl Form {
    /// 类型化表单：绑定业务模型 `State<M>`，字段经 accessor 投影，
    /// `on_submit_typed` 提交即得类型化结果（E-01）。
    pub fn model<M: Clone + Send + Sync + 'static>(model: &State<M>) -> ModelFormBuilder<M> {
        ModelFormBuilder {
            model: model.clone(),
            layout: Form::new(),
            fields: Vec::new(),
            // 默认不登记全模型规则。
            model_rules: Vec::new(),
            on_submit: None,
        }
    }
}
