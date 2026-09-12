//! 脚本值表示、闭包与词法环境。
//!
//! 值是浅共享的引用图（`Rc`）；pair / 字符串 / 向量 / bytevector / record /
//! 参数对象 / promise / 端口经 `RefCell` 提供标准要求的可变操作。所有可
//! 能参与引用环的堆对象在构造时登记进引擎堆表，由标记-清扫回收；分配
//! 记账统一由引擎在构造入口执行。
//!
//! 环必须经过至少一个可变容器（pair / 向量 / record / 环境 / 参数对象 /
//! promise）：不可变对象（闭包、continuation、宏、error 对象、多值载体）
//! 的字段在构造时固定，引用环必含可清空节点，回收据此断环。

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use num_bigint::BigInt;
use num_rational::BigRational;

use super::error::SchemeError;
use super::eval::{ContinuationSnapshot, ControlOp};
use super::syntax_rules::MacroTransformer;
use super::number::Number;
use super::SchemeEngine;

/// M1 原语函数指针；需要引擎以完成分配记账与用户过程调用。
pub type PrimitiveFn = fn(&mut SchemeEngine, &[Value]) -> Result<Value, SchemeError>;

/// 能力注入宿主函数的实现体；注入方自带状态。
///
/// 回调收到引擎引用与实参：引擎只允许用于构造返回值（`new_pair` 等
/// 登记构造器），不得触发求值或修改注册状态；实参在回调返回后不再被
/// 引用。此纪律由桥接层（extensions Module）唯一实现保证。
pub type HostInvoke =
    Arc<dyn Fn(&mut SchemeEngine, &[Value]) -> Result<Value, SchemeError> + Send + Sync>;

/// 按插件清单 `capabilities` 声明并注入的宿主函数。
pub struct HostFunction {
    name: String,
    invoke: HostInvoke,
}

const MAX_CAPABILITY_NAME_BYTES: usize = 128;

impl HostFunction {
    /// 构造能力名受控的宿主函数；名称与清单声明使用同一字符集。
    pub fn new(name: impl Into<String>, invoke: HostInvoke) -> Result<Self, SchemeError> {
        let name = name.into();
        if name.is_empty()
            || name.len() > MAX_CAPABILITY_NAME_BYTES
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'.' | b'_'))
        {
            return Err(SchemeError::InvalidCapabilityName { name });
        }
        Ok(Self { name, invoke })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn invoke(
        &self,
        engine: &mut SchemeEngine,
        arguments: &[Value],
    ) -> Result<Value, SchemeError> {
        (self.invoke)(engine, arguments)
    }
}

impl fmt::Debug for HostFunction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("HostFunction").field("name", &self.name).finish()
    }
}

/// 一对单元；可变性支持 `set-car!` / `set-cdr!`。
pub struct Pair {
    pub(crate) car: Value,
    pub(crate) cdr: Value,
}

/// `define-record-type` 生成的记录实例；类型名与字段按声明序。
pub struct Record {
    type_name: Rc<str>,
    fields: Vec<Value>,
}

impl Record {
    pub(crate) fn new(type_name: Rc<str>, fields: Vec<Value>) -> Self {
        Self { type_name, fields }
    }

    pub(crate) fn type_name(&self) -> &Rc<str> {
        &self.type_name
    }

    pub(crate) fn fields(&self) -> &[Value] {
        &self.fields
    }

    pub(crate) fn set_field(&mut self, index: usize, value: Value) -> bool {
        self.fields.get_mut(index).is_some_and(|slot| {
            *slot = value;
            true
        })
    }
}

/// 脚本闭包；捕获定义处词法环境。
pub struct Closure {
    pub(crate) parameters: Vec<Rc<str>>,
    pub(crate) rest: Option<Rc<str>>,
    pub(crate) body: Rc<[Value]>,
    pub(crate) env: Env,
}

/// 词法环境节点；父子链随闭包捕获。
pub(crate) struct EnvNode {
    pub(crate) bindings: BTreeMap<Rc<str>, Value>,
    pub(crate) parent: Option<Env>,
}

pub(crate) type Env = Rc<RefCell<EnvNode>>;

/// `raise` / `error` 抛出的异常对象；不可变。
pub struct ErrorObject {
    message: String,
    irritants: Vec<Value>,
    kind: ErrorKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// 脚本 `error` 过程。
    User,
    /// 文件形态操作（受限宿主环境映射自宿主端口失败）。
    File,
    /// 读取器失败。
    Read,
    /// 其它脚本 `raise`。
    Raised,
}

