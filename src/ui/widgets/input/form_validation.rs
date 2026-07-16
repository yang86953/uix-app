//! 应用侧表单值、规则与校验结果。

use std::any::Any;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::ops::RangeInclusive;
use std::rc::Rc;
use std::sync::Arc;

use regex::Regex;

use crate::ui::{FocusHandle, FocusHandleError, State};

use super::form::Form;

/// 把公开默认值转换成可保留具体类型的表单值，并提供供文本规则使用的稳定投影。
pub trait IntoFormValue {
    type Stored: Send + Sync + 'static;

    fn into_form_value(self) -> Self::Stored;

    /// 返回内置文本规则与 inline custom validator 读取的稳定文本。
    fn form_text(value: &Self::Stored) -> String;
}

impl IntoFormValue for &str {
    type Stored = String;

    fn into_form_value(self) -> Self::Stored {
        self.to_string()
    }

    fn form_text(value: &Self::Stored) -> String {
        value.clone()
    }
}

impl IntoFormValue for &String {
    type Stored = String;

    fn into_form_value(self) -> Self::Stored {
        self.clone()
    }

    fn form_text(value: &Self::Stored) -> String {
        value.clone()
    }
}

macro_rules! impl_identity_form_value {
    ($($value:ty),+ $(,)?) => {
        $(
            impl IntoFormValue for $value {
                type Stored = Self;

                fn into_form_value(self) -> Self::Stored {
                    self
                }

                fn form_text(value: &Self::Stored) -> String {
                    value.to_string()
                }
            }
        )+
    };
}

impl_identity_form_value!(
    String, bool, char, i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64,
);

impl IntoFormValue for HashSet<String> {
    type Stored = Self;

    fn into_form_value(self) -> Self::Stored {
        self
    }

    fn form_text(value: &Self::Stored) -> String {
        let mut values = value.iter().map(String::as_str).collect::<Vec<_>>();
        values.sort_unstable();
        values.join("\n")
    }
}

#[derive(Clone)]
struct StoredValue {
    typed: Arc<dyn Any + Send + Sync>,
    text: String,
}

impl StoredValue {
    fn new<V: IntoFormValue>(value: V) -> Self {
        let stored = value.into_form_value();
        let text = V::form_text(&stored);
        Self {
            typed: Arc::new(stored),
            text,
        }
    }

    fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }

    fn is_value<T>(&self, value: &T) -> bool
    where
        T: PartialEq + Send + Sync + 'static,
    {
        self.typed
            .as_ref()
            .downcast_ref::<T>()
            .is_some_and(|current| current == value)
    }

    fn is_text(&self, value: &str) -> bool {
        self.typed
            .as_ref()
            .downcast_ref::<String>()
            .is_some_and(|current| current == value)
    }
}

/// 一次成功校验得到的具名 typed values。
#[derive(Clone, Default)]
pub struct Values {
    entries: BTreeMap<String, Arc<dyn Any + Send + Sync>>,
}

impl Values {
    /// 按字段名和具体类型读取值；类型不匹配时返回 `None`。
    pub fn get<T: Send + Sync + 'static>(&self, field: &str) -> Option<&T> {
        self.entries.get(field)?.as_ref().downcast_ref::<T>()
    }

    pub fn contains(&self, field: &str) -> bool {
        self.entries.contains_key(field)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl fmt::Debug for Values {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Values")
            .field("fields", &self.entries.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// 单个字段的首个校验失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldError {
    field: String,
    message: String,
}

impl FieldError {
    fn new(field: &str, message: impl Into<String>) -> Self {
        Self {
            field: field.to_string(),
            message: message.into(),
        }
    }

    pub fn field(&self) -> &str {
        &self.field
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn into_parts(self) -> (String, String) {
        (self.field, self.message)
    }
}

impl fmt::Display for FieldError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.field, self.message)
    }
}

impl std::error::Error for FieldError {}

/// 字段规则的激活时机。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// 只在整表提交校验时激活。
    #[default]
    OnSubmit,
    /// 字段失焦时激活。
    OnBlur,
    /// 字段值变化时激活。
    OnChange,
}

type CustomValidator = Arc<dyn Fn(&str) -> Result<(), String> + 'static>;
type DependencyValidator = Arc<dyn Fn(&str, &Values) -> Result<(), String> + 'static>;

#[derive(Clone)]
enum FieldRule {
    Required(String),
    Email(String),
    Range {
        start: f64,
        end: f64,
        message: String,
    },
    Length {
        start: usize,
        end: usize,
        message: String,
    },
    Pattern {
        matcher: Option<Regex>,
        message: String,
    },
    Custom(CustomValidator),
    Dependency {
        field: String,
        validator: DependencyValidator,
    },
}

