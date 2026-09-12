//! OpenGL ES 的 retained thin RHI owner 与 surface bridge。

// 引入 glow 上下文扩展方法，供 surface readback 与资源释放使用。
use glow::HasContext as _;

// 引入统一错误和结果类型。
use crate::core::{Errc, Error, Result};

// 引入当前原生 OpenGL runtime owner。
use super::NativeOpenGlRuntime;

// 引入薄 RHI 资源设备。
use rhi_device::OpenGlRhiDevice;

// 描述当前 OpenGL framebuffer 与物理 drawable 范围。
#[derive(Clone, Copy)]
pub(crate) struct TargetState {
    // None 表示原生 swapchain framebuffer。
    pub(crate) framebuffer: Option<glow::Framebuffer>,
    // 保存物理 drawable 宽度。
    pub(crate) drawable_width: i32,
    // 保存物理 drawable 高度。
    pub(crate) drawable_height: i32,
}

// 提供 swapchain target 的规范构造。
impl TargetState {
    // 从逻辑与物理尺寸建立默认 framebuffer 状态。
    pub(crate) fn swapchain(
        _logical_width: i32,
        _logical_height: i32,
        drawable_width: i32,
        drawable_height: i32,
    ) -> Self {
        // 保存经过下限约束的物理 drawable 范围。
        Self {
            // 默认 framebuffer 由 None 表示。
            framebuffer: None,
            // drawable 宽度至少保留一个像素。
            drawable_width: drawable_width.max(1),
            // drawable 高度至少保留一个像素。
            drawable_height: drawable_height.max(1),
        }
    }
}

/// 当前原生 OpenGL context 的 retained RHI owner。
pub(crate) struct OpenGlRasterPipeline {
    // 保存 owner-thread OpenGL runtime。
    runtime: NativeOpenGlRuntime,
    // 保存默认 swapchain target 状态。
    swapchain: TargetState,
    // 保存当前 pass 完成后应恢复的 target 状态。
    current: TargetState,
    // 防止重复释放原生资源。
    released: bool,
    // 保存通用 FramePlan 使用的 OpenGL ES RHI 资源和 pass 状态。
    rhi: OpenGlRhiDevice,
}

// Drop 不执行可能越过 native context 生命周期的 OpenGL 命令。
impl Drop for OpenGlRasterPipeline {
    // 原生 context 必须在 owner-thread shutdown 中显式调用 release。
    fn drop(&mut self) {}
}

// 把 glow 字符串错误转换为项目 typed error。
fn gl_error(operation: &str, error: String) -> Error {
    // 保留操作名与驱动返回细节。
    Error::new(
        Errc::PlatformError,
        format!("OpenGL {operation} failed: {error}"),
    )
}

// 保存 retained RHI owner 的构造与 surface 元数据更新。
mod pipeline;
// 保存默认 framebuffer 恢复、readback 与检查式释放。
mod pipeline2;

// 保持固定 GLSL ABI 与资源设备实现处于同一 native adapter 边界。
#[path = "rhi_shaders.rs"]
mod rhi_shaders;
// 将 RHI 资源表和 pass 生命周期拆出，避免主 raster 文件继续膨胀。
#[path = "rhi_device.rs"]
mod rhi_device;
// 将 OpenGlRasterPipeline 的 RHI bridge 暴露给 WGL/EGL context。
#[path = "rhi.rs"]
mod rhi;

// 真实 EGL/GLES 一致性测试仍复用私有 RHI Device，不建立第二套绘制实现。
#[cfg(all(target_os = "linux", uix_gpu_parity_opengl))]
pub(crate) fn run_gpu_parity_test() {
    rhi_device::run_gpu_parity_test();
}
