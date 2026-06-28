// ============================================================================
// platform/presenter.rs — 像素呈现器
//
// 包含 IPresenter（CPU 像素呈现）、IGraphicsContext（GPU 图形上下文）
// 和 NullPresenter（空操作实现）。
// ============================================================================

use crate::error::Error;

// ════════════════════════════════════════════════════════════════════════════
// IPresenter — 像素呈现（每个窗口独立）
// ════════════════════════════════════════════════════════════════════════════

pub trait IPresenter {
    /// 将 ARGB 像素缓冲区呈现到窗口。
    /// `dirty_rect` 为局部更新区域 `(x, y, w, h)`，`None` 表示全帧。
    fn present(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        dirty_rect: Option<(i32, i32, i32, i32)>,
    ) -> Result<(), Error>;
    /// 窗口尺寸变化时重建中间资源
    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error>;
}

// ════════════════════════════════════════════════════════════════════════════
// IGraphicsContext — GPU 图形上下文（平台层管理 GL 上下文生命周期）
//
// GpuEngine 通过此 trait 获取 GL 上下文，直接使用 glow 进行 GPU 渲染。
// platform 层只负责上下文生命周期（创建/激活/交换/销毁），
// graphics 层负责所有着色器、纹理、绘制逻辑。
// ════════════════════════════════════════════════════════════════════════════

pub trait IGraphicsContext {
    /// 初始化 GL 上下文，绑定到指定原生窗口。
    /// `native_window` 是平台原生窗口句柄（如 Wayland wl_surface*、Windows HWND）。
    fn initialize(
        &mut self,
        native_window: *mut std::ffi::c_void,
        width: i32,
        height: i32,
    ) -> Result<(), Error>;

    /// 调整为新的帧缓冲尺寸（窗口 resize 后调用）。
    fn resize(&mut self, width: i32, height: i32);

    /// 使此上下文成为当前线程的活跃 GL 上下文。
    /// GpuEngine 在每帧开始前必须调用此方法。
    fn make_current(&mut self);

    /// 交换前后缓冲区，将渲染结果呈现到窗口。
    fn swap_buffers(&mut self);

    /// 销毁 GL 上下文，释放所有 GPU 资源。
    fn shutdown(&mut self);

    /// 从默认帧缓冲读回像素（用于 CPU fallback 或诊断）。
    /// 返回 BGRA 格式的 u32 像素数组。
    fn read_pixels(&mut self, x: i32, y: i32, width: i32, height: i32) -> Vec<u32>;

    /// 当前帧缓冲宽度。
    fn width(&self) -> i32;

    /// 当前帧缓冲高度。
    fn height(&self) -> i32;

    /// 获取 GL 函数指针（用于 glow 等 GL 绑定库加载函数）。
    /// 返回 `None` 表示该函数不可用。
    fn get_proc_address(&self, name: &str) -> Option<*const std::ffi::c_void> {
        let _ = name;
        None
    }
}

// ════════════════════════════════════════════════════════════════════════════
// NullPresenter — 空操作实现
// ════════════════════════════════════════════════════════════════════════════

/// 空操作呈现器（丢弃所有像素）。
///
/// 用于初始化阶段（窗口尚未创建）或测试场景。
#[derive(Default)]
pub struct NullPresenter;

impl NullPresenter {
    pub fn new() -> Self {
        Self
    }
}

impl IPresenter for NullPresenter {
    fn present(
        &mut self,
        _pixels: &[u32],
        _width: i32,
        _height: i32,
        _dirty_rect: Option<(i32, i32, i32, i32)>,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn resize(&mut self, _width: i32, _height: i32) -> Result<(), Error> {
        Ok(())
    }
}
