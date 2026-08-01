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

use crate::ui::form::form::{Form, FormLayout};
use crate::ui::form::form_binding::{FormInputItem, FormInputNumberItem, FormSelectItem};
use crate::ui::form::form_validation::{FieldError, FormBuilder, FormModel, IntoFormValue};
use crate::ui::view::{View, ViewNode};
use crate::ui::widgets::input::input_number::InputNumberValue;
use crate::ui::widgets::input::select::SelectValue;
use crate::ui::State;

/// 类型化字段控件声明：`Form::model(...).field(name, accessor, item)` 的 item 契约。
///
/// 已支持 `FormInputItem`（文本）、`FormInputNumberItem<T>`（数值）、
/// `FormSelectItem<T>`（单选）；其余字段控件按需扩展。
pub trait FormItemSpec<F> {
    /// 绑定值 State 并构建字段 View（FormItem + 控件）。
    fn bind_view(&self, form: &FormModel, value: &State<F>) -> ViewNode;

    /// 字段是否必填（`Form::model` 构建时登记校验规则）。
    fn required(&self) -> bool {
        false
    }
}

impl FormItemSpec<String> for FormInputItem {
    fn bind_view(&self, form: &FormModel, value: &State<String>) -> ViewNode {
        let mut bound = form
            .input_item(&self.field, value)
            .expect("Form::model 字段未在内部模型登记");
        bound.placeholder = self.placeholder.clone();
        bound.required = self.required;
        bound.show_error = self.show_error;
        bound.build()
    }

    fn required(&self) -> bool {
        self.required
    }
}

impl<T> FormItemSpec<T> for FormInputNumberItem<T>
where
    T: InputNumberValue + IntoFormValue<Stored = T> + Clone + Send + Sync + 'static,
{
    fn bind_view(&self, form: &FormModel, value: &State<T>) -> ViewNode {
        let mut bound = form
            .input_number_item(&self.field, value)
            .expect("Form::model 字段未在内部模型登记");
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
        let mut bound = form
            .select_item(&self.field, value)
            .expect("Form::model 字段未在内部模型登记");
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
    fn required(&self) -> bool;
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

    fn required(&self) -> bool {
        self.item.required()
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
            let mut fb = match form_builder.take() {
                Some(fb) => fb.field(field.name(), field.name()),
                None => self.layout.clone().field(field.name(), field.name()),
            };
            if field.required() {
                fb = fb.required("必填");
            }
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
            on_submit: None,
        }
    }
}
