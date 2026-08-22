//! 显式 feature 驱动的 Linux Wayland/EGL/OpenGL ES 真实窗口呈现测试。

#![cfg(target_os = "linux")]

// 从真实 UI WidgetRender 入口闭合共享 FramePlan 到 EGL window surface。
#[test]
fn ui_drawing_frame_plan_presents_through_real_opengl_es_wsi() {
    uix::__run_opengl_wsi_production_chain_test();
}
