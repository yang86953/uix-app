//! 主循环调度队列 — app System 私有边界（SMC-06）。
//!
//! event-loop 与 window 两个 Module 共用的调度基础设施：
//!
//! | 模块 | 职责 |
//! |------|------|
//! | [`clock`] | 应用时钟抽象（wall / monotonic 注入点） |
//! | [`active_work_registry`] | 活跃工作注册表（窗口驱动循环的挂起工作追踪） |
//! | [`app_timer`] | 定时器队列（`TimerHandle` 的宿主） |
//! | [`main_thread_queue`] | 主线程任务队列 |
//!
//! 按 SMC「跨 Module 契约归 System 私有边界」收口：window 的窗口驱动循环与
//! event-loop 的主循环都消费本边界，二者之间不再互相引用（兄弟 Module 零依赖）。
//! 本边界只依赖 `crate::core` / `crate::draw` / `crate::ui` 的域外类型，不依赖
//! app 内任何 Module。

pub(crate) mod active_work_registry;
pub(crate) mod app_timer;
pub(crate) mod clock;
pub(crate) mod main_thread_queue;
