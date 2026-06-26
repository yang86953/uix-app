//! 图形引擎编排——生命周期 + 帧控制 + Canvas 入口。

use uix_core::Rect;
use uix_diag::Error;

use super::{Canvas2D, Canvas3D, UpdateStrategy};
use crate::engine::RenderOutcome;
use crate::ImageHandle;

/// 图形引擎 trait。
///
/// 组合 RenderingBackend + Canvas2D + Canvas3D，
/// 只负责生命周期、帧控制、提供绘制能力入口。
///
/// 字体/文字/图片资源管理归 UI 层，引擎只接收已光栅化的像素数据。
pub trait GraphicsEngine: 'static {
    /// 初始化引擎。
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error>;

    /// 释放引擎资源。
    fn shutdown(&mut self);

    /// 尺寸变更。
    fn resize(&mut self, width: i32, height: i32);

    /// 开始帧。`strategy` 决定清除策略。
    /// 返回 `Idle` 可跳过本帧 rendering 以省电。
    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome;

    /// 结束帧。
    /// 自动检测本帧是否有绘制调用，无调用返回 `Idle`。
    fn end_frame(&mut self) -> RenderOutcome;

    /// 获取 2D 绘制上下文。
    fn canvas_2d(&mut self) -> &mut dyn Canvas2D;

    /// 获取 3D 绘制上下文。
    ///
    /// SoftwareEngine 返回 NotSupported 错误。
    fn canvas_3d(&mut self) -> &mut dyn Canvas3D;

    // ── 离屏缓冲管理（可选，默认空操作） ──

    /// 创建离屏渲染表面，返回句柄。
    /// SoftwareEngine 正经实现，GpuEngine 返回 None。
    fn create_offscreen(&mut self, width: i32, height: i32) -> Option<ImageHandle> {
        let _ = (width, height);
        None
    }

    /// 销毁离屏渲染表面。
    fn destroy_offscreen(&mut self, handle: ImageHandle) {
        let _ = handle;
    }

    /// 获取离屏表面的 Canvas2D 引用（用于渲染到离屏）。
    fn offscreen_canvas(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> {
        let _ = handle;
        None
    }

    /// 将离屏表面 blit 到主表面。
    fn blit_offscreen(&mut self, handle: &ImageHandle, dst_rect: Rect) {
        let _ = (handle, dst_rect);
    }

    // ── 可选诊断 ──

    /// 当前内存使用量（字节）。默认返回 0。
    fn memory_usage(&self) -> usize {
        0
    }

    /// 打印内存诊断信息到日志。默认空实现。
    fn diagnose_memory(&self) {}
}
