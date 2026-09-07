#![deny(clippy::unwrap_used, clippy::expect_used)]
// Error 的 #[must_use] 保持默认 warn 级：不按诊断规范处置错误时提出
// 编译警告提示，不阻断构建（运行时落地检测与静态审计兜底）。

//! UIX — Rust Native Application Framework
//!
//! # 功能域
//!
//! | 模块 | 职责 |
//! |------|------|
//! | [`core`] | 基础设施 — 错误、几何 |
//! | [`bus`] | 基础设施 — 进程内同步类型化事件分发（模式外，待定级） |
//! | `native` | 平台能力 — OS 抽象（Win32 / Wayland） |
//! | [`draw`] | 绘制能力 — 2D 引擎、光栅化、字体、合成 |
//! | [`ui`] | 界面能力 — 组件、布局、主题、View DSL |
//! | [`app`] | 应用能力 — 生命周期、主循环、CLI、DI |
//! | [`data`] | 数据能力 — 配置持久化 |
//!
//! # uix-lang 入口 compile-fail 测试
//!
//! 下列契约确认公开 `uix!` 会在编译期拒绝路径、语法与映射错误，而不是把失败
//! 延迟到运行期 parser。
//!
//! ```compile_fail
//! use uix::prelude::*;
//! let _: ViewNode = uix!("tests/fixtures/uix_lang/does_not_exist.uix");
//! ```
//!
//! ```compile_fail
//! use uix::prelude::*;
//! let _: ViewNode = uix!("<Column>\n<Text>x</Column>");
//! ```
//!
//! ```compile_fail
//! use uix::prelude::*;
//! let _: ViewNode = uix!("<Mystery />");
//! ```
//!
//! ```compile_fail
//! use uix::prelude::*;
//! let _: ViewNode = uix!(r#"<Input type="search" />"#);
//! ```
//!
//! # 公开面 compile-fail 测试
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
//! use uix::native::presentation::graphics::d3d11::D3d11Context;
//! ```
//!
//! ```compile_fail
//! // 平台共享事件源实现不可达（SMC-02 后位于 windowing Module）。
//! use uix::native::windowing::shared::event_loop::OsEventSource;
//! ```
//!
//! ```compile_fail
//! // native 私有 Module 边界不可达：capabilities / windowing / presentation。
//! use uix::native::capabilities;
//! ```
//!
//! ```compile_fail
//! use uix::platform::presentation::GraphicsRecipeContext;
//! ```
//!
//! ```compile_fail
//! // platform 私有 adapter 不可达（feature 启用与否都不允许外部引用）。
//! use uix::platform::adapters::transport::AgentEndpoint;
//! ```
//!
//! ```compile_fail
//! // 测试平台聚合不可达：native 测试替身已随平台 facade 重构移除。
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
//!
//! SMC-04 后，以下 ui 旧平铺路径不得复活（12 Module 边界与 System 私有边界收口）：
//!
//! ```compile_fail
//! // ui 旧运行时平铺路径归 component Module（SMC-04）。
//! use uix::ui::core::widget::WidgetTree;
//! ```
//!
//! ```compile_fail
//! // foundation 目录已拆解：state 归 reactive、style 归 theme（SMC-04）。
//! use uix::ui::foundation::state::State;
//! ```
//!
//! ```compile_fail
//! // traits 目录已拆解：组件契约归 component（SMC-04）。
//! use uix::ui::traits::Widget;
//! ```
//!
//! ```compile_fail
//! // 表单组件归 form Module，不得经 widgets::input 路径恢复（SMC-04）。
//! use uix::ui::widgets::input::form::Form;
//! ```
//!
//! ```compile_fail
//! // view DSL 组合子归 widgets（view 不再构建具体组件，SMC-04）。
//! use uix::ui::view::combinators::button;
//! ```
//!
//! ```compile_fail
//! // 语义快照/覆盖归 accessibility Module（SMC-04）。
//! use uix::ui::semantic_snapshot::SemanticTarget;
//! ```
//!
//! ```compile_fail
//! // ViewAdapter 是 System 私有边界粘合（SMC-04）。
//! use uix::ui::adapter::ViewAdapter;
//! ```
//!
//! ```compile_fail
//! // 树-组件语义访问点为 System 私有边界（SMC-04）。
//! use uix::ui::tree_widget_hooks::modal_was_present;
//! ```
//!
//! ```compile_fail
//! // 旧 ui 根级模块路径不得复活：window_chrome 归 widgets（SMC-04）。
//! use uix::ui::window_chrome::WindowControl;
//! ```
//!
//! SMC-05 后，data 私有 Module 边界不得经公开路径访问：
//!
//! ```compile_fail
//! // data settings Module 为私有边界（SMC-05）。
//! use uix::data::settings::SettingsService;
//! ```
//!
//! SMC-06 后，app 旧平铺路径与 System 私有边界不得复活：
//!
//! ```compile_fail
//! // 旧 shell 目录已拆解为 application / event_loop / window / agent（SMC-06）。
//! use uix::app::shell::application::App;
//! ```
//!
//! ```compile_fail
//! // 组合根 session_runtime 是 System 私有边界（SMC-06）。
//! use uix::app::session_runtime::AppRuntime;
//! ```
//!
//! ```compile_fail
//! // 主循环调度队列是 System 私有边界（SMC-06）。
//! use uix::app::queues::app_timer::TimerHandle;
//! ```
//!
//! ```compile_fail
//! // 每窗口语义状态是 System 私有边界（SMC-06）。
//! use uix::app::window_semantics::WindowSemanticState;
//! ```
//!
//! ```compile_fail
//! // agent Module 为私有边界，自动化 IPC 不经公开面暴露（SMC-06）。
//! use uix::app::agent::agent_transport::AgentTransport;
//! ```
//!
//! ```compile_fail
//! // frame_scheduler 已归 window Module，旧 event_loop 路径不得复活（SMC-06）。
//! use uix::app::event_loop::frame_scheduler::FrameScheduler;
//! ```
//!
//! ```compile_fail
//! // window 内部驱动实现不可达（SMC-06）。
//! use uix::app::window::window_driver::WindowDriver;
//! ```