impl ErrorObject {
    pub(crate) fn new(message: String, irritants: Vec<Value>, kind: ErrorKind) -> Self {
        Self {
            message,
            irritants,
            kind,
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub(crate) fn irritants(&self) -> &[Value] {
        &self.irritants
    }

    pub(crate) fn kind(&self) -> ErrorKind {
        self.kind
    }
}

/// `make-parameter` 的参数对象；converter 在值写入时应用。
pub struct ParameterState {
    pub(crate) converter: Option<Value>,
    pub(crate) current: Value,
}

/// `(scheme lazy)` 的 promise。
pub enum PromiseState {
    Pending(Value),
    Forced(Value),
}

/// 内存字符串端口；文件端口属受限环境，不提供。
pub enum PortState {
    Input {
        text: String,
        position: usize,
        closed: bool,
    },
    Output {
        text: String,
        closed: bool,
    },
}

fn lookup_exact(env: &Env, name: &str) -> Option<Value> {
    let mut current = env.clone();
    loop {
        let found = current.borrow().bindings.get(name).cloned();
        if let Some(value) = found {
            return Some(value);
        }
        let parent = current.borrow().parent.clone();
        current = parent?;
    }
}

impl EnvNode {
    /// 创建以全局环境为根的子环境。
    pub(crate) fn child(parent: &Env) -> Env {
        Rc::new(RefCell::new(Self {
            bindings: BTreeMap::new(),
            parent: Some(parent.clone()),
        }))
    }

    /// 创建无父环境；仅引擎构造全局环境时使用。
    pub(crate) fn root() -> Env {
        Rc::new(RefCell::new(Self {
            bindings: BTreeMap::new(),
            parent: None,
        }))
    }

    /// 在当前帧定义或覆盖绑定。
    pub(crate) fn define(env: &Env, name: &Rc<str>, value: Value) {
        env.borrow_mut().bindings.insert(name.clone(), value);
    }

    /// 沿词法链查找绑定值。
    ///
    /// 宏展开引入的重命名标识（`name^N`）查找失败时回退到原始名：
    /// 宏体内的自由标识解析到宏定义环境的绑定，实现卫生语义。
    pub(crate) fn lookup(env: &Env, name: &str) -> Option<Value> {
        if let Some(value) = lookup_exact(env, name) {
            return Some(value);
        }
        match name.split_once('^') {
            // 重命名标识回退在定义环境（词法链根）解析，不受使用处
            // shadow 影响；这是宏自由标识卫生语义的实现。
            Some((base, _)) if !base.is_empty() => {
                let mut root = env.clone();
                loop {
                    let parent = root.borrow().parent.clone();
                    match parent {
                        Some(parent) => root = parent,
                        None => break,
                    }
                }
                lookup_exact(&root, base)
            }
            _ => None,
        }
    }

    /// 沿词法链修改既有绑定；返回是否存在该绑定。
    pub(crate) fn set(env: &Env, name: &str, value: Value) -> bool {
        let mut current = env.clone();
        loop {
            let mut frame = current.borrow_mut();
            if frame.bindings.contains_key(name) {
                frame.bindings.insert(Rc::from(name), value);
                return true;
            }
            let parent = frame.parent.clone();
            drop(frame);
            match parent {
                Some(parent) => current = parent,
                None => return false,
            }
        }
    }
}

/// 脚本值；`Unspecified` 用作 define / set! 等的未指定返回值。
///
/// 数值塔分四个 variant；规范化由 `number` 保证——同值 exact 数值
/// 永远折叠到同一 variant，eqv? 因此可以按 variant 与值直接比较。
#[derive(Clone)]
pub enum Value {
    Null,
    Unspecified,
    Eof,
    Bool(bool),
    Fixnum(i64),
    Bignum(Rc<BigInt>),
    Rational(Rc<BigRational>),
    Flonum(f64),
    Char(char),
    Symbol(Rc<str>),
    String(Rc<RefCell<String>>),
    Pair(Rc<RefCell<Pair>>),
    Vector(Rc<RefCell<Vec<Value>>>),
    Bytevector(Rc<RefCell<Vec<u8>>>),
    Record(Rc<RefCell<Record>>),
    Closure(Rc<Closure>),
    Primitive(PrimitiveFn),
    /// 需要机器状态的控制原语（call/cc、dynamic-wind、values 等）。
    Control(ControlOp),
    /// 一等 continuation：帧栈与 wind 链的不可变快照。
    Continuation(Rc<ContinuationSnapshot>),
    /// `syntax-rules` 宏变换器。
    Macro(Rc<MacroTransformer>),
    Host(Rc<HostFunction>),
    /// `raise` / `error` 的异常对象。
    ErrorObject(Rc<ErrorObject>),
    /// `values` 的多值载体。
    Values(Rc<Vec<Value>>),
    /// `make-parameter` 的参数对象。
    Parameter(Rc<RefCell<ParameterState>>),
    /// `(scheme lazy)` promise。
    Promise(Rc<RefCell<PromiseState>>),
    /// 内存字符串端口。
    Port(Rc<RefCell<PortState>>),
}

impl Value {
    /// 判断是否为布尔假；仅 `#f` 为假。
    pub fn is_false(&self) -> bool {
        matches!(self, Value::Bool(false))
    }

    pub fn is_number(&self) -> bool {
        matches!(
            self,
            Value::Fixnum(_) | Value::Bignum(_) | Value::Rational(_) | Value::Flonum(_)
        )
    }

    pub fn is_list(&self) -> bool {
        let mut current = self.clone();
        loop {
            match current {
                Value::Null => return true,
                Value::Pair(pair) => {
                    current = pair.borrow().cdr.clone();
                }
                _ => return false,
            }
        }
    }
}

/// 多值在单值上下文的投影：取首值，空载体为未指定值。
pub(crate) fn single_value(value: Value) -> Value {
    match value {
        Value::Values(items) => items.first().cloned().unwrap_or(Value::Unspecified),
        other => other,
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Null, Value::Null)
            | (Value::Unspecified, Value::Unspecified)
            | (Value::Eof, Value::Eof) => true,
            (Value::Bool(left), Value::Bool(right)) => left == right,
            (Value::Symbol(left), Value::Symbol(right)) => left == right,
            (Value::Char(left), Value::Char(right)) => left == right,
            // 与 eqv? 一致：数值要求同一精确层；跨层相等交给 equal? 与 =。
            (Value::Fixnum(left), Value::Fixnum(right)) => left == right,
            (Value::Bignum(left), Value::Bignum(right)) => left == right,
            (Value::Rational(left), Value::Rational(right)) => left == right,
            (Value::Flonum(left), Value::Flonum(right)) => left.to_bits() == right.to_bits(),
            (Value::String(left), Value::String(right)) => Rc::ptr_eq(left, right),
            (Value::Pair(left), Value::Pair(right)) => Rc::ptr_eq(left, right),
            (Value::Vector(left), Value::Vector(right)) => Rc::ptr_eq(left, right),
            (Value::Bytevector(left), Value::Bytevector(right)) => Rc::ptr_eq(left, right),
            (Value::Record(left), Value::Record(right)) => Rc::ptr_eq(left, right),
            (Value::Closure(left), Value::Closure(right)) => Rc::ptr_eq(left, right),
            (Value::Primitive(left), Value::Primitive(right)) => {
                std::ptr::fn_addr_eq(*left, *right)
            }
            (Value::Control(left), Value::Control(right)) => left == right,
            (Value::Continuation(left), Value::Continuation(right)) => Rc::ptr_eq(left, right),
            (Value::Macro(left), Value::Macro(right)) => Rc::ptr_eq(left, right),
            (Value::Host(left), Value::Host(right)) => Rc::ptr_eq(left, right),
            (Value::ErrorObject(left), Value::ErrorObject(right)) => Rc::ptr_eq(left, right),
            (Value::Values(left), Value::Values(right)) => Rc::ptr_eq(left, right),
            (Value::Parameter(left), Value::Parameter(right)) => Rc::ptr_eq(left, right),
            (Value::Promise(left), Value::Promise(right)) => Rc::ptr_eq(left, right),
            (Value::Port(left), Value::Port(right)) => Rc::ptr_eq(left, right),
            _ => false,
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_write_string())
    }
}

