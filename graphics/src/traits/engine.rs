//! 图形引擎编排——生命周期 + 帧控制 + Canvas 入口。

use uix_diag::Error;

use super::{Canvas2D, Canvas3D, UpdateStrategy};
use crate::engine::RenderOutcome;

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

    // ── 可选诊断 ──

    /// 当前内存使用量（字节）。默认返回 0。
    fn memory_usage(&self) -> usize {
        0
    }

    /// 打印内存诊断信息到日志。默认空实现。
    fn diagnose_memory(&self) {}
}
