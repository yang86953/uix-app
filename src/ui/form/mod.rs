//! 类型化表单 — Form 模型、绑定、列表与校验。
//!
//! # SMC 边界（SMC-04）
//!
//! form Module 消费 widgets（Input / Select / 日期选择等）与 view DSL，
//! 是 ui System 的叶子 Module；不依赖兄弟 Module 的私有实现。

pub mod form;
pub mod form_binding;
pub mod form_list;
pub mod form_validation;
pub mod model_form;

pub use form::*;
pub use form_binding::*;
pub use form_list::*;
pub use form_validation::*;
pub use model_form::*;