const MAX_WRITTEN_BYTES: usize = 1_024;

impl Value {
    /// 返回 write 形式的有界表示；用于错误信息与诊断，不用于语言语义。
    pub fn to_write_string(&self) -> String {
        let mut sink = String::new();
        let mut budget = MAX_WRITTEN_BYTES;
        write_value(&mut sink, self, &mut budget, 0);
        sink
    }
}

fn write_value(sink: &mut String, value: &Value, budget: &mut usize, depth: u32) {
    if *budget == 0 || depth > 64 {
        if *budget > 0 {
            sink.push_str("...");
            *budget = 0;
        }
        return;
    }
    match value {
        Value::Null => push(sink, budget, "()"),
        Value::Unspecified => push(sink, budget, "#<unspecified>"),
        Value::Eof => push(sink, budget, "#<eof>"),
        Value::Bool(true) => push(sink, budget, "#t"),
        Value::Bool(false) => push(sink, budget, "#f"),
        Value::Fixnum(number) => push(sink, budget, &number.to_string()),
        Value::Bignum(number) => push(sink, budget, &number.to_string()),
        Value::Rational(number) => {
            let text = format!("{}/{}", number.numer(), number.denom());
            push(sink, budget, &text);
        }
        Value::Flonum(number) => {
            let text = Number::Flonum(*number).to_text();
            push(sink, budget, &text);
        }
        Value::Char(character) => {
            let text = match character {
                ' ' => "#\\space".to_string(),
                '\n' => "#\\newline".to_string(),
                '\t' => "#\\tab".to_string(),
                other => format!("#\\{other}"),
            };
            push(sink, budget, &text);
        }
        Value::Symbol(name) => push(sink, budget, name),
        Value::String(cell) => {
            let text = cell.borrow();
            let mut escaped = String::with_capacity(text.len() + 2);
            escaped.push('"');
            for character in text.chars() {
                match character {
                    '"' => escaped.push_str("\\\""),
                    '\\' => escaped.push_str("\\\\"),
                    '\n' => escaped.push_str("\\n"),
                    '\t' => escaped.push_str("\\t"),
                    '\r' => escaped.push_str("\\r"),
                    other => escaped.push(other),
                }
            }
            escaped.push('"');
            push(sink, budget, &escaped);
        }
        Value::Pair(pair) => {
            let pair = pair.borrow();
            push(sink, budget, "(");
            write_value(sink, &pair.car, budget, depth + 1);
            let mut tail = pair.cdr.clone();
            loop {
                if *budget == 0 {
                    push(sink, budget, " ...");
                    break;
                }
                match tail {
                    Value::Null => break,
                    Value::Pair(next) => {
                        let next = next.borrow();
                        push(sink, budget, " ");
                        write_value(sink, &next.car, budget, depth + 1);
                        tail = next.cdr.clone();
                    }
                    improper => {
                        push(sink, budget, " . ");
                        write_value(sink, &improper, budget, depth + 1);
                        break;
                    }
                }
            }
            push(sink, budget, ")");
        }
        Value::Vector(items) => {
            push(sink, budget, "#(");
            let items = items.borrow();
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    push(sink, budget, " ");
                }
                write_value(sink, item, budget, depth + 1);
            }
            push(sink, budget, ")");
        }
        Value::Bytevector(items) => {
            push(sink, budget, "#u8(");
            let items = items.borrow();
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    push(sink, budget, " ");
                }
                push(sink, budget, &item.to_string());
            }
            push(sink, budget, ")");
        }
        Value::Record(record) => {
            let record = record.borrow();
            push(sink, budget, &format!("#<record:{}>", record.type_name()));
            for field in record.fields() {
                push(sink, budget, " ");
                write_value(sink, field, budget, depth + 1);
            }
            push(sink, budget, ">");
        }
        Value::Closure(_) => push(sink, budget, "#<procedure>"),
        Value::Primitive(_) => push(sink, budget, "#<primitive>"),
        Value::Control(_) => push(sink, budget, "#<control>"),
        Value::Continuation(_) => push(sink, budget, "#<continuation>"),
        Value::Macro(_) => push(sink, budget, "#<macro>"),
        Value::Host(_) => push(sink, budget, "#<host>"),
        Value::ErrorObject(error) => {
            push(sink, budget, &format!("#<error:{}>", error.message()))
        }
        Value::Values(items) => {
            push(sink, budget, "#<values");
            for item in items.iter() {
                push(sink, budget, " ");
                write_value(sink, item, budget, depth + 1);
            }
            push(sink, budget, ">");
        }
        Value::Parameter(_) => push(sink, budget, "#<parameter>"),
        Value::Promise(_) => push(sink, budget, "#<promise>"),
        Value::Port(cell) => {
            let kind = match &*cell.borrow() {
                PortState::Input { .. } => "input",
                PortState::Output { .. } => "output",
            };
            push(sink, budget, &format!("#<{kind}-port>"));
        }
    }
}

