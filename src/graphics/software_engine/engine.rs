use super::asset_store::AssetStore;
use super::core::RenderTarget;
use super::fontdue_backend::FontdueBackend;
use crate::diag::{Errc, Error};
use crate::graphics::text_backend::TextBackend;
use crate::graphics::{BlendMode, Color, DirtyRegion, FontHandle, GraphicsEngine, ImageHandle, Radius, TextLayoutOptions};
use crate::base::{Point, Rect, Size};
use crate::graphics::{GradientDirection, Transform};

// ════════════════════════════════════════════════════════════════════════════
// SoftwareEngine — 组合 AssetStore + RenderTarget
// ════════════════════════════════════════════════════════════════════════════

enum ActiveTarget {
    Main,
    Offscreen(usize),
}

pub struct SoftwareEngine {
    pub(crate) rt: RenderTarget,
    pub(crate) assets: AssetStore,
    pub(crate) text_backend: Box<dyn TextBackend>,
    active_target: ActiveTarget,
    main_width: i32,
    main_height: i32,
    saved_pixels: Vec<u32>,
    pre_frame_clip: Rect,
    /// Cached last loaded font handle for the `load_font` GraphicsEngine API.
    loaded_font_handle: FontHandle,
}

impl Default for SoftwareEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SoftwareEngine {
    pub fn new() -> Self {
        Self {
            rt: RenderTarget::new(),
            assets: AssetStore::new(),
            text_backend: Box::new(FontdueBackend::new()),
            active_target: ActiveTarget::Main,
            main_width: 0,
            main_height: 0,
            saved_pixels: Vec::new(),
            pre_frame_clip: Rect::new(0.0, 0.0, f32::MAX, f32::MAX),
            loaded_font_handle: FontHandle::new(0),
        }
    }

    /// Replace the text backend with a custom one (e.g. a FreeType backend).
    /// Must be called before `initialize()` to take effect.
    pub fn with_text_backend(mut self, backend: Box<dyn TextBackend>) -> Self {
        self.text_backend = backend;
        self
    }

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

    // ── 字体加载辅助 ──

    /// Try to load a font from raw bytes, validating that glyphs can actually be
    /// rasterized. Some font formats (e.g. CFF2-based variable fonts) parse
    /// successfully but produce empty glyphs, causing all text to disappear.
    /// We test-rasterize a common glyph to catch such broken fonts.
    ///
    /// Validation happens before the font enters the backend's store, so a
    /// broken font file is never committed.
    ///
    /// Returns `true` if the font was loaded and usable.
    fn try_load_user_font(&mut self, data: Vec<u8>, size: f32) -> bool {
        // Pre-validation: create a temporary fontdue::Font and test a glyph.
        // fontdue::Font owns all glyph outlines — it does NOT borrow from `data`,
        // so the temporary Font can be safely dropped after validation.
        let Ok(ref test_font) = fontdue::Font::from_bytes(
            data.as_slice(),
            fontdue::FontSettings::default(),
        ) else {
            return false;
        };
        let (metrics, _) = test_font.rasterize('A', size);
        if metrics.width == 0 || metrics.height == 0 {
            // Font produces empty glyphs (e.g. CFF2 variable font without gvar
            // support in ttf-parser). Skip silently — the bitmap font fallback
            // at the end will still provide basic text.
            return false;
        }
        // Font is valid — load it into the text backend.
        self.text_backend.load_font(&data).is_ok()
    }

    // ── 系统字体自动检测 ──

