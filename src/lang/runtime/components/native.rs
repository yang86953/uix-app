//! 原生属性的一次类型声明，同时提供检查签名与宿主参数解码。
use super::*;
use std::{marker::PhantomData, rc::Rc};

/// UI owner 的同步事件入口；闭包应弱引用 owner，避免界面回调保活已卸载树。
#[derive(Clone)]
pub struct EventDispatcher(Rc<dyn Fn(EventToken, Vec<Value>) -> RuntimeResult<Value>>);
impl EventDispatcher {
    pub fn new(call: impl Fn(EventToken, Vec<Value>) -> RuntimeResult<Value> + 'static) -> Self {
        Self(Rc::new(call))
    }
    pub fn dispatch(&self, token: EventToken, arguments: Vec<Value>) -> RuntimeResult<Value> {
        (self.0)(token, arguments)
    }
}

pub trait NativeProp: Sized {
    fn prop_type() -> Type;
    fn from_projected(value: &ProjectedValue, events: &EventDispatcher) -> RuntimeResult<Self>;
}
/// 只转换拥有型数据，不允许函数或 View 充当原生资源句柄。
pub trait NativeData: NativeProp {
    fn into_data(self) -> DataValue;
    fn from_data(value: &DataValue) -> RuntimeResult<Self>;
}
/// 区分未提供属性与提供零/false/空值，不把默认值哨兵混入业务类型。
#[derive(Debug, Clone, Default)]
pub struct OptionalProp<T>(pub Option<T>);
impl<T: NativeProp> NativeProp for OptionalProp<T> {
    fn prop_type() -> Type {
        T::prop_type()
    }
    fn from_projected(value: &ProjectedValue, events: &EventDispatcher) -> RuntimeResult<Self> {
        T::from_projected(value, events).map(|value| Self(Some(value)))
    }
}
fn wrong() -> RuntimeError {
    RuntimeError::new(ErrorKind::Type, "原生属性的数据类型不符")
}
macro_rules! scalar {
    ($rust:ty,$variant:ident) => {
        impl NativeData for $rust {
            fn into_data(self) -> DataValue {
                DataValue::$variant(self)
            }
            fn from_data(value: &DataValue) -> RuntimeResult<Self> {
                match value {
                    DataValue::$variant(value) => Ok(value.clone()),
                    _ => Err(wrong()),
                }
            }
        }
        impl NativeProp for $rust {
            fn prop_type() -> Type {
                Type::Data(DataType::$variant)
            }
            fn from_projected(value: &ProjectedValue, _: &EventDispatcher) -> RuntimeResult<Self> {
                match value {
                    ProjectedValue::Data(value) => Self::from_data(value),
                    _ => Err(wrong()),
                }
            }
        }
    };
}
scalar!(String, String);
scalar!(bool, Bool);
scalar!(i64, Int);
scalar!(f64, Float);
impl NativeData for () {
    fn into_data(self) -> DataValue {
        DataValue::Unit
    }
    fn from_data(value: &DataValue) -> RuntimeResult<Self> {
        if *value == DataValue::Unit {
            Ok(())
        } else {
            Err(wrong())
        }
    }
}
impl NativeProp for () {
    fn prop_type() -> Type {
        Type::Data(DataType::Unit)
    }
    fn from_projected(value: &ProjectedValue, _: &EventDispatcher) -> RuntimeResult<Self> {
        match value {
            ProjectedValue::Data(value) => Self::from_data(value),
            _ => Err(wrong()),
        }
    }
}
impl<T: NativeProp> NativeProp for Vec<T> {
    fn prop_type() -> Type {
        Type::array(T::prop_type())
    }
    fn from_projected(value: &ProjectedValue, events: &EventDispatcher) -> RuntimeResult<Self> {
        match value {
            ProjectedValue::Array(values) => values
                .iter()
                .map(|value| T::from_projected(value, events))
                .collect(),
            ProjectedValue::Data(value) => match value.as_ref() {
                DataValue::Array(values) => values
                    .iter()
                    .map(|value| {
                        T::from_projected(&ProjectedValue::Data(Arc::new(value.clone())), events)
                    })
                    .collect(),
                _ => Err(wrong()),
            },
            _ => Err(wrong()),
        }
    }
}
impl<T: NativeData> NativeData for Vec<T> {
    fn into_data(self) -> DataValue {
        DataValue::Array(self.into_iter().map(T::into_data).collect())
    }
    fn from_data(value: &DataValue) -> RuntimeResult<Self> {
        match value {
            DataValue::Array(values) => values.iter().map(T::from_data).collect(),
            _ => Err(wrong()),
        }
    }
}

pub trait CallbackArguments {
    fn types() -> Vec<Type>;
    fn values(self) -> Vec<Value>;
}
impl CallbackArguments for () {
    fn types() -> Vec<Type> {
        Vec::new()
    }
    fn values(self) -> Vec<Value> {
        Vec::new()
    }
}
macro_rules! arguments {
    ($($name:ident),+)=>{impl<$($name:NativeData),+> CallbackArguments for ($($name,)+){
        fn types()->Vec<Type>{vec![$($name::prop_type()),+]}
        #[allow(non_snake_case)]fn values(self)->Vec<Value>{let ($($name,)+)=self;vec![$(Value::data($name.into_data())),+]}
    }}
}
arguments!(A);
arguments!(A, B);
arguments!(A, B, C);
arguments!(A, B, C, D);