fn push(sink: &mut String, budget: &mut usize, text: &str) {
    let remaining = *budget;
    if remaining == 0 {
        return;
    }
    if text.len() <= remaining {
        sink.push_str(text);
        *budget = remaining - text.len();
    } else {
        let cut = text
            .char_indices()
            .map(|(index, _)| index)
            .take_while(|index| *index <= remaining)
            .last()
            .unwrap_or(0);
        sink.push_str(&text[..cut]);
        *budget = 0;
    }
}

// ---------- 标记-清扫堆 ----------
//
// 标记遍历在引擎侧统一执行（需要同时标记 Value 与环境节点两类指针身份）；
// 本模块只提供槽位的指针身份、断环清空与字节估算。

/// 堆登记槽位；持有弱引用，对象自然释放后槽位失效。
pub(crate) enum HeapSlot {
    Pair(Weak<RefCell<Pair>>),
    String(Weak<RefCell<String>>),
    Vector(Weak<RefCell<Vec<Value>>>),
    Bytevector(Weak<RefCell<Vec<u8>>>),
    Record(Weak<RefCell<Record>>),
    Closure(Weak<Closure>),
    Env(Weak<RefCell<EnvNode>>),
    Continuation(Weak<ContinuationSnapshot>),
    Macro(Weak<MacroTransformer>),
    Parameter(Weak<RefCell<ParameterState>>),
    Promise(Weak<RefCell<PromiseState>>),
    Port(Weak<RefCell<PortState>>),
    ErrorObject(Weak<ErrorObject>),
    Values(Weak<Vec<Value>>),
}

