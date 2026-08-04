//! 应用侧表单值、规则与校验结果。

use std::any::Any;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::ops::RangeInclusive;
use std::rc::Rc;
use std::sync::Arc;

// 仅在使用方启用表单 pattern capability 时引入正则引擎。
#[cfg(feature = "form-pattern")]
// 正则类型只存放在同 feature 门控的字段规则中。
use regex::Regex;

use crate::draw::Color;
use crate::ui::{FocusHandle, FocusHandleError, State};

use crate::ui::form::form::Form;
use crate::ui::widgets::input::{Date, Time};

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

impl IntoFormValue for Date {
    type Stored = Self;

    fn into_form_value(self) -> Self::Stored {
        self
    }

    fn form_text(value: &Self::Stored) -> String {
        value.format()
    }
}

impl IntoFormValue for Time {
    type Stored = Self;

    fn into_form_value(self) -> Self::Stored {
        self
    }

    fn form_text(value: &Self::Stored) -> String {
        value.format()
    }
}

impl IntoFormValue for Color {
    type Stored = Self;

    fn into_form_value(self) -> Self::Stored {
        self
    }

    fn form_text(value: &Self::Stored) -> String {
        value.to_string()
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

    fn cloned<T>(&self) -> Option<T>
    where
        T: Clone + Send + Sync + 'static,
    {
        self.typed.as_ref().downcast_ref::<T>().cloned()
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

impl Trigger {
    /// 只在整表提交校验时激活；与 [`Trigger::OnSubmit`] 等价。
    #[allow(non_upper_case_globals)]
    pub const Submit: Self = Self::OnSubmit;
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
    // 关闭表单 pattern capability 时不保留正则规则变体。
    #[cfg(feature = "form-pattern")]
    // 启用后保存预编译 matcher 与用户错误文案。
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

mod builder;

pub use self::builder::{FormBuilder, FormInitialValues, FormModel};