/// 有类型参数与结果的当前投影回调；不会直接获取组件状态的可变引用。
pub struct NativeCallback<A, R = ()> {
    token: Option<EventToken>,
    events: Option<EventDispatcher>,
    marker: PhantomData<fn(A) -> R>,
}
impl<A, R> Clone for NativeCallback<A, R> {
    fn clone(&self) -> Self {
        Self {
            token: self.token,
            events: self.events.clone(),
            marker: PhantomData,
        }
    }
}
impl<A, R> Default for NativeCallback<A, R> {
    fn default() -> Self {
        Self {
            token: None,
            events: None,
            marker: PhantomData,
        }
    }
}
impl<A: CallbackArguments, R: NativeData> NativeCallback<A, R> {
    pub fn is_bound(&self) -> bool {
        self.token.is_some()
    }
    pub fn call(&self, args: A) -> RuntimeResult<R> {
        let token = self
            .token
            .ok_or_else(|| RuntimeError::new(ErrorKind::UnknownEvent, "可选回调未提供"))?;
        let value = self
            .events
            .as_ref()
            .ok_or_else(wrong)?
            .dispatch(token, args.values())?;
        R::from_data(value.as_data()?)
    }
}
impl<A: CallbackArguments> NativeCallback<A, ()> {
    /// 未提供的可选通知不执行；真实事件错误通过返回值及 owner 错误状态交付。
    pub fn emit(&self, args: A) -> RuntimeResult<()> {
        if self.is_bound() {
            self.call(args)
        } else {
            Ok(())
        }
    }
}
impl<A: CallbackArguments, R: NativeData> NativeProp for NativeCallback<A, R> {
    fn prop_type() -> Type {
        let parameters = A::types();
        Type::Function(FunctionSignature {
            minimum_arguments: parameters.len(),
            parameters,
            returns: Box::new(R::prop_type()),
            effect: Effect::Command,
        })
    }
    fn from_projected(value: &ProjectedValue, events: &EventDispatcher) -> RuntimeResult<Self> {
        match value {
            ProjectedValue::Event(token) => Ok(Self {
                token: Some(*token),
                events: Some(events.clone()),
                marker: PhantomData,
            }),
            _ => Err(wrong()),
        }
    }
}

/// 具名 View 属性，真实原生构造器通过 UiBuildContext 在当前上下文中展开。
#[derive(Clone, Default)]
pub struct Children(pub Vec<NativeNode>);
impl NativeProp for Children {
    fn prop_type() -> Type {
        Type::View
    }
    fn from_projected(value: &ProjectedValue, _: &EventDispatcher) -> RuntimeResult<Self> {
        let mut nodes = Vec::new();
        let mut pending = vec![value];
        while let Some(value) = pending.pop() {
            match value {
                ProjectedValue::View(children) => nodes.extend(children.iter().cloned()),
                ProjectedValue::Array(values) => pending.extend(values.iter().rev()),
                _ => return Err(wrong()),
            }
        }
        Ok(Self(nodes))
    }
}
pub trait NativeProps: Sized + 'static {
    fn signature() -> ComponentSignature;
    fn read(
        properties: &BTreeMap<String, ProjectedValue>,
        events: &EventDispatcher,
    ) -> RuntimeResult<Self>;
}

/// 从一次 Rust 属性声明生成接口与解码；支持明确的 UIX 名称和有类型默认值。
/// 纯 .uix 组件不使用此宏，也不需要逐组件登记。
#[macro_export]
macro_rules! native_props {
    ($visibility:vis struct $name:ident { $($field:ident : $ty:ty $(=> $external:literal)? $(= $default:expr)?),* $(,)? })=>{
        $visibility struct $name {$(pub $field:$ty),*}
        impl $crate::lang::runtime::components::NativeProps for $name {
            fn signature()->$crate::lang::runtime::components::ComponentSignature {
                #[allow(unused_mut)] let mut required=::std::collections::BTreeSet::new();
                $( $crate::native_props!(@required required,$crate::native_props!(@name $field $(,$external)?) $(,$default)?); )*
                $crate::lang::runtime::components::ComponentSignature { parameters:vec![$(($crate::native_props!(@name $field $(,$external)?).into(),<$ty as $crate::lang::runtime::components::NativeProp>::prop_type())),*],required }
            }
            fn read(properties:&::std::collections::BTreeMap<String,$crate::lang::runtime::components::ProjectedValue>,events:&$crate::lang::runtime::components::EventDispatcher)->$crate::lang::runtime::RuntimeResult<Self>{
                const NAMES:&[&str]=&[$($crate::native_props!(@name $field $(,$external)?)),*];
                if let Some(name)=properties.keys().find(|name| !NAMES.contains(&name.as_str())) {
                    return Err($crate::lang::runtime::RuntimeError::new($crate::lang::runtime::ErrorKind::Argument,format!("未知原生属性 {name}")));
                }
                Ok(Self{$($field:match properties.get($crate::native_props!(@name $field $(,$external)?)){
                    Some(value)=><$ty as $crate::lang::runtime::components::NativeProp>::from_projected(value,events)?,
                    None=>$crate::native_props!(@missing $field $(,$default)?),
                }),*})
            }
        }
    };
    (@name $field:ident,$external:literal)=>{$external};
    (@name $field:ident)=>{stringify!($field)};
    (@required $required:ident,$name:expr,$default:expr)=>{};
    (@required $required:ident,$name:expr)=>{$required.insert($name.into());};
    (@missing $field:ident,$default:expr)=>{$default};
    (@missing $field:ident)=>{return Err($crate::lang::runtime::RuntimeError::new($crate::lang::runtime::ErrorKind::Argument,concat!("缺少原生属性 ",stringify!($field))))};
}