    /// Auto-detect and load the OS system default font.
    ///
    /// Strategy per platform:
    /// - **Windows**: scans `%WINDIR%\Fonts\` for `.ttf`/`.ttc` files, prioritising
    ///   system UI fonts (Segoe UI, Microsoft Sans Serif, Arial).
    /// - **Linux**: runs `fc-match` (fontconfig), then recursively scans common font
    ///   directories.  Prefers non-variable TrueType fonts because fontdue 0.9 does
    ///   not render CFF2 variable fonts correctly.
    /// - **macOS**: scans `/System/Library/Fonts/` and `/Library/Fonts/`.
    ///
    /// The loaded font becomes `FontHandle(0)` — the default.
    /// If no font is found, the built-in bitmap font is used as fallback.
    pub fn load_default_system_font(&mut self, size: f32) {
        // ── Build font directory list ──
        let (font_dirs, preferred_names): (Vec<String>, &[&str]) = if cfg!(windows) {
            let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
            (
                vec![format!(r"{}\Fonts", windir)],
                &[
                    "segoeui.ttf", "segoeuib.ttf", "segoeuii.ttf",
                    "segoeuiz.ttf", "seguisb.ttf", "seguibl.ttf",
                    "Segoe UI.ttf", "SegoeUIVariable.ttf",
                    "micross.ttf", "arial.ttf", "arialbd.ttf",
                    "msyh.ttc", "msyhbd.ttc",
                ],
            )
        } else if cfg!(target_os = "macos") {
            (
                vec!["/System/Library/Fonts".into(), "/Library/Fonts".into()],
                &[
                    "SFNS.ttf", "SFNSDisplay.ttf",
                    "Helvetica.ttf", "HelveticaNeue.ttc",
                    "Arial.ttf",
                ],
            )
        } else {
            (
                vec!["/usr/share/fonts".into()],
                // Prefer non-variable TrueType fonts known to work with fontdue.
                // CFF2-based variable fonts (e.g. NotoSansCJK-VF.ttc) parse
                // successfully but produce empty glyphs.
                &[
                    "liberation-sans-fonts/LiberationSans-Regular.ttf",
                    "liberation-sans-fonts/LiberationSans-Bold.ttf",
                    "open-sans/OpenSans-Regular.ttf",
                    "google-droid-sans-fonts/DroidSansFallbackFull.ttf",
                    "google-droid-sans-fonts/DroidSans.ttf",
                ],
            )
        };

        // Primary: try preferred UI font filenames in each font directory.
        for dir in &font_dirs {
            let dir_path = std::path::Path::new(dir);
            if !dir_path.is_dir() {
                continue;
            }
            for name in preferred_names {
                let path = dir_path.join(name);
                if let Ok(data) = std::fs::read(&path) {
                    if self.try_load_user_font(data, size) {
                        crate::diag::log::info_fn(format!("Loaded font: {}", path.display()));
                        return;
                    }
                }
            }
        }

        // Fallback 1 (Linux): fc-match discover.
        #[cfg(target_os = "linux")]
        {
            if let Ok(output) = std::process::Command::new("fc-match")
                .args(["-f", "%{file}\n", "sans-serif"])
                .output()
            {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !path.is_empty() && path != "(null)" {
                        if let Ok(data) = std::fs::read(&path) {
                            if self.try_load_user_font(data, size) {
                                crate::diag::log::info_fn(format!("Loaded font: {}", path));
                                return;
                            }
                        }
                    }
                }
            }
        }

