use super::*;

/// 组件值图不可共享可变局部槽。普通数据复用框架数据值，回调和 UI 不伪装成数据。
#[derive(Debug, Clone)]
pub enum Value {
    Data(Arc<DataValue>),
    Function(Arc<Callback>),
    View(Arc<[Element]>),
    Array(Arc<[Value]>),
    Record(Arc<BTreeMap<String, Value>>),
}
impl Value {
    pub fn data(value: DataValue) -> Self {
        Self::Data(Arc::new(value))
    }
    pub fn unit() -> Self {
        Self::data(DataValue::Unit)
    }
    /// 引用已显式提供的原生函数；不授予未登记的宿主能力。
    pub fn native_function(key: ExportKey) -> Self {
        Self::Function(Arc::new(Callback {
            engine: 0,
            owner: None,
            target: CallbackTarget::Native(key),
            captures: BTreeMap::new(),
        }))
    }
    pub fn as_data(&self) -> RuntimeResult<&DataValue> {
        match self {
            Self::Data(value) => Ok(value),
            _ => Err(invalid("需要普通数据值")),
        }
    }
    pub fn array(values: Vec<Self>) -> Self {
        if values.iter().all(|value| matches!(value, Self::Data(_))) {
            Self::data(DataValue::Array(
                values
                    .into_iter()
                    .map(|value| {
                        let Self::Data(value) = value else {
                            unreachable!()
                        };
                        (*value).clone()
                    })
                    .collect(),
            ))
        } else {
            Self::Array(values.into())
        }
    }
    pub fn record(values: BTreeMap<String, Self>) -> Self {
        if values.values().all(|value| matches!(value, Self::Data(_))) {
            Self::data(DataValue::Record(
                values
                    .into_iter()
                    .map(|(name, value)| {
                        let Self::Data(value) = value else {
                            unreachable!()
                        };
                        (name, (*value).clone())
                    })
                    .collect(),
            ))
        } else {
            Self::Record(Arc::new(values))
        }
    }
    pub(crate) fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Data(a), Self::Data(b)) => Arc::ptr_eq(a, b) || a == b,
            (Self::Function(a), Self::Function(b)) => {
                Arc::ptr_eq(a, b)
                    || (a.engine == b.engine
                        && a.owner == b.owner
                        && a.target == b.target
                        && a.captures.len() == b.captures.len()
                        && a.captures
                            .iter()
                            .all(|(key, a)| b.captures.get(key).is_some_and(|b| a.same(b))))
            }
            (Self::View(a), Self::View(b)) => Arc::ptr_eq(a, b),
            (Self::Array(a), Self::Array(b)) => {
                Arc::ptr_eq(a, b)
                    || (a.len() == b.len() && a.iter().zip(b.iter()).all(|(a, b)| a.same(b)))
            }
            (Self::Record(a), Self::Record(b)) => {
                Arc::ptr_eq(a, b)
                    || (a.len() == b.len()
                        && a.iter()
                            .all(|(name, a)| b.get(name).is_some_and(|b| a.same(b))))
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Component(ComponentId),
    Native(ExportKey),
}
#[derive(Debug, Clone)]
pub struct Element {
    pub(crate) location: Location,
    pub(crate) engine: u64,
    pub(crate) owner: Option<InstanceId>,
    pub(crate) site: usize,
    pub(crate) target: Target,
    pub(crate) key: Option<Key>,
    pub(crate) properties: BTreeMap<String, Value>,
}
#[derive(Debug, Clone)]
pub struct Callback {
    pub(crate) engine: u64,
    pub(crate) owner: Option<InstanceId>,
    pub(crate) target: CallbackTarget,
    pub(crate) captures: BTreeMap<BindingId, Value>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CallbackTarget {
    Function(FunctionId),
    Native(ExportKey),
}
