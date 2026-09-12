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
//! State (`reactive`), layout (`layout`), drawing (`graphics`), native adapters
//! (`platform`) and window applications (`application`) are separate capabilities.
//! UIX build tools run on the host; `uix-modules` and `extensions` are optional
//! execution engines. See `docs/架构/框架能力拆分.md` for the dependency graph.

// 让派生宏从当前 crate 根通过 `uix_app` 稳定路径回指自身。
extern crate self as uix_app;
mod build_policy;
// Windows TSF 的 `#[implement]` 宏展开只在 Windows 目标解析此 crate。
#[cfg(all(windows, feature = "platform"))]
// 宏生成代码从 crate 根查找 `windows_core`。
extern crate windows_core;

// 导出 uix-lang 编译期入口与路由 key 派生宏。
pub mod lang;
// 显式 Vulkan/OpenGL parity 的跨 System 测试组合不进入生产公开面。
#[cfg(any(
    uix_gpu_parity_vulkan,
    all(target_os = "linux", uix_gpu_parity_opengl)
))]
#[path = "../tests-src/graphics_parity.rs"]
mod graphics_parity;

// GPU parity 的 lib 内可执行用例（UI 生产链/WSI/生命周期合同），仅测试构建编译。
#[cfg(all(test, any(uix_gpu_parity_vulkan, all(target_os = "linux", uix_gpu_parity_opengl))))]
#[path = "../tests-src/gpu_parity_entry_tests.rs"]
mod gpu_parity_entry_tests;
// 只为显式 GPU parity 测试目标转发 crate 内 Vulkan harness，不进入默认公开面。
#[cfg(uix_gpu_parity_vulkan)]
#[doc(hidden)]
pub fn __run_vulkan_gpu_parity_test() {
    native::presentation::graphics::vulkan::platform::run_gpu_parity_test();
}
// 从真实 UI WidgetRender 入口验收 Drawing FramePlan 与 Vulkan Device adapter。
#[cfg(uix_gpu_parity_vulkan)]
#[doc(hidden)]
pub fn __run_vulkan_ui_production_chain_test() {
    graphics_parity::run_vulkan_ui_production_chain_test();
}
// 从真实平台窗口验收 Vulkan WSI、共享 Surface 生命周期与最终 present。
#[cfg(uix_gpu_parity_vulkan)]
#[doc(hidden)]
pub fn __run_vulkan_wsi_production_chain_test() {
    graphics_parity::run_vulkan_wsi_production_chain_test();
}
// 只为显式 Linux GPU parity 测试目标转发 crate 内 OpenGL ES/EGL harness。
#[cfg(all(target_os = "linux", uix_gpu_parity_opengl))]
#[doc(hidden)]
pub fn __run_opengl_gpu_parity_test() {
    native::presentation::graphics::opengl::run_gpu_parity_test();
}
// 从真实 Wayland 窗口验收 EGL Surface、共享生命周期与最终 present。
#[cfg(all(target_os = "linux", uix_gpu_parity_opengl))]
#[doc(hidden)]
pub fn __run_opengl_wsi_production_chain_test() {
    graphics_parity::run_opengl_wsi_production_chain_test();
}
// 只为真实 Wayland/EGL paced frame 测试开放分配与耗时测量边界。
#[cfg(all(target_os = "linux", uix_gpu_parity_opengl))]
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
#[cfg(all(windows, uix_gpu_parity_d3d11))]
#[doc(hidden)]
pub fn __run_d3d11_gpu_parity_test() {
    native::presentation::graphics::d3d11::run_gpu_parity_test();
}
// 只为显式 Vulkan 验证目标转发 API 无关 Surface 生命周期契约。
#[cfg(uix_gpu_parity_vulkan)]
#[doc(hidden)]
pub fn __run_surface_lifecycle_contract_test() {
    platform::presentation::rhi::run_surface_lifecycle_contract_test();
    native::presentation::graphics::vulkan::platform::run_present_completion_contract_test();
}
// 只为显式 Vulkan 验证目标转发共享 device 与逐窗口恢复合同。
#[cfg(uix_gpu_parity_vulkan)]
#[doc(hidden)]
pub fn __run_vulkan_shared_device_contract_test() {
    native::presentation::graphics::vulkan::platform::run_shared_device_contract_test();
}
// capability 编译合同只在 rustdoc 收集测试时进入 crate，不污染正常使用方公开面；
// 载体文件在发布白名单外，仓库内以 RUSTDOCFLAGS='--cfg uix_repo_doc_contract'
// 运行 cargo test --doc 收集，发布包内该门控恒为关闭、无悬空模块。
#[cfg(all(doctest, uix_repo_doc_contract))]
// 隐藏测试载体，用户文档只保留稳定 capability 说明。
#[doc(hidden)]
// 每个 feature 的启用与关闭侧都由同一模块中的条件文档测试验证。
#[path = "../tests-src/capability_compile_contract.rs"]
pub mod capability_compile_contract;