        // Fallback 2: scan directories for any loadable .ttf / .ttc / .otf file.
        for dir in &font_dirs {
            let dir_path = std::path::Path::new(dir);
            if !dir_path.is_dir() {
                continue;
            }
            if let Ok(entries) = std::fs::read_dir(dir_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let ext = path.extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.to_lowercase())
                        .unwrap_or_default();
                    if ext != "ttf" && ext != "ttc" && ext != "otf" {
                        continue;
                    }
                    if let Ok(data) = std::fs::read(&path) {
                        if self.try_load_user_font(data, size) {
                            crate::diag::log::info_fn(format!("Loaded font: {}", path.display()));
                            return;
                        }
                    }
                }
            }
        }

        // Fallback 3 (Linux): shallow recursive scan one level deeper.
        #[cfg(target_os = "linux")]
        {
            for dir in &font_dirs {
                let dir_path = std::path::Path::new(dir);
                if !dir_path.is_dir() {
                    continue;
                }
                if let Ok(entries) = std::fs::read_dir(dir_path) {
                    for entry in entries.flatten() {
                        let sub = entry.path();
                        if !sub.is_dir() {
                            continue;
                        }
                        if let Ok(sub_entries) = std::fs::read_dir(&sub) {
                            for sub_entry in sub_entries.flatten() {
                                let path = sub_entry.path();
                                let ext = path.extension()
                                    .and_then(|e| e.to_str())
                                    .map(|e| e.to_lowercase())
                                    .unwrap_or_default();
                                if ext != "ttf" && ext != "ttc" && ext != "otf" {
                                    continue;
                                }
                                if let Ok(data) = std::fs::read(&path) {
                                    if self.try_load_user_font(data, size) {
                                        crate::diag::log::info_fn(
                                            format!("Loaded font: {}", path.display())
                                        );
                                        return;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        crate::diag::log::info_fn("No system font, using bitmap fallback");
    }
}

// ════════════════════════════════════════════════════════════════════════════
// GraphicsEngine trait 实现
// ════════════════════════════════════════════════════════════════════════════

impl GraphicsEngine for SoftwareEngine {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.main_width = width;
        self.main_height = height;
        self.rt.initialize(width, height);
        self.active_target = ActiveTarget::Main;
        // Auto-load system font as default (non-fatal if none found)
        self.load_default_system_font(14.0);
        Ok(())
    }

    fn shutdown(&mut self) {
        self.rt = RenderTarget::new();
        self.assets.shutdown();
        self.main_width = 0;
        self.main_height = 0;
        self.active_target = ActiveTarget::Main;
        self.saved_pixels.clear();
    }

    fn resize(&mut self, width: i32, height: i32) {
        self.main_width = width;
        self.main_height = height;
        self.rt.initialize(width, height);
        self.active_target = ActiveTarget::Main;
    }

    // ── 帧控制 ──

    fn begin_frame(&mut self, dirty: &DirtyRegion) {
        self.pre_frame_clip = self.rt.clip_rect();
        self.rt.clip_stack_mut().clear();
        let tw = self.rt.width();
        let th = self.rt.height();

        if dirty.full_frame {
            self.rt.clear_all();
            *self.rt.clip_rect_mut() = Rect::new(0.0, 0.0, tw as f32, th as f32);
        } else if dirty.clear_required {
            let r = dirty.rect;
            let x = r.x.max(0.0) as i32;
            let y = r.y.max(0.0) as i32;
            let w = (r.w as i32).min(tw - x);
            let h = (r.h as i32).min(th - y);
            if w > 0 && h > 0 {
                self.rt.clear_region(x, y, w, h);
            }
            *self.rt.clip_rect_mut() = Rect::new(
                r.x.max(0.0),
                r.y.max(0.0),
                (r.w).min(tw as f32 - r.x.max(0.0)),
                (r.h).min(th as f32 - r.y.max(0.0)),
            );
        } else {
            *self.rt.clip_rect_mut() = Rect::new(0.0, 0.0, tw as f32, th as f32);
        }
    }

    fn end_frame(&mut self, _dirty: &DirtyRegion) {
        *self.rt.clip_rect_mut() = self.pre_frame_clip;
        self.rt.clip_stack_mut().clear();
    }

    fn scroll_region(&mut self, viewport: Rect, dy: f32) {
        self.rt.scroll_region(viewport, dy);
    }

    fn push_clip_rect(&mut self, rect: Rect) {
        self.rt.push_clip_rect(rect);
    }
    fn pop_clip_rect(&mut self) {
        self.rt.pop_clip_rect();
    }
    fn set_opacity(&mut self, opacity: f32) {
        self.rt.set_opacity(opacity);
    }
    fn opacity(&self) -> f32 {
        self.rt.opacity_val()
    }
    fn save(&mut self) {
        self.rt.save();
    }
    fn restore(&mut self) {
        self.rt.restore();
    }
    fn set_transform(&mut self, t: Transform) {
        self.rt.set_transform(t);
    }
    fn reset_transform(&mut self) {
        self.rt.reset_transform();
    }
    fn set_blend_mode(&mut self, mode: BlendMode) {
        self.rt.set_blend_mode(mode);
    }

    fn fill_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
        self.rt.fill_rect(rect, color, radius);
    }

    fn stroke_rect(&mut self, rect: Rect, color: Color, line_width: f32, radius: Option<Radius>) {
        self.rt.stroke_rect(rect, color, line_width, radius);
    }

    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        self.rt.fill_circle(cx, cy, r, color);
    }

    fn fill_circle_radial(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        self.rt.fill_circle_radial(cx, cy, r, color);
    }

    fn fill_sector(
        &mut self,
        cx: f32, cy: f32,
        r: f32,
        start_angle: f32,
        end_angle: f32,
        color: Color,
    ) {
        self.rt.fill_sector(cx, cy, r, start_angle, end_angle, color);
    }

    fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color, line_width: f32) {
        self.rt.stroke_circle(cx, cy, r, color, line_width);
    }

    fn fill_ellipse(&mut self, rect: Rect, color: Color) {
        self.rt.fill_ellipse(rect, color);
    }

    fn draw_box_shadow(
        &mut self,
        rect: Rect,
        blur_radius: f32,
        offset_x: f32,
        offset_y: f32,
        color: Color,
        corner_radius: Option<Radius>,
    ) {
        self.rt
            .draw_box_shadow(rect, blur_radius, offset_x, offset_y, color, corner_radius);
    }

    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32) {
        self.rt.draw_line(x1, y1, x2, y2, color, width);
    }

    fn fill_linear_gradient(
        &mut self,
        rect: Rect,
        color_a: Color,
        color_b: Color,
        dir: GradientDirection,
    ) {
        self.rt.fill_linear_gradient(rect, color_a, color_b, dir);
    }

    fn fill_radial_gradient(
        &mut self,
        cx: f32,
        cy: f32,
        inner_r: f32,
        outer_r: f32,
        inner_color: Color,
        outer_color: Color,
    ) {
        self.rt
            .fill_radial_gradient(cx, cy, inner_r, outer_r, inner_color, outer_color);
    }

    fn load_font(&mut self, data: &[u8], _size: f32) -> Result<&mut FontHandle, Error> {
        let handle = self.text_backend.load_font(data)?;
        self.loaded_font_handle = handle;
        Ok(&mut self.loaded_font_handle)
    }

    fn unload_font(&mut self, font: &FontHandle) {
        self.text_backend.unload_font(font);
    }

    fn measure_text(&self, font: &FontHandle, text: &str, opts: &TextLayoutOptions) -> Size {
        if self.text_backend.is_valid(font) {
            let backend_opts = crate::graphics::text_backend::TextLayoutOptions::from(opts.clone());
            let layout = self.text_backend.layout_text(font, text, &backend_opts);
            Size::new(layout.width, layout.height.max(opts.font_size))
        } else {
            self.assets.bitmap_font().measure(text, opts)
        }
    }

    fn draw_text(
        &mut self,
        font: &FontHandle,
        text: &str,
        pos: Point,
        color: Color,
        opts: &TextLayoutOptions,
    ) {
        if !self.text_backend.is_valid(font) {
            self.rt.draw_bitmap_text(text, pos, color);
            return;
        }
        if text.is_empty() {
            return;
        }

        let fs = opts.font_size.max(1.0);
        let premul = self.rt.apply_opacity(RenderTarget::premul(color));
        let backend_opts = crate::graphics::text_backend::TextLayoutOptions::from(opts.clone());
        let layout = self.text_backend.layout_text(font, text, &backend_opts);

        // When VAlign::Top (no max_height), fontdue glyphs may have y > 0,
        // shifting text downward. Compute min_y to adjust all glyphs up.
        let is_top = matches!(opts.v_align, crate::graphics::VAlign::Top);
        let y_off = if is_top && !layout.glyphs.is_empty() {
            layout
                .glyphs
                .iter()
                .map(|g| g.y)
                .fold(f32::MAX, f32::min)
                .min(0.0) as i32
        } else {
            0
        };

        for gp in &layout.glyphs {
            let raster = self.text_backend.rasterize_glyph(font, gp.glyph_id, fs);
            if raster.width == 0 || raster.height == 0 {
                continue;
            }
            let gx = (pos.x + gp.x) as i32;
            let gy = (pos.y + gp.y) as i32 - y_off;
            let c = premul;

            for row in 0..raster.height {
                let sy = gy + row as i32;
                for col in 0..raster.width {
                    let cov = raster.coverage[row * raster.width + col];
                    if cov == 0 {
                        continue;
                    }
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

        // Debug auxiliary lines (UIX_TTF_DEBUG=1)
        if std::env::var("UIX_TTF_DEBUG").as_deref() == Ok("1") && !layout.glyphs.is_empty() {
            let debug_color = 0x80FF0000u32;
            let baseline_color = 0xFF0088FFu32;
            let _advance_color = 0xFFFF00FFu32;
            for gp in &layout.glyphs {
                let raster = self.text_backend.rasterize_glyph(font, gp.glyph_id, fs);
                let bx = (pos.x + gp.x) as i32;
                let by = (pos.y + gp.y) as i32 - y_off;
                let bw = raster.width as i32;
                let bh = raster.height as i32;
                for row in 0..bh {
                    let sy = by + row;
                    if row == 0 || row == bh - 1 {
                        for col in 0..bw {
                            self.rt.put_pixel_raw(bx + col, sy, debug_color);
                        }
                    } else {
                        self.rt.put_pixel_raw(bx, sy, debug_color);
                        self.rt.put_pixel_raw(bx + bw - 1, sy, debug_color);
                    }
                }
            }
            // Blue baseline
            let bl_y = pos.y as i32
                + self
                    .text_backend
                    .horizontal_line_metrics(font, fs)
                    .map(|m| m.ascent as i32)
                    .unwrap_or((fs * 0.8) as i32);
            for x in (pos.x as i32)..=(pos.x + 200.0) as i32 {
                self.rt.put_pixel_raw(x, bl_y, baseline_color);
                self.rt.put_pixel_raw(x, bl_y + 1, baseline_color);
            }
        }
    }

    fn load_image(&mut self, data: &[u8]) -> Result<&mut ImageHandle, Error> {
        let img = image::load_from_memory(data)
            .map_err(|e| Error::new(Errc::FormatError, format!("cannot decode image: {}", e)))?;
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let pixels: Vec<u32> = rgba
            .chunks_exact(4)
            .map(|p| {
                let a = p[3] as u32;
                let r = (p[0] as u32 * a / 255).min(255);
                let g = (p[1] as u32 * a / 255).min(255);
                let b = (p[2] as u32 * a / 255).min(255);
                (a << 24) | (r << 16) | (g << 8) | b
            })
            .collect();
        Ok(self.assets.load_image(pixels, w as i32, h as i32))
    }

    fn unload_image(&mut self, _image: &ImageHandle) {}

    fn image_size(&self, image: &ImageHandle) -> Size {
        if let Some((w, h)) = self.assets.find_any_size(image) {
            Size::new(w as f32, h as f32)
        } else {
            Size::new(0.0, 0.0)
        }
    }

    fn draw_image(&mut self, image: &ImageHandle, src: Rect, dst: Rect) {
        if let Some((pixels, w, h)) = self.assets.find_image(image) {
            let src_clone = src;
            let dst_clone = dst;
            let op = self.rt.opacity_val();
            self.rt.blit_image(pixels, w, h, &src_clone, &dst_clone, op);
            return;
        }
        if let Some((pixels, w, h)) = self.assets.find_offscreen(image) {
            let src_clone = src;
            let dst_clone = dst;
            let op = self.rt.opacity_val();
            self.rt.blit_image(pixels, w, h, &src_clone, &dst_clone, op);
        }
    }

    fn create_offscreen(&mut self, w: i32, h: i32) -> Result<&mut ImageHandle, Error> {
        self.assets.create_offscreen(w, h)
    }

    fn destroy_offscreen(&mut self, _offscreen: &ImageHandle) {}

    fn begin_offscreen(&mut self, offscreen: &ImageHandle) {
        if let Some(idx) = self.assets.offscreen_slot_index(offscreen) {
            self.saved_pixels = self.rt.take_pixels();
            let (pixels, w, h) = self.assets.take_offscreen_pixels(idx);
            self.rt.set_pixels(pixels, w, h);
            self.active_target = ActiveTarget::Offscreen(idx);
        }
    }

    fn end_offscreen(&mut self) {
        if let ActiveTarget::Offscreen(idx) = self.active_target {
            let pixels = self.rt.take_pixels();
            self.assets.return_offscreen_pixels(idx, pixels);
            self.rt.set_pixels(
                std::mem::take(&mut self.saved_pixels),
                self.main_width,
                self.main_height,
            );
            self.active_target = ActiveTarget::Main;
        }
    }

    fn pixels(&self) -> &[u32] {
        self.rt.pixels()
    }
    fn width(&self) -> i32 {
        self.main_width
    }
    fn height(&self) -> i32 {
        self.main_height
    }
    fn set_supersample_level(&mut self, level: u8) {
        self.rt.set_supersample_level(level);
    }
    fn supersample_level(&self) -> u8 {
        self.rt.supersample_level()
    }
}
