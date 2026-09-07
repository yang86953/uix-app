//! AOT 与动态执行共同采用的拥有型值、类型和失败语义。

use std::collections::BTreeMap;

/// 可跨宿主边界的值；集合复制不共享可变状态。
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Unit,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    Array(Vec<Value>),
    Record(BTreeMap<String, Value>),
    Optional(Option<Box<Value>>),
    Result(Result<Box<Value>, Box<Value>>),
}

/// 可移植值类型；整数固定为 i64，浮点固定为有限 f64。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Unit,
    Bool,
    Int,
    Float,
    String,
    Bytes,
    Array(Box<Type>),
    Record(BTreeMap<String, Type>),
    Optional(Box<Type>),
    Result(Box<Type>, Box<Type>),
}

impl Type {
    /// 校验完整值图，拒绝非有限浮点和错误的记录形状。
    pub fn accepts(&self, value: &Value) -> bool {
        match (self, value) {
            (Self::Unit, Value::Unit) | (Self::Bool, Value::Bool(_))
            | (Self::Int, Value::Int(_)) | (Self::String, Value::String(_))
            | (Self::Bytes, Value::Bytes(_)) => true,
            (Self::Float, Value::Float(v)) => v.is_finite(),
            (Self::Array(t), Value::Array(v)) => v.iter().all(|v| t.accepts(v)),
            (Self::Record(t), Value::Record(v)) => t.len() == v.len()
                && t.iter().all(|(k, t)| v.get(k).is_some_and(|v| t.accepts(v))),
            (Self::Optional(_), Value::Optional(None)) => true,
            (Self::Optional(t), Value::Optional(Some(v))) => t.accepts(v),
            (Self::Result(ok, _), Value::Result(Ok(v))) => ok.accepts(v),
            (Self::Result(_, err), Value::Result(Err(v))) => err.accepts(v),
            _ => false,
        }
    }
}

/// 运行失败的可观察类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Argument, Type, Arithmetic, Bounds, Quota, Cancelled, Timeout,
    CapabilityDenied, HostFailure, UnknownCommand, Conflict, Closed, InvalidModule,
}

/// 原始 UIX 来源位置；生成和动态执行保持同一位置。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Location {
    pub source: String,
    pub line: usize,
    pub column: usize,
}

/// 运行失败。调用方负责消费或转交，不自动记录领域数据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeError {
    pub kind: ErrorKind,
    pub message: String,
    pub location: Location,
}

impl RuntimeError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self { kind, message: message.into(), location: Location::default() }
    }
    pub fn at(mut self, location: &Location) -> Self {
        if self.location.source.is_empty() { self.location = location.clone(); }
        self
    }
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}: {:?}: {}", self.location.source,
            self.location.line, self.location.column, self.kind, self.message)
    }
}
impl std::error::Error for RuntimeError {}

pub type RuntimeResult<T> = Result<T, RuntimeError>;

impl Value {
    pub fn as_bool(&self) -> RuntimeResult<bool> {
        match self { Self::Bool(v) => Ok(*v), _ => Err(type_error("需要 Bool")) }
    }
    pub fn as_int(&self) -> RuntimeResult<i64> {
        match self { Self::Int(v) => Ok(*v), _ => Err(type_error("需要 Int")) }
    }
    pub fn as_str(&self) -> RuntimeResult<&str> {
        match self { Self::String(v) => Ok(v), _ => Err(type_error("需要 String")) }
    }
    pub fn as_array(&self) -> RuntimeResult<&[Value]> {
        match self { Self::Array(v) => Ok(v), _ => Err(type_error("需要 Array")) }
    }
    /// 展示只支持标量；不隐式序列化记录或集合。
    pub fn display_text(&self) -> RuntimeResult<String> {
        match self {
            Self::Unit => Ok(String::new()),
            Self::Bool(v) => Ok(v.to_string()),
            Self::Int(v) => Ok(v.to_string()),
            Self::Float(v) if v.is_finite() => Ok(v.to_string()),
            Self::String(v) => Ok(v.clone()),
            _ => Err(type_error("展示需要标量值")),
        }
    }
    /// 检查有界值图；字节计数包含集合槽位，避免大量空值逃过预算。
    pub fn validate_budget(&self, max_bytes: usize, max_items: usize) -> RuntimeResult<()> {
        let mut pending = vec![(self, 0usize)];
        let mut bytes = 0usize;
        let mut items = 0usize;
        while let Some((value, depth)) = pending.pop() {
            items = items.saturating_add(1);
            bytes = bytes.saturating_add(std::mem::size_of::<Value>());
            if depth > 64 || items > max_items { return Err(quota_error()); }
            match value {
                Self::String(v) => bytes = bytes.saturating_add(v.len()),
                Self::Bytes(v) => bytes = bytes.saturating_add(v.len()),
                Self::Float(v) if !v.is_finite() => return Err(type_error("浮点必须有限")),
                Self::Array(v) => {
                    if v.len() > max_items.saturating_sub(items) { return Err(quota_error()); }
                    pending.extend(v.iter().map(|v| (v, depth + 1)));
                }
                Self::Record(v) => {
                    if v.len() > max_items.saturating_sub(items) { return Err(quota_error()); }
                    for (k, v) in v {
                        bytes = bytes.saturating_add(k.len());
                        pending.push((v, depth + 1));
                    }
                }
                Self::Optional(Some(v)) | Self::Result(Ok(v)) | Self::Result(Err(v)) => pending.push((v, depth + 1)),
                _ => {}
            }
            if bytes > max_bytes { return Err(quota_error()); }
        }
        Ok(())
    }
}