// 文档测试样例载体 core_error_types（自源码 /// 迁出，仅源仓 doctest 收集）。
#[cfg(all(doctest, uix_repo_doc_contract))]
#[doc(hidden)]
#[path = "../tests-src/doc_examples/core_error_types.rs"]
mod doc_examples_core_error_types;

// 文档测试样例载体 core_mod（自源码 /// 迁出，仅源仓 doctest 收集）。
#[cfg(all(doctest, uix_repo_doc_contract))]
#[doc(hidden)]
#[path = "../tests-src/doc_examples/core_mod.rs"]
mod doc_examples_core_mod;

// 文档测试样例载体 diagnostics_mod（自源码 /// 迁出，仅源仓 doctest 收集）。
#[cfg(all(doctest, uix_repo_doc_contract))]
#[doc(hidden)]
#[path = "../tests-src/doc_examples/diagnostics_mod.rs"]
mod doc_examples_diagnostics_mod;

// 文档测试样例载体 draw_backend_contract（自源码 /// 迁出，仅源仓 doctest 收集）。
#[cfg(all(doctest, uix_repo_doc_contract))]
#[doc(hidden)]
#[path = "../tests-src/doc_examples/draw_backend_contract.rs"]
mod doc_examples_draw_backend_contract;

// 文档测试样例载体 draw_geometry_path（自源码 /// 迁出，仅源仓 doctest 收集）。
#[cfg(all(doctest, uix_repo_doc_contract))]
#[doc(hidden)]
#[path = "../tests-src/doc_examples/draw_geometry_path.rs"]
mod doc_examples_draw_geometry_path;

// 文档测试样例载体 draw_geometry_spatial_physical_box（自源码 /// 迁出，仅源仓 doctest 收集）。
#[cfg(all(doctest, uix_repo_doc_contract))]
#[doc(hidden)]
#[path = "../tests-src/doc_examples/draw_geometry_spatial_physical_box.rs"]
mod doc_examples_draw_geometry_spatial_physical_box;

// 文档测试样例载体 draw_geometry_spatial_unit（自源码 /// 迁出，仅源仓 doctest 收集）。
#[cfg(all(doctest, uix_repo_doc_contract))]
#[doc(hidden)]
#[path = "../tests-src/doc_examples/draw_geometry_spatial_unit.rs"]
mod doc_examples_draw_geometry_spatial_unit;

// 文档测试样例载体 draw_mod（自源码 /// 迁出，仅源仓 doctest 收集）。
#[cfg(all(doctest, uix_repo_doc_contract))]
#[doc(hidden)]
#[path = "../tests-src/doc_examples/draw_mod.rs"]
mod doc_examples_draw_mod;

// 文档测试样例载体 draw_painting_paint_context_mod（自源码 /// 迁出，仅源仓 doctest 收集）。
#[cfg(all(doctest, uix_repo_doc_contract))]
#[doc(hidden)]
#[path = "../tests-src/doc_examples/draw_painting_paint_context_mod.rs"]
mod doc_examples_draw_painting_paint_context_mod;

// 文档测试样例载体 lib（自源码 /// 迁出，仅源仓 doctest 收集）。
#[cfg(all(doctest, uix_repo_doc_contract))]
#[doc(hidden)]
#[path = "../tests-src/doc_examples/lib.rs"]
mod doc_examples_lib;

