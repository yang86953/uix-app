// ============================================================================
// platform/types/mod.rs — 平台层数据类型入口
//
// 按领域拆分到子模块，统一在此 re-export。
// lib.rs / api.rs 通过 `crate::types::*` 访问全部类型。
// ============================================================================

mod key;
mod input;
mod console;
mod display;
mod system;
pub mod status;

pub use key::*;
pub use input::*;
pub use console::*;
pub use display::*;
pub use system::*;
pub use status::*;
