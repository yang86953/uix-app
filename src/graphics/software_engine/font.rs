use super::core::RenderTarget;
use super::engine::SoftwareEngine;
use crate::base::{Point, Size};
use crate::graphics::{FontHandle, TextLayoutOptions};

// ════════════════════════════════════════════════════════════════════════════
// SoftwareEngine — 字体相关内部方法
// ════════════════════════════════════════════════════════════════════════════

impl SoftwareEngine {
    /// 设置首选字体族名称。
    ///
    /// 必须在调用 `initialize()` **之前**调用才能生效。
    /// 如果从未调用，`load_default_system_font()` 会自动使用系统默认无衬线字体。
    ///
    /// 字体系列名可以是具体字体名（如 "Noto Sans SC"、"Segoe UI"），
    /// 也可以是 CSS 通用家族名（"sans-serif"、"serif"、"monospace"）。
    pub fn set_font_family(&mut self, family: impl Into<String>) {
        self.primary_family = family.into();
        self.user_family_set = true;
    }

    // ── 字体加载 ──
    ///
    /// - 未配置字体：仅通过平台层（Linux: fc-match）取系统默认，不扫描
    /// - 配置了字体族：通过平台层按名称查询字体，fc-match 解析路径
    pub fn load_default_system_font(&mut self, size: f32) {
        if self.user_family_set {
            // 用户指定了字体族：fc-match 解析路径，不扫描
            let family = self.primary_family.clone();
            let found = self.load_family_font(&family, size);
            if found.is_some() { return; }

            #[cfg(target_os = "linux")]
            {
                // 尝试 sans-serif 兜底（用户配置的字体可能不在系统中）
                let fallback = crate::platform::system_info::probe_system_default_font();
                if let Some(path) = fallback {
                    if let Ok(data) = std::fs::read(&path) {
                        if self.load_raw_font(data, size).is_some() {
                            crate::diag::log::info_fn(format!(
                                "Configured font '{}' not found, fallback: {}",
                                self.primary_family, path
                            ));
                            return;
                        }
                    }
                }
            }
        } else {
            // 未配置：平台层取系统默认，不扫描
            if let Some(path) = crate::platform::system_info::probe_system_default_font() {
                if let Ok(data) = std::fs::read(&path) {
                    if self.load_raw_font(data, size).is_some() {
                        crate::diag::log::info_fn(format!("Loaded system default font: {}", path));
                        return;
                    }
                }
            }
        }
        crate::diag::log::info_fn("No primary font, using bitmap fallback");
    }

    /// 通过平台层按字体族名称查找并加载字体（fc-match，不扫描）。
    fn load_family_font(&mut self, family: &str, size: f32) -> Option<()> {
        #[cfg(target_os = "linux")]
        {
            let pattern = format!("{}:scalable=true", family);
            let path = crate::platform::linux::system_info::probe_font_path_via_fc_match(&pattern)
                .or_else(|| {
                    let p2 = format!("{}", family);
                    crate::platform::linux::system_info::probe_font_path_via_fc_match(&p2)
                });
            if let Some(p) = path {
                if let Ok(data) = std::fs::read(&p) {
                    if self.load_raw_font(data, size).is_some() {
                        crate::diag::log::info_fn(format!("Loaded family font '{}': {}", family, p));
                        return Some(());
                    }
                }
            }
        }
        // 非 Linux 暂不支持按族名查找，返回 None
        #[cfg(not(target_os = "linux"))]
        {
            let _ = family; let _ = size;
        }
        None
    }

    /// 直接加载原始字体数据（跳过 fontdb，仅 fontdue 验证）。
    fn load_raw_font(&mut self, data: Vec<u8>, size: f32) -> Option<FontHandle> {
        let Ok(ref test_font) = fontdue::Font::from_bytes(
            data.as_slice(),
            fontdue::FontSettings::default(),
        ) else { return None };
        let (metrics, _) = test_font.rasterize('A', size);
        if metrics.width == 0 || metrics.height == 0 { return None; }
        self.text_backend.load_font(&data).ok()
    }

    /// 用指定字体测量文本宽度。
    pub(crate) fn measure_with_font(&self, font: &FontHandle, text: &str, opts: &TextLayoutOptions) -> Size {
        if self.text_backend.is_valid(font) {
            let backend_opts = crate::graphics::text_backend::TextLayoutOptions::from(opts.clone());
            let layout = self.text_backend.layout_text(font, text, &backend_opts);
            Size::new(layout.width, layout.height.max(opts.font_size))
        } else {
            self.assets.bitmap_font().measure(text, opts)
        }
    }

    /// 用指定字体渲染一段文本（内部实现，不涉及字体回退）。
    pub(crate) fn render_text_segment(
        &mut self,
        font: &FontHandle,
        text: &str,
        pos: Point,
        color: crate::graphics::Color,
        opts: &TextLayoutOptions,
    ) {
        if !self.text_backend.is_valid(font) {
            self.rt.draw_bitmap_text(text, pos, color);
            return;
        }
        if text.is_empty() { return; }

        let fs = opts.font_size.max(1.0);
        let premul = self.rt.apply_opacity(RenderTarget::premul(color));
        let backend_opts = crate::graphics::text_backend::TextLayoutOptions::from(opts.clone());
        let layout = self.text_backend.layout_text(font, text, &backend_opts);

        let is_top = matches!(opts.v_align, crate::graphics::VAlign::Top);
        let y_off = if is_top && !layout.glyphs.is_empty() {
            layout.glyphs.iter().map(|g| g.y).fold(f32::MAX, f32::min).min(0.0) as i32
        } else { 0 };

        for gp in &layout.glyphs {
            let raster = self.text_backend.rasterize_glyph(font, gp.glyph_id, fs);
            if raster.width == 0 || raster.height == 0 { continue; }
            let gx = (pos.x + gp.x) as i32;
            let gy = (pos.y + gp.y) as i32 - y_off;
            let c = premul;
            for row in 0..raster.height {
                let sy = gy + row as i32;
                for col in 0..raster.width {
                    let cov = raster.coverage[row * raster.width + col];
                    if cov == 0 { continue; }
                    let cov_u32 = cov as u32;
                    let alpha = (c >> 24) & 0xFF;
                    let blended_alpha = (alpha * cov_u32 / 255).min(255);
                    let r = ((c >> 16) & 0xFF) * cov_u32 / 255;
                    let g = ((c >> 8) & 0xFF) * cov_u32 / 255;
                    let b = (c & 0xFF) * cov_u32 / 255;
                    let pixel = (blended_alpha << 24) | (r << 16) | (g << 8) | b;
                    self.rt.put_pixel_raw(gx + col as i32, sy, pixel);
                }
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// GraphicsEngine trait — 字体相关方法
// ════════════════════════════════════════════════════════════════════════════

