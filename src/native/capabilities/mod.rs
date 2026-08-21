//! platform `capabilities` Module：系统、硬件与轻量系统服务。
//!
//! SMC 边界：本 Module 是 platform System 的私有实现，由 System 私有边界
//! （`crate::native` 根）拥有。职责：向公开面 `src/platform` 提供 owned
//! 描述值与 typed Result；不拥有窗口事件循环、surface、Renderer 或
//! device-lost 恢复。
//!
//! 契约与实现：`system` 是本 Module 的窄能力契约（OS 提供者在 `backends`
//! 中实现），`services` 提供通知与文件服务。本 Module 不依赖兄弟 Module
//! （windowing / presentation / agent-transport）。显示合同由 `platform`
//! 中立叶唯一持有。

pub(crate) mod services;
pub(crate) mod system;

pub(crate) use system::*;
