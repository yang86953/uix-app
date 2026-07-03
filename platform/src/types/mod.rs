// ============================================================================
// platform/types/mod.rs — 平台层数据类型入口
//
// 按领域拆分到子模块，统一在此 re-export。
// lib.rs / api.rs 通过 `crate::types::*` 访问全部类型。
// ============================================================================

mod console;
mod display;
mod input;
mod key;
pub mod status;
mod system;

pub use console::*;
pub use display::*;
pub use input::*;
pub use key::*;
pub use status::*;
pub use system::*;
