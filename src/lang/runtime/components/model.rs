use super::*;

pub type BindingId = usize;
pub type FunctionId = usize;
pub type ComponentId = usize;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExportKey {
    pub package: String,
    pub name: String,
}
impl ExportKey {
    pub fn new(package: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            package: package.into(),
            name: name.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    Component(ComponentId),
    Function(FunctionId),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingKind {
    Input,
    State,
    Parameter,
    Local,
    Function(FunctionId),
}
#[derive(Debug, Clone)]
pub struct Binding {
    pub name: String,
    pub ty: Type,
    pub owner: Owner,
    pub kind: BindingKind,
}

/// 源码中一个函数、默认值或初始化体。Native 不携带 AST/IR。
#[derive(Clone)]
pub enum Body {
    Native(for<'a, 'env> fn(&mut Frame<'a, 'env>) -> RuntimeResult<Value>),
    #[cfg(any(feature = "lang-build", feature = "uix-dynamic"))]
    Dynamic(Arc<[ir::Statement]>),
}
impl std::fmt::Debug for Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Native(_) => f.write_str("Native"),
            #[cfg(any(feature = "lang-build", feature = "uix-dynamic"))]
            Self::Dynamic(body) => f.debug_tuple("Dynamic").field(body).finish(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Function {
    pub signature: FunctionSignature,
    pub parameters: Vec<BindingId>,
    pub defaults: Vec<Option<Body>>,
    /// 已闭包提升的局部值捕获；组件输入/state 由 owner 实时读取。
    pub captures: Vec<BindingId>,
    pub component: Option<ComponentId>,
    pub body: Body,
    pub location: Location,
}

#[derive(Debug, Clone)]
pub struct Component {
    /// 来源及导出名用于诊断与后续显式替换，不把函数/变量位置当跨版本身份。
    pub name: String,
    pub signature: ComponentSignature,
    pub inputs: Vec<BindingId>,
    pub defaults: Vec<Option<Body>>,
    pub states: Vec<(BindingId, Body)>,
    pub render: FunctionId,
    pub location: Location,
}

#[derive(Debug, Clone)]
pub struct Program {
    pub components: Vec<Component>,
    pub functions: Vec<Function>,
    pub bindings: Vec<Binding>,
    pub natives: BTreeMap<ExportKey, NativeExport>,
    pub entry: ComponentId,
}

/// 宿主函数不会得到 UI owner 的可变引用，不可在宿主回调中重入同一事务。
pub type NativeCall = Arc<dyn Fn(&[Value]) -> RuntimeResult<Value> + Send + Sync>;
#[derive(Clone)]
pub enum NativeBinding {
    Function {
        signature: FunctionSignature,
        call: NativeCall,
    },
    /// 非 GUI 的投影合同；真实 UI Adapter 必须从实际原生构造器生成此接口。
    Component(ComponentSignature),
    Type(Type),
}
impl NativeBinding {
    pub fn interface(&self) -> NativeExport {
        match self {
            Self::Function { signature, .. } => NativeExport::Function(signature.clone()),
            Self::Component(signature) => NativeExport::Component(signature.clone()),
            Self::Type(ty) => NativeExport::Type(ty.clone()),
        }
    }
}
pub type NativeBindings = BTreeMap<ExportKey, NativeBinding>;

#[derive(Debug, Clone)]
pub struct ComponentLimits {
    pub evaluation: Limits,
    pub instances: usize,
    pub nodes: usize,
    pub events: usize,
}
impl Default for ComponentLimits {
    fn default() -> Self {
        Self {
            evaluation: Limits::default(),
            instances: 4096,
            nodes: 4096,
            events: 16384,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct InstanceId(pub(crate) u64);
impl InstanceId {
    pub fn value(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct EventToken {
    pub(crate) engine: u64,
    pub(crate) revision: u64,
    pub(crate) index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Key {
    Int(i64),
    String(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum IdentityStep {
    Element {
        site: usize,
        key: Option<Key>,
        position: usize,
    },
    Slot(String),
    Index(usize),
}
pub type Identity = Vec<IdentityStep>;

#[derive(Debug, Clone)]
pub enum ProjectedValue {
    Data(Arc<DataValue>),
    Event(EventToken),
    View(Vec<NativeNode>),
    Array(Vec<ProjectedValue>),
    Record(BTreeMap<String, ProjectedValue>),
}

#[derive(Debug, Clone)]
pub struct NativeNode {
    pub identity: Identity,
    pub export: ExportKey,
    pub properties: BTreeMap<String, ProjectedValue>,
}
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub revision: u64,
    pub roots: Vec<NativeNode>,
    pub instances: usize,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateStats {
    pub rendered_instances: usize,
    pub reused_instances: usize,
    pub unmounted_instances: usize,
}