// 让派生宏从当前 crate 根通过 `uix` 稳定路径回指自身。
extern crate self as uix;
// Windows TSF 的 `#[implement]` 宏展开只在 Windows 目标解析此 crate。
#[cfg(windows)]
// 宏生成代码从 crate 根查找 `windows_core`。
extern crate windows_core;

// 导出 uix-lang 编译期入口与路由 key 派生宏。
pub use uix_derive::{Display, uix, uix_app, uix_items, uix_module};
// 显式 Vulkan/OpenGL parity 的跨 System 测试组合不进入生产公开面。
#[cfg(any(
    feature = "vulkan-parity-test",
    all(target_os = "linux", feature = "opengl-parity-test")
))]
mod graphics_parity;
// 只为显式 GPU parity 测试目标转发 crate 内 Vulkan harness，不进入默认公开面。
#[cfg(feature = "vulkan-parity-test")]
#[doc(hidden)]
pub fn __run_vulkan_gpu_parity_test() {
    native::presentation::graphics::vulkan::platform::run_gpu_parity_test();
}
// 从真实 UI WidgetRender 入口验收 Drawing FramePlan 与 Vulkan Device adapter。
#[cfg(feature = "vulkan-parity-test")]
#[doc(hidden)]
pub fn __run_vulkan_ui_production_chain_test() {
    graphics_parity::run_vulkan_ui_production_chain_test();
}
// 从真实平台窗口验收 Vulkan WSI、共享 Surface 生命周期与最终 present。
#[cfg(feature = "vulkan-parity-test")]
#[doc(hidden)]
pub fn __run_vulkan_wsi_production_chain_test() {
    graphics_parity::run_vulkan_wsi_production_chain_test();
}
// 只为显式 Linux GPU parity 测试目标转发 crate 内 OpenGL ES/EGL harness。
#[cfg(all(target_os = "linux", feature = "opengl-parity-test"))]
#[doc(hidden)]
pub fn __run_opengl_gpu_parity_test() {
    native::presentation::graphics::opengl::run_gpu_parity_test();
}
// 从真实 Wayland 窗口验收 EGL Surface、共享生命周期与最终 present。
#[cfg(all(target_os = "linux", feature = "opengl-parity-test"))]
#[doc(hidden)]
pub fn __run_opengl_wsi_production_chain_test() {
    graphics_parity::run_opengl_wsi_production_chain_test();
}
// 只为真实 Wayland/EGL paced frame 测试开放分配与耗时测量边界。
#[cfg(all(target_os = "linux", feature = "opengl-parity-test"))]
#[doc(hidden)]
pub fn __run_opengl_wsi_production_chain_profile(
    mut before_profiled_frame: impl FnMut(),
    mut after_profiled_frame: impl FnMut(std::time::Duration),
) {
    graphics_parity::run_opengl_wsi_production_chain_profile(
        &mut before_profiled_frame,
        &mut after_profiled_frame,
    );
}
// 只为显式 Windows GPU parity 测试目标转发 crate 内 D3D11 生产 Adapter harness。
#[cfg(all(windows, feature = "d3d11-parity-test"))]
#[doc(hidden)]
pub fn __run_d3d11_gpu_parity_test() {
    native::presentation::graphics::d3d11::run_gpu_parity_test();
}
// 只为显式 Vulkan 验证目标转发 API 无关 Surface 生命周期契约。
#[cfg(feature = "vulkan-parity-test")]
#[doc(hidden)]
pub fn __run_surface_lifecycle_contract_test() {
    platform::presentation::rhi::run_surface_lifecycle_contract_test();
    native::presentation::graphics::vulkan::platform::run_present_completion_contract_test();
}
// 只为显式 Vulkan 验证目标转发共享 device 与逐窗口恢复合同。
#[cfg(feature = "vulkan-parity-test")]
#[doc(hidden)]
pub fn __run_vulkan_shared_device_contract_test() {
    native::presentation::graphics::vulkan::platform::run_shared_device_contract_test();
}
// capability 编译合同只在 rustdoc 收集测试时进入 crate，不污染正常使用方公开面。
#[cfg(doctest)]
// 隐藏测试载体，用户文档只保留稳定 capability 说明。
#[doc(hidden)]
// 每个 feature 的启用与关闭侧都由同一模块中的条件文档测试验证。
pub mod capability_compile_contract;

/// 按 key 取当前资源表文案（E-08）：`t!("common.save")`。
///
/// 返回 `String`（只影响显示，不改变业务值）；未命中时返回 key 原文。
/// 资源经 `uix::ui::register_translations` / `set_translations` 注册。
#[macro_export]
macro_rules! t {
    ($key:literal) => {
        $crate::ui::t_lookup($key)
    };
    // 格式化参数遵循当前 crate 的 Rust 2024 表达式语义。
    ($key:literal $(, $arg:expr)+) => {
        $crate::ui::t_lookup_fmt($key, &[$($crate::t_fmt_arg!($arg)),+])
    };
}

/// 把参数转成 `String` 供 `t!` 格式化占位（`{0}` 起始）。
#[doc(hidden)]
#[macro_export]
macro_rules! t_fmt_arg {
    // 单个格式化参数允许使用 Rust 2024 新增的表达式形式。
    ($arg:expr) => {
        ::std::string::ToString::to_string(&$arg)
    };
}

pub mod app;
pub mod bus;
pub mod core;
pub mod data;
pub mod diagnostics;
pub mod draw;
pub(crate) mod native;
pub mod platform;
pub mod prelude;
pub mod ui;
