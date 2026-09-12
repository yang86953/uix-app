//! 静态、动态和原生绑定共用的组件接口类型。

use crate::lang::runtime::{Effect, Type as DataType};
use std::collections::{BTreeMap, BTreeSet};

/// 数据图使用已有运行时类型；函数和 View 不得偷渡进数据/宿主值图。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Data(DataType),
    View,
    Function(FunctionSignature),
    Array(Box<Type>),
    Record(BTreeMap<String, Type>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSignature {
    pub minimum_arguments: usize,
    pub parameters: Vec<Type>,
    pub returns: Box<Type>,
    /// 对参数函数是允许的效果上界；具名函数和 lambda 的实际效果单独推导。
    pub effect: Effect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentSignature {
    pub parameters: Vec<(String, Type)>,
    pub required: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeExport {
    Component(ComponentSignature),
    Function(FunctionSignature),
    Type(Type),
}

/// 原生库在同一包级接口提供导出；这里不包含调用实现，不等于动态宿主已绑定。
pub type NativeLibrary = BTreeMap<String, NativeExport>;
pub type NativeLibraries = BTreeMap<String, NativeLibrary>;

impl Type {
    pub fn array(element: Self) -> Self {
        match element {
            Self::Data(ty) => Self::Data(DataType::Array(Box::new(ty))),
            ty => Self::Array(Box::new(ty)),
        }
    }
    pub fn record(fields: BTreeMap<String, Self>) -> Self {
        if fields.values().all(|ty| matches!(ty, Self::Data(_))) {
            Self::Data(DataType::Record(
                fields
                    .into_iter()
                    .map(|(name, ty)| {
                        let Self::Data(ty) = ty else { unreachable!() };
                        (name, ty)
                    })
                    .collect(),
            ))
        } else {
            Self::Record(fields)
        }
    }
    pub(crate) fn element(&self) -> Option<Self> {
        match self {
            Self::Data(DataType::Array(ty)) => Some(Self::Data(*ty.clone())),
            Self::Array(ty) => Some(*ty.clone()),
            _ => None,
        }
    }
    pub(crate) fn fields(&self) -> Option<BTreeMap<String, Self>> {
        match self {
            Self::Data(DataType::Record(fields)) => Some(
                fields
                    .iter()
                    .map(|(name, ty)| (name.clone(), Self::Data(ty.clone())))
                    .collect(),
            ),
            Self::Record(fields) => Some(fields.clone()),
            _ => None,
        }
    }
    pub(crate) fn is_view(&self) -> bool {
        *self == Self::View || self.element().is_some_and(|ty| ty.is_view())
    }
}