// 文档测试样例载体 platform_presentation_contracts_mod（自源码 /// 迁出，仅源仓 doctest 收集）。
#[cfg(all(doctest, uix_repo_doc_contract))]
#[doc(hidden)]
#[path = "../tests-src/doc_examples/platform_presentation_contracts_mod.rs"]
mod doc_examples_platform_presentation_contracts_mod;

// 文档测试样例载体 prelude（自源码 /// 迁出，仅源仓 doctest 收集）。
#[cfg(all(doctest, uix_repo_doc_contract))]
#[doc(hidden)]
#[path = "../tests-src/doc_examples/prelude.rs"]
mod doc_examples_prelude;

// 文档测试样例载体 ui_macros_helpers（自源码 /// 迁出，仅源仓 doctest 收集）。
#[cfg(all(doctest, uix_repo_doc_contract))]
#[doc(hidden)]
#[path = "../tests-src/doc_examples/ui_macros_helpers.rs"]
mod doc_examples_ui_macros_helpers;

// 文档测试样例载体 ui_macros_mod（自源码 /// 迁出，仅源仓 doctest 收集）。
#[cfg(all(doctest, uix_repo_doc_contract))]
#[doc(hidden)]
#[path = "../tests-src/doc_examples/ui_macros_mod.rs"]
mod doc_examples_ui_macros_mod;

// 文档测试样例载体 ui_theme_i18n（自源码 /// 迁出，仅源仓 doctest 收集）。
#[cfg(all(doctest, uix_repo_doc_contract))]
#[doc(hidden)]
#[path = "../tests-src/doc_examples/ui_theme_i18n.rs"]
mod doc_examples_ui_theme_i18n;

// 文档测试样例载体 ui_widget_runtime_children（自源码 /// 迁出，仅源仓 doctest 收集）。
#[cfg(all(doctest, uix_repo_doc_contract))]
#[doc(hidden)]
#[path = "../tests-src/doc_examples/ui_widget_runtime_children.rs"]
mod doc_examples_ui_widget_runtime_children;

// 文档测试样例载体 ui_widget_runtime_traits（自源码 /// 迁出，仅源仓 doctest 收集）。
#[cfg(all(doctest, uix_repo_doc_contract))]
#[doc(hidden)]
#[path = "../tests-src/doc_examples/ui_widget_runtime_traits.rs"]
mod doc_examples_ui_widget_runtime_traits;

// 文档测试样例载体 ui_widgets_combinators（自源码 /// 迁出，仅源仓 doctest 收集）。
#[cfg(all(doctest, uix_repo_doc_contract))]
#[doc(hidden)]
#[path = "../tests-src/doc_examples/ui_widgets_combinators.rs"]
mod doc_examples_ui_widgets_combinators;

// 文档测试样例载体 ui_widgets_combinators_label（自源码 /// 迁出，仅源仓 doctest 收集）。
#[cfg(all(doctest, uix_repo_doc_contract))]
#[doc(hidden)]
#[path = "../tests-src/doc_examples/ui_widgets_combinators_label.rs"]
mod doc_examples_ui_widgets_combinators_label;


/// 按 key 取当前资源表文案（E-08）：`t!("common.save")`。
///
/// 返回 `String`（只影响显示，不改变业务值）；未命中时返回 key 原文。
/// 资源经 `uix_app::ui::register_translations` / `set_translations` 注册。
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

#[cfg(feature = "application")]
pub mod app;
#[cfg(feature = "application")]
pub mod bus;
#[cfg(feature = "core")]
pub mod core;
#[cfg(feature = "data")]
pub mod data;
#[cfg(feature = "diagnostics")]
pub mod diagnostics;
#[cfg(feature = "graphics")]
pub mod draw;
#[cfg(feature = "platform")]
pub(crate) mod native;
#[cfg(feature = "platform-contracts")]
pub mod platform;
#[cfg(feature = "core")]
pub mod prelude;
#[cfg(any(feature = "reactive", feature = "layout", feature = "ui"))]
pub mod ui;

#[cfg(feature = "extensions")]
pub mod extensions;
