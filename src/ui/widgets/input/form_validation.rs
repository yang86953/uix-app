//! 应用侧表单值、规则与校验结果。

use std::any::Any;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::ops::RangeInclusive;
use std::sync::Arc;

use regex::Regex;

use super::form::Form;

/// 把公开默认值转换成可保留具体类型的表单值。
pub trait IntoFormValue {
    type Stored: ToString + Send + Sync + 'static;

    fn into_form_value(self) -> Self::Stored;
}

impl IntoFormValue for &str {
    type Stored = String;

    fn into_form_value(self) -> Self::Stored {
        self.to_string()
    }
}

impl IntoFormValue for &String {
    type Stored = String;

    fn into_form_value(self) -> Self::Stored {
        self.clone()
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
            }
        )+
    };
}

impl_identity_form_value!(
    String, bool, char, i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64,
);

#[derive(Clone)]
struct StoredValue {
    typed: Arc<dyn Any + Send + Sync>,
    text: String,
}

impl StoredValue {
    fn new<V: IntoFormValue>(value: V) -> Self {
        let stored = value.into_form_value();
        let text = stored.to_string();
        Self {
            typed: Arc::new(stored),
            text,
        }
    }

    fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
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

type CustomValidator = Box<dyn Fn(&str) -> Result<(), String> + 'static>;

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
}

struct FormField {
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
            .push(FieldRule::Custom(Box::new(validator)));
        self
    }

    /// 设置当前字段的规则激活时机。
    pub fn validate_trigger(mut self, trigger: Trigger) -> Self {
        self.current.trigger = trigger;
        self
    }

    /// 完成声明；同名字段以最后一次声明为准。
    pub fn build(mut self) -> FormModel {
        self.fields.push(self.current);
        let mut unique = Vec::<FormField>::with_capacity(self.fields.len());
        for field in self.fields {
            unique.retain(|existing| existing.name != field.name);
            unique.push(field);
        }
        FormModel {
            layout: self.layout,
            active_errors: RefCell::new(vec![None; unique.len()]),
            fields: unique,
        }
    }
}

/// 应用侧表单模型；自定义校验闭包不会进入 widget component。
pub struct FormModel {
    layout: Form,
    fields: Vec<FormField>,
    active_errors: RefCell<Vec<Option<FieldError>>>,
}

impl FormModel {
    /// 校验全部字段；每个字段返回首个错误，字段间按声明顺序收集。
    pub fn validate(&self) -> Result<Values, Vec<FieldError>> {
        let field_errors: Vec<Option<FieldError>> =
            self.fields.iter().map(validate_field).collect();
        let errors = field_errors.iter().flatten().cloned().collect::<Vec<_>>();
        *self.active_errors.borrow_mut() = field_errors;
        if !errors.is_empty() {
            return Err(errors);
        }

        let entries = self
            .fields
            .iter()
            .map(|field| (field.name.clone(), Arc::clone(&field.value.typed)))
            .collect();
        Ok(Values { entries })
    }

    /// 更新字段值；字段不存在时返回 `false`。
    pub fn set_value<V: IntoFormValue>(&mut self, field: &str, value: V) -> bool {
        let Some(index) = self.fields.iter().position(|item| item.name == field) else {
            return false;
        };
        self.fields[index].value = StoredValue::new(value);
        let error = (self.fields[index].trigger == Trigger::OnChange)
            .then(|| validate_field(&self.fields[index]))
            .flatten();
        self.active_errors.borrow_mut()[index] = error;
        true
    }

    /// 通知字段失焦；仅 `OnBlur` 字段会在此时执行规则。
    pub fn blur(&mut self, field: &str) -> bool {
        let Some(index) = self.fields.iter().position(|item| item.name == field) else {
            return false;
        };
        if self.fields[index].trigger == Trigger::OnBlur {
            self.active_errors.borrow_mut()[index] = validate_field(&self.fields[index]);
        }
        true
    }

    /// 返回当前已激活的字段错误。
    pub fn field_error(&self, field: &str) -> Option<FieldError> {
        let index = self.fields.iter().position(|item| item.name == field)?;
        self.active_errors.borrow()[index].clone()
    }

    /// 按字段声明顺序返回当前已激活的错误。
    pub fn errors(&self) -> Vec<FieldError> {
        self.active_errors
            .borrow()
            .iter()
            .flatten()
            .cloned()
            .collect()
    }

    pub fn field_label(&self, field: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|item| item.name == field)
            .map(|item| item.label.as_str())
    }

    pub fn layout(&self) -> &Form {
        &self.layout
    }

    pub fn into_layout(self) -> Form {
        self.layout
    }
}

impl Form {
    /// 开始声明应用侧字段校验模型。
    pub fn field(self, name: impl Into<String>, label: impl Into<String>) -> FormBuilder {
        FormBuilder::new(self, name, label)
    }
}

fn validate_field(field: &FormField) -> Option<FieldError> {
    for rule in &field.rules {
        let failed_message = match rule {
            FieldRule::Required(message) if field.value.is_empty() => Some(message.clone()),
            FieldRule::Email(message)
                if !field.value.is_empty() && !is_valid_email(&field.value.text) =>
            {
                Some(message.clone())
            }
            FieldRule::Range {
                start,
                end,
                message,
            } if !field.value.is_empty()
                && field
                    .value
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
            } if !field.value.is_empty()
                && !(*start..=*end).contains(&field.value.text.chars().count()) =>
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
            } if !field.value.is_empty() && !matcher.is_match(&field.value.text) => {
                Some(message.clone())
            }
            FieldRule::Custom(validator) => validator(&field.value.text).err(),
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