#[derive(Clone)]
pub(crate) struct FormField {
    name: String,
    label: String,
    value: StoredValue,
    rules: Vec<FieldRule>,
    trigger: Trigger,
}

impl FormField {
    fn new(name: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            value: StoredValue::new(String::new()),
            rules: Vec::new(),
            trigger: Trigger::OnSubmit,
        }
    }
}

/// `Form::field` 返回的字段声明构建器。
pub struct FormBuilder {
    layout: Form,
    fields: Vec<FormField>,
    current: FormField,
}

impl FormBuilder {
    pub(crate) fn new(layout: Form, name: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            layout,
            fields: Vec::new(),
            current: FormField::new(name, label),
        }
    }

    pub fn field(mut self, name: impl Into<String>, label: impl Into<String>) -> Self {
        let next = FormField::new(name, label);
        self.fields.push(std::mem::replace(&mut self.current, next));
        self
    }

    pub fn default<V: IntoFormValue>(mut self, value: V) -> Self {
        self.current.value = StoredValue::new(value);
        self
    }

    pub fn required(mut self, message: impl Into<String>) -> Self {
        self.current.rules.push(FieldRule::Required(message.into()));
        self
    }

    pub fn validate_email(mut self, message: impl Into<String>) -> Self {
        self.current.rules.push(FieldRule::Email(message.into()));
        self
    }

    pub fn validate_range<N>(mut self, range: RangeInclusive<N>, message: impl Into<String>) -> Self
    where
        N: Copy + Into<f64>,
    {
        self.current.rules.push(FieldRule::Range {
            start: (*range.start()).into(),
            end: (*range.end()).into(),
            message: message.into(),
        });
        self
    }

    pub fn validate_length(
        mut self,
        range: RangeInclusive<usize>,
        message: impl Into<String>,
    ) -> Self {
        self.current.rules.push(FieldRule::Length {
            start: *range.start(),
            end: *range.end(),
            message: message.into(),
        });
        self
    }

    pub fn validate_pattern(
        mut self,
        pattern: impl AsRef<str>,
        message: impl Into<String>,
    ) -> Self {
        self.current.rules.push(FieldRule::Pattern {
            matcher: Regex::new(pattern.as_ref()).ok(),
            message: message.into(),
        });
        self
    }

    pub fn custom(mut self, validator: impl Fn(&str) -> Result<(), String> + 'static) -> Self {
        self.current
            .rules
            .push(FieldRule::Custom(Arc::new(validator)));
        self
    }

    /// 声明当前字段依赖另一个字段；依赖源变化后会级联重验当前字段。
    pub fn depends_on(
        mut self,
        field: impl Into<String>,
        validator: impl Fn(&str, &Values) -> Result<(), String> + 'static,
    ) -> Self {
        self.current.rules.push(FieldRule::Dependency {
            field: field.into(),
            validator: Arc::new(validator),
        });
        self
    }

    /// 设置当前字段的规则激活时机。
    pub fn validate_trigger(mut self, trigger: Trigger) -> Self {
        self.current.trigger = trigger;
        self
    }

    /// 完成声明；同名字段以最后一次声明为准。
    pub fn build(self) -> FormModel {
        let (layout, fields) = self.into_parts();
        FormModel::from_fields(layout, fields)
    }

    pub(crate) fn into_parts(mut self) -> (Form, Vec<FormField>) {
        self.fields.push(self.current);
        let mut unique = Vec::<FormField>::with_capacity(self.fields.len());
        for field in self.fields {
            unique.retain(|existing| existing.name != field.name);
            unique.push(field);
        }
        (self.layout, unique)
    }
}

struct FormModelInner {
    layout: Form,
    fields: Vec<FormField>,
    values: RefCell<Vec<StoredValue>>,
    active_errors: State<Vec<Option<FieldError>>>,
    validation_active: RefCell<Vec<bool>>,
    focus_handles: RefCell<BTreeMap<String, FocusHandle>>,
}

/// 应用侧表单模型；克隆后仍共享字段值、校验状态与字段焦点句柄。
///
/// 自定义校验闭包只保留在这个应用侧句柄中，不进入 widget component。
#[derive(Clone)]
pub struct FormModel {
    inner: Rc<FormModelInner>,
}

impl FormModel {
    pub(crate) fn from_fields(layout: Form, fields: Vec<FormField>) -> Self {
        let values = fields.iter().map(|field| field.value.clone()).collect();
        let field_count = fields.len();
        Self {
            inner: Rc::new(FormModelInner {
                layout,
                fields,
                values: RefCell::new(values),
                active_errors: State::new(vec![None; field_count]),
                validation_active: RefCell::new(vec![false; field_count]),
                focus_handles: RefCell::new(BTreeMap::new()),
            }),
        }
    }

