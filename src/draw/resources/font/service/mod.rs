//! FontService — 独立于渲染器的字体系统。
//!
//! 职责：字体加载、元数据管理、文本布局、字形光栅化（含统一缓存）、
//!       自动多字体回退（主字体→回退链→Bitmap 位图回退）。
//! 渲染引擎只需通过 `draw_glyph_raster` 绘制像素，无需实现字体逻辑。

mod font_cache;
mod font_layout;
// 保存跨帧文本布局结果，避免页面切换时重复执行 shaping 与断行。
mod layout_cache;
// 确定性字体包的原子安装事务独立实现，保持 FontService 主文件体积边界。
mod bundle_install;
// 保存中性文本布局的两端对齐空白扩展算法。
mod text_justify;
// 帧诊断：向窗口驱动暴露文本布局调用计数器。
pub(crate) use font_layout::take_text_layout_calls;
// 保存 FontService 的 UAX #9 字体段切分与视觉 cluster 重排。
mod bidi_layout;

pub use font_cache::{FontFace, GlyphCache};

use std::sync::Arc;

use crate::core::Error;
use crate::core::{Point, Size};
use crate::draw::FontHandle;
use crate::draw::TextBackend;
use crate::draw::resources::font::text_backend::{self as tb, GlyphRaster, TextLayoutOptions};

pub(crate) use font_cache::{CachedRaster, FontSlot, GlyphCacheKey};

// 系统主字体必须至少能画出一枚基础拉丁字形，避免只完成字符映射却静默输出空栅格。
const PRIMARY_RASTER_PROBES: [char; 2] = ['A', '0'];
// CJK 回退必须真实画出完整探针集合，不能只以 cmap 映射冒充可用字体。
const CJK_RASTER_PROBES: [char; 3] = ['中', '国', '文'];

// ════════════════════════════════════════════════════════════════════════════
// FontService — 字体管理器
// ════════════════════════════════════════════════════════════════════════════

/// 字体服务——渲染器无关的字体子系统。
///
/// 管理所有已加载字体（通过 `text_backend`），提供：
/// - 字体加载/卸载（委托给后端）
/// - 字体元数据注册（family、path）
/// - 统一字形缓存
/// - 多字体回退布局（主字体 → fallback_chain → BitmapFont）
/// - 文本度量、布局、击中测试
pub struct FontService {
    pub(crate) text_backend: Box<dyn TextBackend>,
    /// 字体注册表：索引 = FontHandle.0。
    registry: Vec<FontSlot>,
    pub(crate) primary_family: String,
    pub(crate) user_family_set: bool,
    /// 主字体句柄（由 load_default_system_font 设置）。
    pub loaded_font_handle: FontHandle,
    /// 回退字体链（有序）。BitmapFont 自动作为最终回退。
    fallback_handles: Vec<FontHandle>,
    /// 统一字形缓存。
    glyph_cache: GlyphCache,
    /// 有界文本布局缓存，由字体服务统一拥有并随字体配置失效。
    layout_cache: layout_cache::TextLayoutCache,
}

impl FontService {
    /// 创建新的字体服务实例（ab_glyph 后端）。
    pub fn new() -> Self {
        Self {
            text_backend: Box::new(
                crate::draw::resources::font::text_backends::ab_glyph::AbGlyphBackend::new(),
            ),
            registry: Vec::new(),
            primary_family: "sans-serif".into(),
            user_family_set: false,
            loaded_font_handle: FontHandle::new(0),
            fallback_handles: Vec::new(),
            glyph_cache: GlyphCache::new(),
            layout_cache: layout_cache::TextLayoutCache::new(),
        }
    }

    /// 替换文本后端（例如替换为 FreeType 后端）。
    /// 必须在 `load_default_system_font` 之前调用。
    pub fn with_text_backend(mut self, backend: Box<dyn TextBackend>) -> Self {
        self.text_backend = backend;
        self
    }

    // ── 字体元数据查询 ──

