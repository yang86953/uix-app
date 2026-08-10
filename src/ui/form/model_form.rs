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
use crate::ui::form::form_binding::{FormInputItem, FormInputNumberItem, FormSelectItem};
use crate::ui::form::form_validation::{FieldError, FormBuilder, FormModel, IntoFormValue};
use crate::ui::view::{View, ViewNode};
use crate::ui::widgets::input::input_number::InputNumberValue;
use crate::ui::widgets::input::select::SelectValue;

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

// 集中验证类型化表单字段元数据投影。
#[cfg(test)]
mod tests {
    // 引入当前类型化表单实现。
    use super::*;
    // 引入稳定组件快照字段枚举。
    use crate::ui::component_snapshot::SnapshotFields;

    // 定义测试用业务模型。
    #[derive(Clone)]
    struct ContactForm {
        // 保存邮箱字段值。
        email: String,
    }

    // 验证字段 key 与用户可见标签保持独立。
    #[test]
    fn typed_input_item_projects_explicit_label_to_form_item() {
        // 创建带初始邮箱的受控模型。
        let model = State::new(ContactForm {
            // 提供合法值，避免规则影响结构测试。
            email: "owner@example.com".to_string(),
        });
        // 构建带独立展示标签的类型化字段。
        let form = Form::model(&model)
            // 字段 key 继续对应 Rust 模型成员。
            .field(
                "email",
                |value| &mut value.email,
                FormInputItem::new("email").label("电子邮箱"),
            )
            // 完成表单句柄构建。
            .build();
        // 生成真实字段 View 树。
        let view = form.view();
        // 读取首个 FormItem 的稳定快照字段。
        let fields = view.children[0].widget.snapshot_fields();
        // 快照必须同时保留字段 key 和独立标签。
        match fields {
            // 核对 FormItem 公开语义字段。
            SnapshotFields::FormItem { label, name, .. } => {
                // 标签使用文档声明的用户可见文本。
                assert_eq!(label, "电子邮箱");
                // 字段 key 仍稳定指向业务模型成员。
                assert_eq!(name, "email");
            }
            // 任何其他组件类型都表示字段壳投影失败。
            other => panic!("期望 FormItem 快照，实际为 {other:?}"),
        }
    }
}