    /// 校验全部字段；每个字段返回首个错误，字段间按声明顺序收集。
    pub fn validate(&self) -> Result<Values, Vec<FieldError>> {
        let current_values = self.inner.values.borrow();
        let values = values_from_fields(&self.inner.fields, &current_values);
        let field_errors: Vec<Option<FieldError>> = self
            .inner
            .fields
            .iter()
            .zip(current_values.iter())
            .map(|(field, value)| validate_field(field, value, &values))
            .collect();
        let errors = field_errors.iter().flatten().cloned().collect::<Vec<_>>();
        drop(current_values);
        self.inner.validation_active.borrow_mut().fill(true);
        self.publish_errors(field_errors);
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(values)
    }

    /// 更新字段值；字段不存在时返回 `false`。
    pub fn set_value<V: IntoFormValue>(&self, field: &str, value: V) -> bool {
        let Some(index) = self.inner.fields.iter().position(|item| item.name == field) else {
            return false;
        };
        let mut current_values = self.inner.values.borrow_mut();
        current_values[index] = StoredValue::new(value);
        let values = values_from_fields(&self.inner.fields, &current_values);
        let dependents = dependent_indices(&self.inner.fields, index);
        let should_validate = {
            let mut validation_active = self.inner.validation_active.borrow_mut();
            if self.inner.fields[index].trigger == Trigger::OnChange {
                validation_active[index] = true;
            }
            for dependent in &dependents {
                validation_active[*dependent] = true;
            }
            validation_active[index]
        };
        let error = should_validate
            .then(|| validate_field(&self.inner.fields[index], &current_values[index], &values))
            .flatten();
        let mut active_errors = self.inner.active_errors.get_untracked();
        active_errors[index] = error;
        for dependent in dependents {
            active_errors[dependent] = validate_field(
                &self.inner.fields[dependent],
                &current_values[dependent],
                &values,
            );
        }
        drop(current_values);
        self.publish_errors(active_errors);
        true
    }

    /// 通知字段失焦；仅 `OnBlur` 字段会在此时执行规则。
    pub fn blur(&self, field: &str) -> bool {
        let Some(index) = self.inner.fields.iter().position(|item| item.name == field) else {
            return false;
        };
        if self.inner.fields[index].trigger == Trigger::OnBlur {
            self.inner.validation_active.borrow_mut()[index] = true;
            let current_values = self.inner.values.borrow();
            let values = values_from_fields(&self.inner.fields, &current_values);
            let error = validate_field(&self.inner.fields[index], &current_values[index], &values);
            drop(current_values);
            let mut active_errors = self.inner.active_errors.get_untracked();
            active_errors[index] = error;
            self.publish_errors(active_errors);
        }
        true
    }

    /// 返回当前已激活的字段错误。
    pub fn field_error(&self, field: &str) -> Option<FieldError> {
        let index = self
            .inner
            .fields
            .iter()
            .position(|item| item.name == field)?;
        self.inner.active_errors.get().get(index).cloned().flatten()
    }

    /// 按字段声明顺序返回当前已激活的错误。
    pub fn errors(&self) -> Vec<FieldError> {
        self.inner
            .active_errors
            .get()
            .iter()
            .flatten()
            .cloned()
            .collect()
    }

    pub fn field_label(&self, field: &str) -> Option<&str> {
        self.inner
            .fields
            .iter()
            .find(|item| item.name == field)
            .map(|item| item.label.as_str())
    }

    pub fn layout(&self) -> &Form {
        &self.inner.layout
    }

    pub fn into_layout(self) -> Form {
        self.inner.layout.clone()
    }

    /// 校验整表；失败时尽力把焦点登记到首个错误字段。
    pub fn submit(&self) -> Result<Values, Vec<FieldError>> {
        let result = self.validate();
        if result.is_err() {
            let _ = self.focus_first_error();
        }
        result
    }

    /// 把焦点登记到首个错误字段；当前无错误时返回 `Ok(None)`。
    pub fn focus_first_error(&self) -> Result<Option<String>, FocusHandleError> {
        let Some(error) = self.errors().into_iter().next() else {
            return Ok(None);
        };
        let field = error.field().to_string();
        let handle = self
            .inner
            .focus_handles
            .borrow()
            .get(&field)
            .cloned()
            .ok_or(FocusHandleError::Unbound)?;
        handle.focus()?;
        Ok(Some(field))
    }

    pub(crate) fn has_field(&self, field: &str) -> bool {
        self.inner.fields.iter().any(|item| item.name == field)
    }