    /// 返回字体族名称（若注册表中有记录）。
    pub fn font_family(&self, handle: &FontHandle) -> Option<&str> {
        let i = handle.0 as usize;
        if i < self.registry.len() && self.registry[i].handle.0 != u32::MAX {
            Some(self.registry[i].face.family.as_str())
        } else {
            None
        }
    }

    /// 按调用方声明顺序解析第一个已加载字体族。
    pub fn resolve_font_families<'a, I>(
        &self,
        // 接收中性族名视图，不依赖 UI 样式类型。
        families: I,
        // 接收全部族名不可用时的当前字体句柄。
        fallback: FontHandle,
    ) -> FontHandle
    where
        // 允许 UI 适配器直接传入零分配字符串迭代器。
        I: IntoIterator<Item = &'a str>,
    {
        // 逐项遵守声明的字体回退优先级。
        for family in families {
            // generic family 表示沿用平台当前字体，不要求注册表存在同名字体。
            if is_generic_family(family) {
                // 返回调用方进入绘制阶段时的当前系统字体。
                return fallback;
            }
            // 在仍有效的注册表槽位中执行大小写不敏感匹配。
            if let Some(slot) = self.registry.iter().find(|slot| {
                // 已卸载槽位使用哨兵句柄，不能再次被选择。
                slot.handle.0 != u32::MAX
                    // 字体族名称使用 ASCII 大小写不敏感比较。
                    && slot.face.family.eq_ignore_ascii_case(family)
            }) {
                // 返回第一个可用族对应的稳定句柄。
                return slot.handle;
            }
        }
        // 没有已加载匹配项时稳定回退当前系统字体。
        fallback
    }

    /// 返回字体文件路径（若从文件加载）。
    pub fn font_path(&self, handle: &FontHandle) -> Option<&str> {
        let i = handle.0 as usize;
        if i < self.registry.len() && self.registry[i].handle.0 != u32::MAX {
            self.registry[i].face.path.as_deref()
        } else {
            None
        }
    }

    /// 已注册的字体数量（含已卸载的无效槽位）。
    pub fn font_count(&self) -> usize {
        self.registry.len()
    }

    /// 回退链中的字体数量。
    pub fn fallback_count(&self) -> usize {
        self.fallback_handles.len()
    }

    // ── 字体加载 ──

    /// 设置首选字体族名称。
    ///
    /// 必须在调用 `load_default_system_font()` **之前**调用才能生效。
    pub fn set_font_family(&mut self, family: impl Into<String>) {
        self.primary_family = family.into();
        self.user_family_set = true;
    }

    /// 加载字体数据，返回字体句柄，同时在注册表中记录元数据。
    pub fn load_font(&mut self, data: &[u8]) -> Result<FontHandle, Error> {
        let handle = self.text_backend.load_font(data)?;
        self.register_font(handle, self.primary_family.clone(), None);
        // 注意：不更新 loaded_font_handle。loaded_font_handle 只由
        // load_default_system_font() 设置为主字体句柄。图标字体等辅助字体
        // 应通过 load_font() 加载但不应覆盖主字体句柄。
        Ok(handle)
    }

    // 加载进程期静态辅助字体，并让支持该能力的后端直接借用二进制只读段。
    pub(crate) fn load_static_font(&mut self, data: &'static [u8]) -> Result<FontHandle, Error> {
        let handle = self.text_backend.load_font_static(data)?;
        self.register_font(handle, self.primary_family.clone(), None);
        // 辅助字体与 load_font 相同，不得替换正文主字体句柄。
        Ok(handle)
    }

    /// 从文件路径加载字体，注册时会记录路径信息。
    pub fn load_font_from_path(&mut self, path: &str, _size: f32) -> Option<FontHandle> {
        let data = std::fs::read(path).ok()?;
        let handle = self.text_backend.load_font_owned(data).ok()?;
        self.register_font(
            handle,
            Self::infer_family_from_path(path),
            Some(path.to_owned()),
        );
        Some(handle)
    }

    fn register_font(&mut self, handle: FontHandle, family: String, path: Option<String>) {
        let idx = handle.0 as usize;
        while self.registry.len() <= idx {
            self.registry.push(FontSlot {
                handle: FontHandle::new(u32::MAX),
                face: FontFace {
                    family: String::new(),
                    path: None,
                },
            });
        }
        self.registry[idx] = FontSlot {
            handle,
            face: FontFace { family, path },
        };
    }

    fn install_primary_font(&mut self, handle: FontHandle, family: String, path: Option<String>) {
        self.register_font(handle, family, path);
        self.loaded_font_handle = handle;
    }

    /// 从路径字符串猜测字体族名称。
    fn infer_family_from_path(path: &str) -> String {
        let name = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        // 去掉常见的变体后缀
        let cleaned = name
            .replace("-Regular", "")
            .replace("-Bold", "")
            .replace("-Italic", "")
            .replace("-Medium", "")
            .replace("-Light", "")
            .replace("_Regular", "")
            .replace("_Bold", "");
        if cleaned.is_empty() {
            name.to_owned()
        } else {
            cleaned
        }
    }

    /// 卸载字体。
    pub fn unload_font(&mut self, handle: &FontHandle) {
        self.text_backend.unload_font(handle);
        self.glyph_cache.remove_font(handle.0);
        let idx = handle.0 as usize;
        if idx < self.registry.len() {
            // 保留槽位编号稳定性，同时清除已卸载字体的族名与路径元数据。
            self.registry[idx].handle = FontHandle::new(u32::MAX);
            // 释放反复加载字体时不再需要的族名字符串。
            self.registry[idx].face.family = String::new();
            // 释放文件字体路径字符串，避免失效槽位持续持有路径。
            self.registry[idx].face.path = None;
        }
        self.fallback_handles.retain(|h| h.0 != handle.0);
        if self.loaded_font_handle.0 == handle.0 {
            self.loaded_font_handle = FontHandle::new(u32::MAX);
        }
        self.sync_fallback_fonts();
    }

    /// 检查字体句柄是否有效。
    pub fn is_valid(&self, handle: &FontHandle) -> bool {
        self.text_backend.is_valid(handle)
    }

    /// 检查字体是否包含指定字符的字形。
    pub fn has_glyph(&self, font: &FontHandle, ch: char) -> bool {
        self.text_backend.has_glyph(font, ch)
    }

    /// 添加字体到回退链尾部。
    ///
    /// 在 `load_default_system_font` 之后调用，添加额外的回退字体。
    /// 后添加的字体优先级更低（排在链尾）。
    pub fn add_fallback(&mut self, handle: FontHandle) {
        if self.text_backend.is_valid(&handle)
            && handle.0 != self.loaded_font_handle.0
            && !self.fallback_handles.iter().any(|h| h.0 == handle.0)
        {
            self.fallback_handles.push(handle);
            self.sync_fallback_fonts();
        }
    }

    /// 显式设置完整回退链。
    pub fn set_fallback_chain(&mut self, handles: &[FontHandle]) {
        self.fallback_handles.clear();
        for &handle in handles {
            if self.text_backend.is_valid(&handle)
                && handle.0 != self.loaded_font_handle.0
                && !self.fallback_handles.iter().any(|h| h.0 == handle.0)
            {
                self.fallback_handles.push(handle);
            }
        }
        self.sync_fallback_fonts();
    }

    /// 返回当前回退链。
    pub fn fallback_chain(&self) -> &[FontHandle] {
        &self.fallback_handles
    }

    fn sync_fallback_fonts(&mut self) {
        // 回退链变化会改变同一文本的字体分段，必须先失效旧布局。
        self.layout_cache.clear();
        self.text_backend.set_fallback_fonts(&self.fallback_handles);
    }

    /// 加载系统默认字体（自动检测平台）。
    /// 第一个成功加载的字体作为主字体，其余作为回退链。
    /// 始终在注册表中记录 family 信息。
    pub fn load_default_system_font(
        &mut self,
        size: f32,
        system_info: &(impl crate::platform::services::FontSystemInfo + ?Sized),
    ) {
        if self.user_family_set {
            let family = self.primary_family.clone();
            if self.load_family_font(&family, size, system_info).is_some() {
                self.load_cjk_fallback(size, system_info);
                self.sync_fallback_fonts();
                return;
            }

            let fallback_paths = match system_info.default_font_paths() {
                Ok(paths) => paths,
                Err(error) => {
                    tracing::warn!(
                        "system default font paths unavailable: {}",
                        error.short_what()
                    );
                    Vec::new()
                }
            };
            for path in &fallback_paths {
                if let Some(handle) =
                    self.load_mapped_system_font(path, &PRIMARY_RASTER_PROBES, false, size)
                {
                    self.install_primary_font(
                        handle,
                        Self::infer_family_from_path(path),
                        Some(path.clone()),
                    );
                    tracing::info!(
                        "Configured font '{}' not found, fallback: {}",
                        self.primary_family,
                        path
                    );
                    self.load_cjk_fallback(size, system_info);
                    self.sync_fallback_fonts();
                    return;
                }
            }
        } else {
            let paths = match system_info.default_font_paths() {
                Ok(paths) => paths,
                Err(error) => {
                    tracing::warn!(
                        "system default font paths unavailable: {}",
                        error.short_what()
                    );
                    Vec::new()
                }
            };

            if !paths.is_empty() {
                let mut primary_loaded = false;

                // 启动只装主字体：其余 Latin fallback 不在首帧同步读盘。
                // CJK 由 load_cjk_fallback / probe_cjk_font_path 单独装一枚。
                for path in &paths {
                    if let Some(handle) =
                        self.load_mapped_system_font(path, &PRIMARY_RASTER_PROBES, false, size)
                    {
                        self.install_primary_font(
                            handle,
                            self.primary_family.clone(),
                            Some(path.clone()),
                        );
                        primary_loaded = true;
                        tracing::info!(
                            "Loaded system default font: {} (handle={:?})",
                            path,
                            handle
                        );
                        break;
                    }
                }

                if primary_loaded {
                    self.load_cjk_fallback(size, system_info);
                    self.sync_fallback_fonts();
                    return;
                }
            }
        }

        // 第 4 步：最后的兜底——随机扫描一个可用字体
        tracing::info!("No primary font found via platform, scanning for fallback...");
        if let Some(path) = system_info.scan_fallback_font_path() {
            if let Some(handle) =
                self.load_mapped_system_font(&path, &PRIMARY_RASTER_PROBES, false, size)
            {
                self.install_primary_font(handle, self.primary_family.clone(), Some(path.clone()));
                tracing::info!(
                    "Loaded fallback font (random scan): {} (handle={:?})",
                    path,
                    handle
                );
                self.load_cjk_fallback(size, system_info);
                self.sync_fallback_fonts();
                return;
            }
        }

        tracing::info!("No primary font found, using bitmap fallback");
    }

    /// 加载 CJK 回退字体（通过平台层探测）。
    /// 如果主字体本身已经包含中文字形则跳过，避免重复加载。
    fn load_cjk_fallback(
        &mut self,
        size: f32,
        system_info: &(impl crate::platform::services::FontSystemInfo + ?Sized),
    ) {
        // 内存剖析开关：跳过 CJK 回退字体加载，量化中文字体常驻对 working set 的贡献。
        if std::env::var_os("UIX_SKIP_CJK_FONT").is_some() {
            tracing::info!("UIX_SKIP_CJK_FONT set; skipping CJK fallback font load");
            return;
        }
        let primary_has_cjk = CJK_RASTER_PROBES.iter().all(|&ch| {
            self.text_backend.is_valid(&self.loaded_font_handle)
                && self.font_rasterizes_probe(&self.loaded_font_handle, ch, size)
        });
        if primary_has_cjk {
            tracing::info!("Primary font already supports CJK, skipping CJK fallback load",);
            return;
        }

        let cjk_paths = system_info.probe_cjk_font_paths();
        if cjk_paths.is_empty() {
            tracing::info!("No CJK fallback font found via platform");
            return;
        }

        for path in cjk_paths {
            if let Some(handle) =
                self.load_mapped_system_font(&path, &CJK_RASTER_PROBES, true, size)
            {
                self.register_font(
                    handle,
                    Self::infer_family_from_path(&path),
                    Some(path.clone()),
                );
                self.add_fallback(handle);
                tracing::info!("Loaded CJK fallback font: {}", path);
                return;
            }
            tracing::info!("CJK font '{}' failed mapping or raster probes", path);
        }
        tracing::info!("No CJK fallback font could be loaded via platform");
    }

    /// 通过平台层按字体族名称查找并加载字体。
    fn load_family_font(
        &mut self,
        family: &str,
        size: f32,
        system_info: &(impl crate::platform::services::FontSystemInfo + ?Sized),
    ) -> Option<FontHandle> {
        if let Some(p) = system_info.probe_family_font_path(family) {
            if let Some(handle) =
                self.load_mapped_system_font(&p, &PRIMARY_RASTER_PROBES, false, size)
            {
                self.install_primary_font(handle, family.to_owned(), Some(p.clone()));
                tracing::info!("Loaded family font '{}': {}", family, p);
                return Some(handle);
            }
        }
        None
    }

    /// 从文件路径内存映射加载字体（惰性分页：未触达字形不驻留 working set）。
    fn load_mapped_font(&mut self, path: impl AsRef<std::path::Path>) -> Option<FontHandle> {
        let file = std::fs::File::open(path.as_ref()).ok()?;
        // SAFETY: 映射只读；文件由本函数持有至映射建立；映射生命周期由后端槽位持有。
        let mmap = unsafe { memmap2::Mmap::map(&file).ok()? };
        let handle = self.text_backend.load_font_mapped(mmap).ok()?;
        self.register_font(handle, self.primary_family.clone(), None);
        Some(handle)
    }

    /// 仅为平台自动发现的正文候选执行真实栅格探针；显式辅助字体仍可保留符号专用覆盖。
    fn load_mapped_system_font(
        &mut self,
        path: impl AsRef<std::path::Path>,
        probes: &[char],
        require_all: bool,
        pixel_size: f32,
    ) -> Option<FontHandle> {
        let path = path.as_ref();
        let handle = self.load_mapped_font(path)?;
        let accepts = if require_all {
            probes
                .iter()
                .all(|&ch| self.font_rasterizes_probe(&handle, ch, pixel_size))
        } else {
            probes
                .iter()
                .any(|&ch| self.font_rasterizes_probe(&handle, ch, pixel_size))
        };
        if accepts {
            return Some(handle);
        }
        self.unload_font(&handle);
        tracing::warn!(
            "Rejected system font '{}' because mapped glyphs produced no visible raster",
            path.display()
        );
        None
    }

    /// 用当前文本后端完成布局与栅格化，验证指定字符确实产生可见像素或轮廓。
    fn font_rasterizes_probe(&self, font: &FontHandle, ch: char, pixel_size: f32) -> bool {
        if !self.text_backend.has_glyph(font, ch) {
            return false;
        }
        let mut text = [0_u8; 4];
        let text = ch.encode_utf8(&mut text);
        let options = TextLayoutOptions {
            max_width: f32::INFINITY,
            max_height: 0.0,
            line_height: 0.0,
            word_wrap: false,
            h_align: crate::draw::HAlign::Left,
            v_align: crate::draw::VAlign::Top,
            font_size: tb::bounded_font_size(pixel_size),
        };
        self.text_backend
            .layout_text(font, text, &options)
            .glyphs
            .iter()
            .filter(|glyph| glyph.font == *font)
            .any(|glyph| {
                let raster =
                    self.text_backend
                        .rasterize_glyph(font, glyph.glyph_id, options.font_size);
                raster.width > 0
                    && raster.height > 0
                    && (raster.coverage.iter().any(|coverage| *coverage != 0)
                        || raster
                            .outline_mesh
                            .as_ref()
                            .is_some_and(|mesh| !mesh.is_empty()))
            })
    }

    // ── 文本度量 ──

    /// 测量文本尺寸。
    ///
    /// 使用 TTF 布局（含自动回退），最终回退到 BitmapFont 度量。
    pub fn measure_text(&self, font: &FontHandle, text: &str, opts: &TextLayoutOptions) -> Size {
        if self.text_backend.is_valid(font) {
            let f = *font;
            let layout = self.layout_text_shared(&f, text, opts);
            Size::new(
                layout.width,
                layout.height.max(tb::bounded_font_size(opts.font_size)),
            )
        } else {
            // 回退到 BitmapFont 等宽度量（无后端字体可用时）：按字符数，非字节。
            // 度量从 BitmapFont 读取，禁止在此复制魔数造成双源漂移。
            let bitmap = crate::draw::resources::font::bitmap_font::BitmapFont::new();
            let len = text.chars().count() as f32;
            Size::new(len * bitmap.char_width(), bitmap.line_height())
        }
    }

    /// 光栅化一个字形（使用统一缓存）。
    pub fn rasterize_glyph(
        &self,
        font: &FontHandle,
        glyph_id: u32,
        pixel_size: f32,
    ) -> GlyphRaster {
        let Some(pixel_size) = tb::normalized_raster_pixel_size(pixel_size) else {
            return GlyphRaster::empty();
        };
        let pixel_size = pixel_size as f32;

        if glyph_id == tb::WHITESPACE_GLYPH_ID {
            return GlyphRaster::empty();
        }

        // 缺字 tofu：合成空心方框，避免静默丢字。
        if glyph_id == tb::TOFU_GLYPH_ID {
            return Self::rasterize_tofu(pixel_size);
        }

        if !self.text_backend.is_valid(font) {
            return GlyphRaster::empty();
        }

        let ps = pixel_size as u32;
        let key = GlyphCacheKey {
            font_idx: font.0,
            glyph_id,
            pixel_size: ps,
        };

        // 查缓存
        if let Some(cached) = self.glyph_cache.get(&key) {
            return GlyphRaster {
                width: cached.width,
                height: cached.height,
                // Arc 字段克隆仅增加引用计数，覆盖像素数据零拷贝。
                coverage: cached.coverage.clone(),
                bearing_x: cached.bearing_x,
                bearing_y: cached.bearing_y,
                outline_mesh: cached.outline_mesh.clone(),
            };
        }

        // 从后端光栅化
        let raster = self
            .text_backend
            .rasterize_glyph(font, glyph_id, pixel_size);

        // 写入缓存
        if ps > 0 && raster.width > 0 && raster.height > 0 {
            self.glyph_cache.insert(
                key,
                CachedRaster {
                    width: raster.width,
                    height: raster.height,
                    coverage: Arc::clone(&raster.coverage),
                    bearing_x: raster.bearing_x,
                    bearing_y: raster.bearing_y,
                    outline_mesh: raster.outline_mesh.clone(),
                },
            );
        }

        raster
    }

    /// 合成缺字 tofu（空心方框），相对基线的 bearing 与常规字形一致。
    fn rasterize_tofu(pixel_size: f32) -> GlyphRaster {
        let fs = tb::bounded_font_size(pixel_size);
        let w = ((fs * 0.5).round() as usize).max(4);
        let h = ((fs * 0.7).round() as usize).max(5);
        let Some(pixel_count) = w.checked_mul(h) else {
            return GlyphRaster::empty();
        };
        let mut coverage = Vec::new();
        if coverage.try_reserve_exact(pixel_count).is_err() {
            return GlyphRaster::empty();
        }
        coverage.resize(pixel_count, 0u8);
        for x in 0..w {
            coverage[x] = 220;
            coverage[(h - 1) * w + x] = 220;
        }
        for y in 0..h {
            coverage[y * w] = 220;
            coverage[y * w + (w - 1)] = 220;
        }
        GlyphRaster {
            width: w,
            height: h,
            coverage: Arc::<[u8]>::from(coverage),
            bearing_x: (fs * 0.05).max(0.0),
            // 相对布局 y（≈ ascent）：方框顶落在 ascent 下方一点
            bearing_y: -(fs * 0.75),
            outline_mesh: None,
        }
    }

    /// 命中测试，返回 **字符下标**（`chars()` 序）。
    pub fn hit_test_text(
        &self,
        font: &FontHandle,
        text: &str,
        opts: &TextLayoutOptions,
        point: Point,
    ) -> Option<usize> {
        // 为命中结果建立扩展字素簇边界约束。
        let index_map = crate::draw::resources::font::text_index::TextIndexMap::new(text);
        if text.is_empty() {
            return None;
        }
        if self.text_backend.is_valid(font) {
            let f = *font;
            let layout = self.layout_text_shared(&f, text, opts);
            let total_chars = text.chars().count();
            for li in &layout.lines {
                if point.y >= li.y && point.y < li.y + li.height {
                    let end = li.glyph_start + li.glyph_count;
                    let glyphs = &layout.glyphs[li.glyph_start..end.min(layout.glyphs.len())];
                    if glyphs.is_empty() {
                        // 空视觉行的逻辑起点也必须是完整字素簇边界。
                        return Some(
                            // 归一当前行逻辑起点。
                            index_map
                                // 空行向前收敛到所属字素簇起点。
                                .normalize_char(
                                    // 包装真实字符范围内的行起点。
                                    crate::draw::resources::font::text_index::CharIndex(
                                        li.start_char.min(total_chars),
                                    ),
                                    // 指定向后归一。
                                    crate::draw::resources::font::text_index::BoundaryBias::Backward,
                                )
                                // 返回兼容字符下标。
                                .0,
                        );
                    }
                    // 使用视觉 cluster 与行级 UAX #9 方向解析逻辑边界。
                    let raw_index =
                        crate::draw::resources::font::text_backend::glyph_hit_test_index(
                            // 传入当前视觉行字形。
                            glyphs, // 传入行内水平坐标。
                            point.x,
                        )
                        // 所有返回边界都限制在真实源字符数量内。
                        .map(|index| index.min(total_chars));
                    // 把 shaping cluster 命中归一到最近的扩展字素簇边界。
                    return raw_index.map(|index| {
                        // 使用显式字符索引避免与字节偏移混淆。
                        index_map
                            // 点击命中采用最近边界，距离相同时向前。
                            .normalize_char(
                                // 包装后端返回的逻辑字符下标。
                                crate::draw::resources::font::text_index::CharIndex(index),
                                // 指定点击归一偏向。
                                crate::draw::resources::font::text_index::BoundaryBias::Nearest,
                            )
                            // 向旧有公共接口返回字符下标数值。
                            .0
                    });
                }
            }
            if let Some(last) = layout.lines.last() {
                // 行外命中也必须停在完整扩展字素簇边界。
                return Some(
                    // 归一最后一行逻辑终点。
                    index_map
                        // 行尾使用向后偏向，避免越入下一字素簇。
                        .normalize_char(
                            // 包装真实字符范围内的行尾。
                            crate::draw::resources::font::text_index::CharIndex(
                                last.end_char.min(total_chars),
                            ),
                            // 指定行尾向后归一。
                            crate::draw::resources::font::text_index::BoundaryBias::Backward,
                        )
                        // 返回兼容字符下标。
                        .0,
                );
            }
        }
        Some(0)
    }

    /// 获取指定 **字符下标** 的光标 x 位置。
    pub fn text_cursor_x(
        &self,
        font: &FontHandle,
        text: &str,
        opts: &TextLayoutOptions,
        char_index: usize,
    ) -> f32 {
        if text.is_empty() || !self.text_backend.is_valid(font) {
            return 0.0;
        }
        let f = *font;
        // 一次性光标查询流式归一，避免每帧建立两套边界数组。
        let char_index = crate::draw::resources::font::text_index::normalize_char_in_text(
            text,
            // 显式标注输入单位为字符下标。
            crate::draw::resources::font::text_index::CharIndex(char_index),
            // 距离相同时沿前进方向收敛。
            crate::draw::resources::font::text_index::BoundaryBias::Nearest,
        )
        // 继续复用现有布局接口的数值字符下标。
        .0;
        let layout = self.layout_text_shared(&f, text, opts);
        // 逐行从同一视觉 cluster 数据查询方向感知光标边界。
        for line in &layout.lines {
            // 逻辑边界必须落在当前行源范围内。
            if char_index < line.start_char || char_index > line.end_char {
                // 继续检查下一视觉行。
                continue;
            }
            // 取得当前行字形范围。
            let glyph_end = (line.glyph_start + line.glyph_count).min(layout.glyphs.len());
            // 使用共享几何辅助查询 RTL 或 LTR 边界位置。
            if let Some(x) = crate::draw::resources::font::text_backend::glyph_cursor_x(
                // 传入当前视觉行字形。
                &layout.glyphs[line.glyph_start..glyph_end],
                // 传入逻辑光标边界。
                char_index,
            ) {
                // 返回同一布局派生的主光标坐标。
                return x;
            }
        }
        // 落在末尾或缺口：取最后一个 char_index < 目标 的右缘
        layout
            .glyphs
            .iter()
            .rev()
            .find(|g| g.char_index < char_index)
            .map_or(0.0, |g| g.x + g.width.max(0.0))
    }

    /// 获取水平行度量。
    pub fn horizontal_line_metrics(
        &self,
        font: &FontHandle,
        pixel_size: f32,
    ) -> Option<tb::LineMetrics> {
        let pixel_size = tb::normalized_raster_pixel_size(pixel_size)? as f32;
        self.text_backend.horizontal_line_metrics(font, pixel_size)
    }

    /// 返回字体原始数据。
    pub fn font_data(&self, font: &FontHandle) -> Option<Vec<u8>> {
        self.text_backend.font_data(font)
    }

    /// 清空字形位图缓存（释放内存）。
    pub fn clear_glyph_cache(&mut self) {
        self.glyph_cache.clear();
        // 字形与布局缓存共享字体生命周期，显式清理时保持二者一致。
        self.layout_cache.clear();
        self.text_backend.clear_cache();
    }

    /// 返回字体子系统近似内存使用（字节）。
    pub fn memory_usage(&self) -> usize {
        let backend_mem = self.text_backend.memory_usage();
        let cache_mem = self.glyph_cache.memory_usage();
        backend_mem
            .saturating_add(cache_mem)
            .saturating_add(self.layout_cache.memory_usage())
    }
}

// 判断 CSS 通用字体族是否应保留平台当前字体。
fn is_generic_family(family: &str) -> bool {
    // 去除调用边界可能保留的外围空白。
    let family = family.trim();
    // 对固定小集合执行零分配大小写不敏感匹配。
    [
        "system-ui",
        "sans-serif",
        "serif",
        "monospace",
        "cursive",
        "fantasy",
    ]
    // 遍历全部系统 UI 与 CSS 传统通用字体族。
    .iter()
    // 任一名称匹配即保留平台当前字体。
    .any(|generic| family.eq_ignore_ascii_case(generic))
}

impl Default for FontService {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for FontService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontService")
            .field("backend", &self.text_backend)
            .field("primary_family", &self.primary_family)
            .field("registry_size", &self.registry.len())
            .field("fallback_count", &self.fallback_handles.len())
            .field("cache_entries", &self.glyph_cache.len())
            .finish()
    }
}
