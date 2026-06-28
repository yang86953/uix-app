//! # uix-platform 稳定公开 API
//!
//! 本模块定义 platform 层的公开契约：
//!
//! - [`traits`] — 行为接口（trait），各内部模块实现
//! - [`types`] — 数据契约（值类型），跨层共享
//!
//! 外部使用者应优先通过本模块访问 platform 层能力。
//! 内部代码重构时，只要保持此模块中的签名不变，外部代码就不受影响。

pub mod traits;
pub mod types;

// ── 统一 re-export ─────────────────────────────────────────────
pub use traits::*;
pub use types::*;