    pub(crate) fn field_is_required(&self, field: &str) -> bool {
        self.inner
            .fields
            .iter()
            .find(|item| item.name == field)
            .is_some_and(|item| {
                item.rules
                    .iter()
                    .any(|rule| matches!(rule, FieldRule::Required(_)))
            })
    }

    pub(crate) fn sync_text_value(&self, field: &str, value: &str) -> bool {
        let Some(index) = self.inner.fields.iter().position(|item| item.name == field) else {
            return false;
        };
        if self.inner.values.borrow()[index].is_text(value) {
            return true;
        }
        self.set_value(field, value.to_string())
    }

    pub(crate) fn sync_typed_value<T>(&self, field: &str, value: &T) -> bool
    where
        T: Clone + PartialEq + Send + Sync + IntoFormValue<Stored = T> + 'static,
    {
        let Some(index) = self.inner.fields.iter().position(|item| item.name == field) else {
            return false;
        };
        if self.inner.values.borrow()[index].is_value(value) {
            return true;
        }
        self.set_value(field, value.clone())
    }

    pub(crate) fn focus_handle_for(&self, field: &str) -> Option<FocusHandle> {
        if !self.has_field(field) {
            return None;
        }
        let mut handles = self.inner.focus_handles.borrow_mut();
        Some(handles.entry(field.to_string()).or_default().clone())
    }

    fn publish_errors(&self, errors: Vec<Option<FieldError>>) {
        if self.inner.active_errors.get_untracked() != errors {
            self.inner.active_errors.set(errors);
        }
    }
}

impl Form {
    /// 开始声明应用侧字段校验模型。
    pub fn field(self, name: impl Into<String>, label: impl Into<String>) -> FormBuilder {
        FormBuilder::new(self, name, label)
    }
}

fn values_from_fields(fields: &[FormField], values: &[StoredValue]) -> Values {
    let entries = fields
        .iter()
        .zip(values)
        .map(|(field, value)| (field.name.clone(), Arc::clone(&value.typed)))
        .collect();
    Values { entries }
}

fn dependent_indices(fields: &[FormField], changed: usize) -> Vec<usize> {
    let mut visited = vec![false; fields.len()];
    visited[changed] = true;
    let mut frontier = vec![fields[changed].name.as_str()];
    let mut dependents = Vec::new();

    while let Some(source) = frontier.pop() {
        for (index, field) in fields.iter().enumerate() {
            if visited[index] || !field.depends_on(source) {
                continue;
            }
            visited[index] = true;
            dependents.push(index);
            frontier.push(field.name.as_str());
        }
    }
    dependents.sort_unstable();
    dependents
}

impl FormField {
    fn depends_on(&self, source: &str) -> bool {
        self.rules.iter().any(|rule| {
            matches!(
                rule,
                FieldRule::Dependency { field, .. } if field == source
            )
        })
    }
}

fn validate_field(field: &FormField, current: &StoredValue, values: &Values) -> Option<FieldError> {
    for rule in &field.rules {
        let failed_message = match rule {
            FieldRule::Required(message) if current.is_empty() => Some(message.clone()),
            FieldRule::Email(message) if !current.is_empty() && !is_valid_email(&current.text) => {
                Some(message.clone())
            }
            FieldRule::Range {
                start,
                end,
                message,
            } if !current.is_empty()
                && current
                    .text
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite() && *value >= *start && *value <= *end)
                    .is_none() =>
            {
                Some(message.clone())
            }
            FieldRule::Length {
                start,
                end,
                message,
            } if !current.is_empty()
                && !(*start..=*end).contains(&current.text.chars().count()) =>
            {
                Some(message.clone())
            }
            FieldRule::Pattern {
                matcher: None,
                message,
            } => Some(message.clone()),
            FieldRule::Pattern {
                matcher: Some(matcher),
                message,
            } if !current.is_empty() && !matcher.is_match(&current.text) => Some(message.clone()),
            FieldRule::Custom(validator) => validator(&current.text).err(),
            FieldRule::Dependency { validator, .. } => validator(&current.text, values).err(),
            _ => None,
        };
        if let Some(message) = failed_message {
            return Some(FieldError::new(&field.name, message));
        }
    }
    None
}

fn is_valid_email(value: &str) -> bool {
    if value.chars().any(char::is_whitespace) {
        return false;
    }
    let mut parts = value.split('@');
    let Some(local) = parts.next() else {
        return false;
    };
    let Some(domain) = parts.next() else {
        return false;
    };
    if parts.next().is_some()
        || local.is_empty()
        || domain.starts_with('.')
        || domain.ends_with('.')
    {
        return false;
    }
    domain.split('.').filter(|part| !part.is_empty()).count() >= 2
        && domain.split('.').all(|part| !part.is_empty())
}
