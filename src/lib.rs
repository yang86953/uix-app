#![deny(clippy::unwrap_used, clippy::expect_used)]

//! UIX — Rust Native UI Framework
//!
//! # 功能域
//!
//! | 模块 | 职责 |
//! |------|------|
//! | [`core`] | 基础设施 — 错误、几何 |
//! | [`native`] | 平台能力 — OS 抽象（Win32 / Wayland） |
//! | [`draw`] | 绘制能力 — 2D 引擎、光栅化、字体、合成 |
//! | [`ui`] | 界面能力 — 组件、布局、主题、View DSL |
//! | [`app`] | 应用能力 — 生命周期、主循环、CLI、DI |
//! | [`data`] | 数据能力 — 配置持久化 |
//!
//! # 公开面 deny 证据（compile-fail）
//!
//! 下列 compile-fail 契约锁定系统边界：`native` 根、backend SPI、raw handle、
//! context、厂商对象与旧运行保障入口对外不可达。若未来有人恢复 `pub mod native`
//! 或旧 `core::log` / `core::diagnostic` / `create_platform` 入口，对应文档测试
//! 会编译失败，阻断边界回退。
//!
//! ```compile_fail
//! // native 根不可达（SPI、实现与厂商对象一律不允许外部引用）。
//! use uix::native::factory::create_platform;
//! ```
//!
//! ```compile_fail
//! // 平台 backend SPI 不可达。
//! use uix::native::backends::windows::clipboard::WindowsClipboard;
//! ```
//!
//! ```compile_fail
//! // 图形 context / 厂商对象不可达（SMC-02 后位于 presentation Module）。
//! use uix::native::presentation::graphics::wgpu_backend::WgpuContext;
//! ```
//!
//! ```compile_fail
//! // 平台共享事件源实现不可达（SMC-02 后位于 windowing Module）。
//! use uix::native::windowing::shared::event_loop::OsEventSource;
//! ```
//!
//! ```compile_fail
//! // platform 私有 Module 边界不可达：capabilities / windowing / presentation / agent_transport。
//! use uix::native::capabilities::system::ISystemInfo;
//! ```
//!
//! ```compile_fail
//! use uix::native::windowing::window::IWindowManager;
//! ```
//!
//! ```compile_fail
//! use uix::native::presentation::graphics::IGraphicsContext;
//! ```
//!
//! ```compile_fail
//! // agent-transport Module 不可达（feature 启用与否都不允许外部引用）。
//! use uix::native::agent_transport::AgentListener;
//! ```
//!
//! ```compile_fail
//! // 测试平台聚合不可达。
//! use uix::native::test_harness::FakePlatform;
//! ```
//!
//! ```compile_fail
//! // 旧运行保障入口已被删除：`core::log` 不得复活。
//! use uix::core::log::info_fn;
//! ```
//!
//! ```compile_fail
//! // 旧运行保障入口已被删除：`core::diagnostic` 不得复活。
//! use uix::core::diagnostic::collector::Collector;
//! ```
//!
//! ```compile_fail
//! // 旧平台工厂入口不得通过 prelude 恢复。
//! use uix::prelude::create_platform;
//! ```

// `windows::core::implement` 宏展开依赖 crate 根的 `windows_core`（无平台 cfg：依赖在各目标可用）。
extern crate self as uix;
extern crate windows_core;

pub mod app;
pub mod core;
pub mod data;
pub mod diagnostics;
pub mod draw;
pub(crate) mod native;
pub mod platform;
pub mod prelude;
pub mod ui;
