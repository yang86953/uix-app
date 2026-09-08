//! 编译器输出的可移植执行合同；AOT 和动态产物采用同一函数签名与来源。

use crate::{Binary, Location, RuntimeResult, Type, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

/// 模块内唯一的状态字段。
#[derive(Debug, Clone)]
pub struct StateField {
    pub name: String,
    pub ty: Type,
    pub initial: Value,
}

/// 宿主端口或函数的效果类别。纯函数不读取实例状态或调用宿主。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Effect {
    Pure,
    Query,
    Command,
}

/// 名称、类型与顺序形成完整调用合同。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub name: String,
    pub parameters: Vec<(String, Type)>,
    pub returns: Type,
    pub effect: Effect,
    pub asynchronous: bool,
}

/// 类型化表达式，不持有解析器或 Rust token。
#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub ty: Type,
    pub location: Location,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Literal(Value),
    Local(usize),
    State(usize),
    Unary {
        negate: bool,
        value: Box<Expr>,
    },
    Binary {
        op: Binary,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Conditional {
        condition: Box<Expr>,
        yes: Box<Expr>,
        no: Box<Expr>,
    },
    Array(Vec<Expr>),
    Record(Vec<(String, Expr)>),
    Member {
        value: Box<Expr>,
        name: String,
    },
    Index {
        value: Box<Expr>,
        index: Box<Expr>,
    },
    Call {
        name: String,
        arguments: Vec<Expr>,
    },
    Method {
        value: Box<Expr>,
        name: String,
        arguments: Vec<Expr>,
    },
    Map {
        value: Box<Expr>,
        slot: usize,
        body: Box<Expr>,
        filter: bool,
    },
}

#[derive(Debug, Clone)]
pub enum Statement {
    Local(usize, Expr),
    State(Vec<(usize, Expr)>),
    Evaluate(Expr),
    If(Expr, Vec<Statement>, Vec<Statement>),
    Return(Option<Expr>),
}

/// AOT 产物包含真正的 Rust 函数指针；动态产物才解释类型化语句。
#[derive(Clone)]
pub enum Body {
    Dynamic(Vec<Statement>),
    Native(for<'a, 'env> fn(&mut crate::Frame<'a, 'env>) -> RuntimeResult<Value>),
}
impl std::fmt::Debug for Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dynamic(v) => f.debug_tuple("Dynamic").field(v).finish(),
            Self::Native(_) => f.write_str("Native"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Function {
    pub signature: Signature,
    pub exported: bool,
    pub local_count: usize,
    pub body: Body,
    pub location: Location,
}

/// 不可变模块产物。实例拥有单独状态；产物本身可共享。
#[derive(Debug, Clone)]
pub struct Module {
    pub name: String,
    pub version: String,
    pub state_schema: u64,
    pub states: Vec<StateField>,
    pub functions: Vec<Function>,
    pub ports: Vec<Signature>,
    pub view: Option<ViewTemplate>,
    pub tasks: Vec<Task>,
    pub data_types: BTreeMap<String, Type>,
    pub dependencies: Vec<ModuleDependency>,
}

/// 链接实例的稳定归属；版本可变，别名、身份与 schema 决定状态兼容性。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleDependency {
    pub alias: String,
    pub name: String,
    pub version: String,
    pub state_schema: u64,
}

/// await 之间的同步片段；每段同样可由动态节点或原生 Rust 函数实现。
#[derive(Debug, Clone)]
pub struct Task {
    pub signature: Signature,
    pub exported: bool,
    pub local_count: usize,
    pub stages: Vec<TaskStage>,
}
#[derive(Debug, Clone)]
pub struct TaskStage {
    pub body: TaskBody,
    pub location: Location,
}

/// 控制流块不等于提交边界；只有 Await 和最终返回结束同步片段。
#[derive(Clone)]
pub enum TaskBody {
    Dynamic {
        statements: Vec<Statement>,
        exit: TaskExit,
    },
    Native(for<'a, 'env> fn(&mut crate::Frame<'a, 'env>) -> RuntimeResult<TaskStep>),
}
impl std::fmt::Debug for TaskBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dynamic { statements, exit } => f
                .debug_struct("Dynamic")
                .field("statements", statements)
                .field("exit", exit)
                .finish(),
            Self::Native(_) => f.write_str("Native"),
        }
    }
}
#[derive(Debug, Clone)]
pub enum TaskExit {
    Jump(usize),
    Branch {
        condition: Expr,
        yes: usize,
        no: usize,
    },
    Await {
        name: String,
        arguments: Vec<Expr>,
        next: usize,
        slot: usize,
    },
    TailCall {
        name: String,
        arguments: Vec<Expr>,
    },
    Finish,
}

/// 动态节点和原生片段共同返回的拥有型执行结果。
#[derive(Debug)]
pub enum TaskStep {
    Continue(usize),
    Await {
        name: String,
        arguments: Vec<Value>,
        next: usize,
        slot: usize,
    },
    TailCall {
        name: String,
        arguments: Vec<Value>,
    },
    Complete(Value),
}

/// 界面属性引用编译器生成的只读函数，事件引用私有命令。
#[derive(Debug, Clone)]
pub struct ViewTemplate {
    pub kind: ViewKind,
    pub key: String,
    pub properties: BTreeMap<String, String>,
    pub handler: Option<String>,
    pub children: Vec<ViewTemplate>,
    pub control: Option<ViewControl>,
}

/// 结构绑定只在业务提交时展开，不产生额外布局节点。
#[derive(Debug, Clone)]
pub enum ViewControl {
    If {
        condition: String,
        otherwise: Vec<ViewTemplate>,
    },
    For {
        items: String,
        key: String,
        indexed: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewKind {
    Column,
    Row,
    Text,
    Input,
    Button,
}

/// 执行线程生成的拥有型界面快照；布局和绘制不访问运行器。
#[derive(Debug, Clone, PartialEq)]
pub struct ViewSnapshot {
    pub kind: ViewKind,
    pub key: String,
    pub properties: BTreeMap<String, Value>,
    pub children: Vec<ViewSnapshot>,
}

/// 显式注入的宿主端口。回调必须在应用规定的时间内返回。
#[derive(Clone)]
pub struct HostPort {
    pub signature: Signature,
    pub callback: Arc<dyn Fn(&[Value]) -> RuntimeResult<Value> + Send + Sync>,
}

/// 宿主只授予声明且签名相符的端口；没有隐式服务发现。
pub type HostPorts = BTreeMap<String, HostPort>;