impl HeapSlot {
    /// 槽位对象指针身份（0 表示已释放）。
    pub(crate) fn pointer(&self) -> Option<usize> {
        let pointer = match self {
            HeapSlot::Pair(weak) => weak.as_ptr() as usize,
            HeapSlot::String(weak) => weak.as_ptr() as usize,
            HeapSlot::Vector(weak) => weak.as_ptr() as usize,
            HeapSlot::Bytevector(weak) => weak.as_ptr() as usize,
            HeapSlot::Record(weak) => weak.as_ptr() as usize,
            HeapSlot::Closure(weak) => weak.as_ptr() as usize,
            HeapSlot::Env(weak) => weak.as_ptr() as usize,
            HeapSlot::Continuation(weak) => weak.as_ptr() as usize,
            HeapSlot::Macro(weak) => weak.as_ptr() as usize,
            HeapSlot::Parameter(weak) => weak.as_ptr() as usize,
            HeapSlot::Promise(weak) => weak.as_ptr() as usize,
            HeapSlot::Port(weak) => weak.as_ptr() as usize,
            HeapSlot::ErrorObject(weak) => weak.as_ptr() as usize,
            HeapSlot::Values(weak) => weak.as_ptr() as usize,
        };
        if pointer == 0 {
            None
        } else {
            Some(pointer)
        }
    }

