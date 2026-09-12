//! 可移植 UIX 应用模块的值语义与受控执行组件。
//!
//! 不包含源码解析、Rust 生成器、文件发现或窗口；实例由应用显式创建和关闭。

#[cfg(any(feature = "lang-build", feature = "uix-modules"))]
mod async_port;
#[cfg(any(feature = "lang-build", feature = "uix-components"))]
pub mod components;
mod control;
mod data_contract;
mod data_methods;
#[cfg(any(feature = "lang-build", feature = "uix-modules"))]
mod execute;
#[cfg(any(feature = "lang-build", feature = "uix-modules"))]
mod methods;
#[cfg(any(feature = "lang-build", feature = "uix-modules"))]
mod model;
mod value;
#[cfg(any(feature = "lang-build", feature = "uix-modules"))]
mod worker;
#[cfg(any(feature = "lang-build", feature = "uix-modules"))]
pub use async_port::{AsyncCall, AsyncCompletion, AsyncHostPort, AsyncHostPorts, OperationId};
pub use control::{Cancellation, Limits};
pub use data_contract::{binary_signature, method_names, method_signature};
#[cfg(any(feature = "lang-build", feature = "uix-modules"))]
pub use execute::*;
#[cfg(any(feature = "lang-build", feature = "uix-modules"))]
pub use model::*;
pub use value::*;
#[cfg(any(feature = "lang-build", feature = "uix-modules"))]
pub use worker::*;
