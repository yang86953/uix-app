use super::asset_store::AssetStore;
use super::core::RenderTarget;
use crate::graphics::font_service::FontService;

// ════════════════════════════════════════════════════════════════════════════
// SoftwareEngine — 组合 AssetStore + RenderTarget
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
    pub(crate) pre_frame_clip: crate::base::Rect,
    /// 首帧已渲染标志（用于脏区域回退为全帧清理）
    pub(crate) rendered_first: bool,
    /// 上次渲染时的 WidgetTree 版本号，用于检测结构变更。
    pub(crate) last_tree_version: u64,
    /// 独立的字体服务，不与渲染器绑定
    pub font_service: FontService,
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
              pre_frame_clip: crate::base::Rect::new(0.0, 0.0, f32::MAX, f32::MAX),
              rendered_first: false,
              last_tree_version: 0,
              font_service: FontService::new(),
          }
    }

    /// 替换字体服务的文本后端（例如替换为 FreeType 后端）。
    /// 必须在 `initialize()` 之前调用。
    pub fn with_text_backend(
        mut self,
        backend: Box<dyn crate::graphics::text_backend::TextBackend>,
    ) -> Self {
        self.font_service = self.font_service.with_text_backend(backend);
        self
    }

    // ── 像素缓冲访问 ──

    pub fn pixel_buffer(&self) -> &[u32] {
        self.rt.pixel_buffer()
    }
    pub fn pixel_buffer_mut(&mut self) -> &mut [u32] {
        self.rt.pixel_buffer_mut()
    }
    pub fn pixel_bytes(&self) -> &[u8] {
        self.rt.pixel_bytes()
    }

    /// Set supersample level for software render target (0 = adaptive default).
    pub fn set_supersample_level(&mut self, level: u8) {
        self.rt.set_supersample_level(level);
    }


}
