use super::asset_store::AssetStore;
use super::core::RenderTarget;
use crate::engine::cpu::canvas_2d::CpuCanvas2D;
use crate::engine::cpu::noop_canvas_3d::NoopCanvas3D;
use crate::engine::cpu::pixel_surface::PixelSurface;
use crate::font_service::FontService;
use crate::Color;
use uix_core::Rect;

// ════════════════════════════════════════════════════════════════════════════
// SoftwareEngine — 组合 AssetStore + RenderTarget + FrameManager
// ════════════════════════════════════════════════════════════════════════════

/// 当前活跃的渲染目标（主缓冲或离屏缓冲）。
pub(crate) enum ActiveTarget {
    Main,
    Offscreen(usize),
}

/// CPU 软件渲染引擎 —— 组合 RenderTarget、AssetStore、TextBackend。
///
/// 所有绘制最终委托给 `rt: RenderTarget`，资源管理委托给 `assets: AssetStore`，
/// 字体加载和光栅化委托给 `text_backend: Box<dyn TextBackend>`。
pub struct SoftwareEngine {
    pub(crate) rt: RenderTarget,
    pub(crate) assets: AssetStore,
    pub(crate) active_target: ActiveTarget,
    pub(crate) main_width: i32,
    pub(crate) main_height: i32,
    /// 帧开始前的基础裁剪矩形（`end_frame` 时恢复）
    pub(crate) pre_frame_clip: Rect,

    /// 独立的字体服务，不与渲染器绑定
    pub font_service: FontService,

    /// 脏区域清除时使用的背景色（默认透明黑，上层可设为主题背景色）
    pub clear_color: Color,

    /// 新 2D 绘制上下文（v2 重构）
    pub(crate) canvas_2d: CpuCanvas2D,
    /// 新 3D 绘制上下文（Noop）
    pub(crate) canvas_3d: NoopCanvas3D,
}

impl Default for SoftwareEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SoftwareEngine {
    /// 创建新的软件渲染引擎实例。
    pub fn new() -> Self {
          Self {
              rt: RenderTarget::new(),
              assets: AssetStore::new(),
              active_target: ActiveTarget::Main,
              main_width: 0,
              main_height: 0,
              pre_frame_clip: Rect::new(0.0, 0.0, f32::MAX, f32::MAX),

              font_service: FontService::new(),
              clear_color: Color::from_rgba(0, 0, 0, 0),

              canvas_2d: CpuCanvas2D::new(PixelSurface::new(1, 1)),
              canvas_3d: NoopCanvas3D,
          }
    }

    /// 替换字体服务的文本后端（例如替换为 FreeType 后端）。
    /// 必须在 `initialize()` 之前调用。
    pub fn with_text_backend(
        mut self,
        backend: Box<dyn crate::text_backend::TextBackend>,
    ) -> Self {
        self.font_service = self.font_service.with_text_backend(backend);
        self
    }
}