    /// 清空内容以断开引用环；仅可变容器有实际效果。
    pub(crate) fn drain(&self) -> bool {
        match self {
            HeapSlot::Pair(weak) => {
                if let Some(cell) = weak.upgrade()
                    && let Ok(mut borrowed) = cell.try_borrow_mut()
                {
                    borrowed.car = Value::Unspecified;
                    borrowed.cdr = Value::Unspecified;
                    return true;
                }
                false
            }
            HeapSlot::Vector(weak) => {
                if let Some(cell) = weak.upgrade()
                    && let Ok(mut borrowed) = cell.try_borrow_mut()
                {
                    borrowed.clear();
                    return true;
                }
                false
            }
            HeapSlot::Record(weak) => {
                if let Some(cell) = weak.upgrade()
                    && let Ok(mut borrowed) = cell.try_borrow_mut()
                {
                    for field in borrowed.fields.iter_mut() {
                        *field = Value::Unspecified;
                    }
                    return true;
                }
                false
            }
            HeapSlot::Env(weak) => {
                if let Some(cell) = weak.upgrade()
                    && let Ok(mut borrowed) = cell.try_borrow_mut()
                {
                    borrowed.bindings.clear();
                    borrowed.parent = None;
                    return true;
                }
                false
            }
            HeapSlot::Parameter(weak) => {
                if let Some(cell) = weak.upgrade()
                    && let Ok(mut borrowed) = cell.try_borrow_mut()
                {
                    borrowed.converter = None;
                    borrowed.current = Value::Unspecified;
                    return true;
                }
                false
            }
            HeapSlot::Promise(weak) => {
                if let Some(cell) = weak.upgrade()
                    && let Ok(mut borrowed) = cell.try_borrow_mut()
                {
                    *borrowed = PromiseState::Forced(Value::Unspecified);
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    /// 估算存活字节（粗粒度资源计量，不追求精确）。
    pub(crate) fn approx_bytes(&self) -> usize {
        match self {
            HeapSlot::Pair(_) => std::mem::size_of::<Pair>(),
            HeapSlot::String(weak) => weak
                .upgrade()
                .map(|cell| cell.borrow().len() + std::mem::size_of::<String>())
                .unwrap_or(0),
            HeapSlot::Vector(weak) => weak
                .upgrade()
                .map(|cell| {
                    cell.borrow().len() * std::mem::size_of::<Value>()
                        + std::mem::size_of::<Vec<Value>>()
                })
                .unwrap_or(0),
            HeapSlot::Bytevector(weak) => weak
                .upgrade()
                .map(|cell| cell.borrow().len() + std::mem::size_of::<Vec<u8>>())
                .unwrap_or(0),
            HeapSlot::Record(weak) => weak
                .upgrade()
                .map(|cell| {
                    cell.borrow().fields().len() * std::mem::size_of::<Value>()
                        + std::mem::size_of::<Record>()
                })
                .unwrap_or(0),
            HeapSlot::Closure(weak) => weak
                .upgrade()
                .map(|closure| {
                    closure.parameters.len() * 16
                        + closure.body.len() * std::mem::size_of::<Value>()
                        + std::mem::size_of::<Closure>()
                })
                .unwrap_or(0),
            HeapSlot::Env(weak) => weak
                .upgrade()
                .map(|cell| {
                    cell.borrow().bindings.len()
                        * (16 + std::mem::size_of::<Value>())
                        + std::mem::size_of::<EnvNode>()
                })
                .unwrap_or(0),
            HeapSlot::Continuation(weak) => weak
                .upgrade()
                .map(|snapshot| {
                    (snapshot.frame_count() + snapshot.wind_count())
                        * std::mem::size_of::<Value>()
                        + std::mem::size_of::<ContinuationSnapshot>()
                })
                .unwrap_or(0),
            HeapSlot::Macro(weak) => weak
                .upgrade()
                .map(|transformer| transformer.approx_bytes())
                .unwrap_or(0),
            HeapSlot::Parameter(_) => std::mem::size_of::<ParameterState>(),
            HeapSlot::Promise(_) => std::mem::size_of::<PromiseState>(),
            HeapSlot::Port(weak) => weak
                .upgrade()
                .map(|cell| match &*cell.borrow() {
                    PortState::Input { text, .. } | PortState::Output { text, .. } => {
                        text.len() + std::mem::size_of::<PortState>()
                    }
                })
                .unwrap_or(0),
            HeapSlot::ErrorObject(weak) => weak
                .upgrade()
                .map(|error| {
                    error.message().len() + error.irritants().len() * std::mem::size_of::<Value>()
                })
                .unwrap_or(0),
            HeapSlot::Values(weak) => weak
                .upgrade()
                .map(|items| items.len() * std::mem::size_of::<Value>())
                .unwrap_or(0),
        }
    }
}

impl Value {
    /// 堆对象指针身份；非登记类型返回 None。
    pub(crate) fn heap_pointer(&self) -> Option<usize> {
        let pointer = match self {
            Value::String(cell) => Rc::as_ptr(cell) as usize,
            Value::Pair(cell) => Rc::as_ptr(cell) as usize,
            Value::Vector(cell) => Rc::as_ptr(cell) as usize,
            Value::Bytevector(cell) => Rc::as_ptr(cell) as usize,
            Value::Record(cell) => Rc::as_ptr(cell) as usize,
            Value::Closure(cell) => Rc::as_ptr(cell) as usize,
            Value::Continuation(cell) => Rc::as_ptr(cell) as usize,
            Value::Macro(cell) => Rc::as_ptr(cell) as usize,
            Value::Parameter(cell) => Rc::as_ptr(cell) as usize,
            Value::Promise(cell) => Rc::as_ptr(cell) as usize,
            Value::Port(cell) => Rc::as_ptr(cell) as usize,
            Value::ErrorObject(cell) => Rc::as_ptr(cell) as usize,
            Value::Values(cell) => Rc::as_ptr(cell) as usize,
            _ => return None,
        };
        Some(pointer)
    }
}

/// 环境节点的指针身份。
pub(crate) fn env_pointer(env: &Env) -> usize {
    Rc::as_ptr(env) as usize
}