pub(crate) fn type_error(message: &str) -> RuntimeError { RuntimeError::new(ErrorKind::Type, message) }
pub(crate) fn quota_error() -> RuntimeError { RuntimeError::new(ErrorKind::Quota, "应用模块资源配额耗尽") }

/// 两条执行路径共用的运算定义。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binary { Add, Subtract, Multiply, Divide, Remainder, Equal, NotEqual, Less, LessEqual, Greater, GreaterEqual, And, Or }

/// 运算在求值顺序确定后执行；短路由执行路径在读取右侧前处理。
pub fn binary(op: Binary, left: Value, right: Value) -> RuntimeResult<Value> {
    use Binary::*;
    let arithmetic = || RuntimeError::new(ErrorKind::Arithmetic, "整数溢出、除零或非有限浮点结果");
    if matches!(op, Equal | NotEqual) {
        let equal = left == right;
        return Ok(Value::Bool(if op == Equal { equal } else { !equal }));
    }
    match (left, right) {
        (Value::Int(a), Value::Int(b)) => match op {
            Add => a.checked_add(b).map(Value::Int).ok_or_else(arithmetic),
            Subtract => a.checked_sub(b).map(Value::Int).ok_or_else(arithmetic),
            Multiply => a.checked_mul(b).map(Value::Int).ok_or_else(arithmetic),
            Divide => a.checked_div(b).map(Value::Int).ok_or_else(arithmetic),
            Remainder => a.checked_rem(b).map(Value::Int).ok_or_else(arithmetic),
            Less => Ok(Value::Bool(a < b)), LessEqual => Ok(Value::Bool(a <= b)),
            Greater => Ok(Value::Bool(a > b)), GreaterEqual => Ok(Value::Bool(a >= b)),
            _ => Err(type_error("整数不支持该操作")),
        },
        (Value::Float(a), Value::Float(b)) => {
            if !a.is_finite() || !b.is_finite() { return Err(arithmetic()); }
            let v = match op {
                Add => a + b, Subtract => a - b, Multiply => a * b,
                Divide => a / b, Remainder => a % b,
                Less => return Ok(Value::Bool(a < b)), LessEqual => return Ok(Value::Bool(a <= b)),
                Greater => return Ok(Value::Bool(a > b)), GreaterEqual => return Ok(Value::Bool(a >= b)),
                _ => return Err(type_error("浮点不支持该操作")),
            };
            if v.is_finite() { Ok(Value::Float(v)) } else { Err(arithmetic()) }
        }
        (Value::String(a), Value::String(b)) => match op {
            Add => Ok(Value::String(a + &b)),
            Less => Ok(Value::Bool(a < b)), LessEqual => Ok(Value::Bool(a <= b)),
            Greater => Ok(Value::Bool(a > b)), GreaterEqual => Ok(Value::Bool(a >= b)),
            _ => Err(type_error("字符串不支持该操作")),
        },
        (Value::Bool(a), Value::Bool(b)) => match op {
            And => Ok(Value::Bool(a && b)), Or => Ok(Value::Bool(a || b)),
            _ => Err(type_error("布尔值不支持该操作")),
        },
        _ => Err(type_error("操作数类型不一致")),
    }
}
