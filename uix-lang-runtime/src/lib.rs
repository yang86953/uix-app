//! 可移植 UIX 应用模块的值语义与受控执行组件。
//!
//! 不包含源码解析、Rust 生成器、文件发现或窗口；实例由应用显式创建和关闭。

mod value;
mod model;
mod execute;
mod methods;
mod worker;
pub use value::*;
pub use model::*;
pub use execute::*;
pub use worker::*;
