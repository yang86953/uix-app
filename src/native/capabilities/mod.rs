//! platform `capabilities` Module：系统、硬件、显示器查询与轻量系统服务。
//!
//! SMC 边界：本 Module 是 platform System 的私有实现，由 System 私有边界
//! （`crate::native` 根）拥有。职责：向公开面 `src/platform` 提供 owned
//! 描述值与 typed Result；不拥有窗口事件循环、surface、Renderer 或
//! device-lost 恢复。
//!
//! 契约与实现：`system` / `display` 是本 Module 的窄能力契约（OS 提供者
//! 在 `backends` 中实现），`services` 提供通知与文件服务。本 Module 不
//! 依赖兄弟 Module（windowing / presentation / agent-transport）。

pub mod display;
pub mod services;
pub mod system;

pub use display::*;
pub use system::*;
