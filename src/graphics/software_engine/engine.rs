use super::asset_store::AssetStore;
use super::core::RenderTarget;
use super::fontdue_backend::FontdueBackend;
use crate::graphics::text_backend::TextBackend;
use crate::graphics::FontHandle;

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
    pub(crate) text_backend: Box<dyn TextBackend>,
    pub(crate) active_target: ActiveTarget,
    pub(crate) main_width: i32,
    pub(crate) main_height: i32,
    pub(crate) saved_pixels: Vec<u32>,
    pub(crate) pre_frame_clip: crate::base::Rect,
    /// LayerTree — RepaintBoundary 追踪（跳过干净子树）
    pub(crate) layer_tree: crate::graphics::layer::LayerTree,
    /// LayerTree 是否需要重建（widget 树结构变化后设为 true）
    pub(crate) layer_tree_stale: bool,
    /// 首帧已渲染标志（用于脏区域回退为全帧清理）
    pub(crate) rendered_first: bool,
    /// Cached last loaded font handle for the `load_font` GraphicsEngine API.
    pub(crate) loaded_font_handle: FontHandle,
    /// 首选字体族名称（用户可通过 API 配置，默认 "sans-serif" 走系统默认）。
    pub(crate) primary_family: String,
    /// 用户是否已主动设置过字体族（优先于系统默认）。
    pub(crate) user_family_set: bool,
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
            text_backend: Box::new(FontdueBackend::new()),
            active_target: ActiveTarget::Main,
            main_width: 0,
            main_height: 0,
            saved_pixels: Vec::new(),
            pre_frame_clip: crate::base::Rect::new(0.0, 0.0, f32::MAX, f32::MAX),
            layer_tree: crate::graphics::layer::LayerTree::new(),
            layer_tree_stale: true,
            rendered_first: false,
            loaded_font_handle: FontHandle::new(0),
            primary_family: "sans-serif".into(),
            user_family_set: false,
        }
    }

    /// 替换文本后端（例如替换为 FreeType 后端）。
    /// 必须在 `initialize()` 之前调用。
    pub fn with_text_backend(mut self, backend: Box<dyn TextBackend>) -> Self {
        self.text_backend = backend;
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
