//! 响应式状态 — State / Computed / Effect 与 Provider 上下文。
//!
//! # SMC 边界（SMC-04）
//!
//! reactive Module 是 ui System 的依赖基座：只依赖 `core`（值类型）与
//! `draw::renderer`（Paint 失效句柄），不依赖任何兄弟 Module。

pub mod state;
