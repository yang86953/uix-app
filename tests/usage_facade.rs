//! F 批次「使用落地」公开 API 契约。
//!
//! 覆盖：`Transition` + `View::enter/leave`、`Header::title` /
//! `Content::child` / `Grid::children`、`Platform` 便捷查询公开面、
//! `App::diagnostics_runtime`。

use uix::diagnostics::{Diagnostics, DiagnosticsConfig};
use uix::prelude::*;

#[test]
fn transition_compact_constructors_map_to_animation_config() {
    let fade: AnimationConfig = Transition::fade_in(0.3).into();
    assert!(fade.is_enter() && fade.duration() == 0.3);
    let slide: AnimationConfig = Transition::slide_up(0.25).into();
    assert!(slide.is_enter());
    let staggered: AnimationConfig = Transition::stagger(0.2, Transition::slide_up(0.1)).into();
    assert_eq!(staggered.delay(), 0.2, "stagger 延迟进入 AnimationConfig");
    let exit: AnimationConfig = Transition::fade_out(0.2).into();
    assert!(exit.is_exit());
}

#[test]
fn view_enter_and_leave_accept_transition() {
    let view = label("hello").enter(Transition::fade_in(0.3)).leave(Transition::fade_out(0.2));
    let _node = view.build();
    // 编译期契约：enter/leave 链式可用；播放逻辑由既有 enter_animation 管线覆盖。
}

#[test]
fn layout_facade_builders_produce_nodes() {
    let header = Header::new(48.0).title("工作台");
    let header_node = header.into_node();
    assert_eq!(header_node.children.len(), 0, "Header 为叶子容器");

    let content = Content::new().child(label("body"));
    let content_node = content.into_node();
    assert_eq!(content_node.children.len(), 1, "Content::child 包裹单个子节点");

    let grid = Grid::responsive()
        .cols(vec![Col::new().span(12), Col::new().span(12)])
        .children(vec![label("a"), label("b")]);
    assert_eq!(grid.children.len(), 2, "Grid::children 包裹多个子节点");
}

#[test]
fn platform_convenience_queries_are_public_surface() {
    // 公开面契约：Platform::new 与三个便捷查询均可直接调用（函数指针签名）。
    let _: fn() -> uix::core::Result<uix::platform::Platform> = uix::platform::Platform::new;
    let _: fn(&uix::platform::Platform) -> uix::core::Result<uix::platform::hardware::OsInfo> =
        uix::platform::Platform::os_info;
    let _: fn(&uix::platform::Platform) -> uix::core::Result<uix::platform::hardware::CpuInfo> =
        uix::platform::Platform::cpu_info;
    let _: fn(&uix::platform::Platform) -> uix::core::Result<uix::platform::hardware::MemoryInfo> =
        uix::platform::Platform::memory_info;
    // 文档示例访问器契约：OsInfo::name / CpuInfo::architecture / logical_cores。
    let _: fn(&uix::platform::hardware::OsInfo) -> &str = uix::platform::hardware::OsInfo::name;
    let _: fn(&uix::platform::hardware::CpuInfo) -> &str =
        uix::platform::hardware::CpuInfo::architecture;
    let _: fn(&uix::platform::hardware::CpuInfo) -> std::num::NonZeroUsize =
        uix::platform::hardware::CpuInfo::logical_cores;
}

#[test]
fn diagnostics_runtime_injection_accepts_shared_handle() {
    // 编译期契约：App::diagnostics_runtime 接受已构建 Diagnostics 实例。
    let diagnostics = Diagnostics::new(DiagnosticsConfig::default());
    let app = App::new().diagnostics_runtime(diagnostics.clone());
    let _ = app;
}
