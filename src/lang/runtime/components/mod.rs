//! 组件实例的共享运行合同。AOT 使用原生函数；动态解释器只随 uix-dynamic 编入。
//! 不拥有窗口或后台线程；UI owner 显式推进输入、事件和关闭。

mod engine;
#[cfg(any(feature = "lang-build", feature = "uix-dynamic"))]
pub mod ir;
mod model;
mod native;
mod types;
mod value;

pub use engine::*;
pub use native::*;
#[cfg(feature = "ui")]
pub mod ui;
pub use model::*;
pub use types::*;
pub use value::*;

use crate::lang::runtime::{
    Cancellation, Effect, ErrorKind, Limits, Location, RuntimeError, RuntimeResult,
    Type as DataType, Value as DataValue,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::new(ErrorKind::InvalidComponent, message)
}
