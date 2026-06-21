use super::engine::SoftwareEngine;
use crate::base::{Rect, Size};
use crate::diag::Error;
use crate::graphics::font_service::FontService;
use crate::graphics::{
    BlendMode, Color, DirtyRegion, FontHandle, GraphicsEngine, ImageHandle, Radius,
    TextLayoutOptions,
};
use crate::graphics::{GradientDirection, Transform};

// ════════════════════════════════════════════════════════════════════════════
// GraphicsEngine trait — 核心图形方法
// ════════════════════════════════════════════════════════════════════════════

impl GraphicsEngine for SoftwareEngine {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.main_width = width;
        self.main_height = height;
        self.rt.initialize(width, height);
        self.active_target = super::engine::ActiveTarget::Main;
        // 自动加载系统默认字体（委托给独立的 FontService）
        self.font_service.load_default_system_font(14.0);
        Ok(())
    }

    fn shutdown(&mut self) {
        self.rt = super::core::RenderTarget::new();
        self.assets.shutdown();
        self.main_width = 0;
        self.main_height = 0;
        self.active_target = super::engine::ActiveTarget::Main;
    }

    fn resize(&mut self, width: i32, height: i32) {
        self.main_width = width;
        self.main_height = height;
        self.rt.initialize(width, height);
        self.active_target = super::engine::ActiveTarget::Main;
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
            self.rt.sync_clip_int();
        } else if dirty.clear_required && !dirty.rects().is_empty() {
            // 逐矩形清除：只清理实际脏区域，而非合并的 bounds
            let bounds = dirty.bounds();
            for &r in dirty.rects() {
                let x = r.x.max(0.0) as i32;
                let y = r.y.max(0.0) as i32;
                let w = (r.w as i32).min(tw - x);
                let h = (r.h as i32).min(th - y);
                if w > 0 && h > 0 {
                    self.rt.clear_region(x, y, w, h);
                }
            }
            // Clip 使用 bounds（外接矩形），确保所有脏区域内的绘制都被覆盖
            *self.rt.clip_rect_mut() = Rect::new(
                bounds.x.max(0.0),
                bounds.y.max(0.0),
                (bounds.w).min(tw as f32 - bounds.x.max(0.0)),
                (bounds.h).min(th as f32 - bounds.y.max(0.0)),
            );
            self.rt.sync_clip_int();
        } else {
            *self.rt.clip_rect_mut() = Rect::new(0.0, 0.0, tw as f32, th as f32);
            self.rt.sync_clip_int();
        }
    }

    fn end_frame(&mut self, _dirty: &DirtyRegion) {
        *self.rt.clip_rect_mut() = self.pre_frame_clip;
        self.rt.sync_clip_int();
        self.rt.clip_stack_mut().clear();
    }

    fn scroll_region(&mut self, viewport: Rect, dx: f32, dy: f32) {
        self.rt.scroll_region(viewport, dx, dy);
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

    // ── 形状绘制 ──

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
        cx: f32,
        cy: f32,
        r: f32,
        start_angle: f32,
        end_angle: f32,
        color: Color,
    ) {
        self.rt
            .fill_sector(cx, cy, r, start_angle, end_angle, color);
    }

    fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color, line_width: f32) {
        self.rt.stroke_circle(cx, cy, r, color, line_width);
    }

    fn fill_ellipse(&mut self, rect: Rect, color: Color) {
        self.rt.fill_ellipse(rect, color);
    }

    // ── 阴影 ──

    fn draw_box_shadow(
        &mut self,
        rect: Rect,
        blur_radius: f32,
        offset_x: f32,
        offset_y: f32,
        color: Color,
        corner_radius: Option<Radius>,
    ) {
        // 阴影在当前 clip 区域内绘制，不绕过子 clip。
        // 这样 ScrollView 视口内的 Card 阴影会被视口裁剪（正确行为），
        // 而非 ScrollView 容器内的阴影可以自然溢出到容器外。
        self.rt
            .draw_box_shadow(rect, blur_radius, offset_x, offset_y, color, corner_radius);
    }

    fn draw_box_shadow_ambient(
        &mut self,
        rect: Rect,
        blur_radius: f32,
        offset_x: f32,
        offset_y: f32,
        color: Color,
        corner_radius: Option<Radius>,
    ) {
        // 同 draw_box_shadow，在当前 clip 区域内绘制，不绕过子 clip。
        self.rt.draw_box_shadow_ambient(
            rect,
            blur_radius,
            offset_x,
            offset_y,
            color,
            corner_radius,
        );
    }

    // ── 路径 ──

    fn fill_path(
        &mut self,
        path: &crate::graphics::path::Path,
        color: Color,
        fill_rule: crate::graphics::path::FillRule,
    ) {
        let c = self.rt.apply_opacity(color.premultiplied());
        let polys = crate::graphics::flattener::flatten(path.segments(), 0.25);
        let clip = self.rt.clip_rect();
        self.rt
            .fill_polygons_with_opacity(&polys, clip, c, fill_rule);
    }

    fn stroke_path(
        &mut self,
        path: &crate::graphics::path::Path,
        color: Color,
        options: &crate::graphics::stroker::StrokeOptions,
    ) {
        let stroked = crate::graphics::stroker::stroke_path(path, options);
        self.fill_path(&stroked, color, crate::graphics::path::FillRule::NonZero);
    }

    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32) {
        self.rt.draw_line(x1, y1, x2, y2, color, width);
    }

    // ── 渐变 ──

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

    // ── 图片 ──

    fn load_image(
        &mut self,
        pixels: Vec<u32>,
        width: i32,
        height: i32,
    ) -> Result<&mut ImageHandle, Error> {
        if width <= 0 || height <= 0 || pixels.len() < (width * height) as usize {
            return Err(Error::new(
                crate::diag::Errc::InvalidArgument,
                format!("load_image: invalid size {}x{} / {} pixels", width, height, pixels.len()),
            ));
        }
        Ok(self.assets.load_image(pixels, width, height))
    }

    fn unload_image(&mut self, image: &ImageHandle) {
        self.assets.remove_image(image);
    }

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

    // ── 离屏渲染 ──

    fn create_offscreen(&mut self, w: i32, h: i32) -> Result<&mut ImageHandle, Error> {
        self.assets.create_offscreen(w, h)
    }

    fn destroy_offscreen(&mut self, offscreen: &ImageHandle) {
        self.assets.remove_offscreen(offscreen);
    }

    fn begin_offscreen(&mut self, offscreen: &ImageHandle) {
        if let Some(idx) = self.assets.offscreen_slot_index(offscreen) {
            // 直接交换像素缓冲（零拷贝），离屏槽临时保存主缓冲内容。
            let (mut w, mut h) = (0i32, 0i32);
            self.assets.swap_offscreen_pixels(
                idx,
                &mut self.rt.pixels,
                &mut self.rt.width,
                &mut self.rt.height,
            );
            self.rt.sync_clip_int();
            self.main_width = w;
            self.main_height = h;
            self.active_target = super::engine::ActiveTarget::Offscreen(idx);
        }
    }

    fn end_offscreen(&mut self) {
        if let super::engine::ActiveTarget::Offscreen(idx) = self.active_target {
            let (mut w, mut h) = (self.main_width, self.main_height);
            self.assets.swap_offscreen_pixels(
                idx,
                &mut self.rt.pixels,
                &mut self.rt.width,
                &mut self.rt.height,
            );
            self.rt.sync_clip_int();
            self.main_width = w;
            self.main_height = h;
            self.active_target = super::engine::ActiveTarget::Main;
        }
    }

    // ── 查询 ──

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

    // ── 渲染帧 ──

    fn load_font(&mut self, data: &[u8]) -> Result<FontHandle, Error> {
        self.font_service.load_font(data)
    }

    /// 返回引擎持有的字体服务引用（供 LayerTree 等组件使用）。
    fn font_service(&self) -> &FontService {
        &self.font_service
    }

    // ── 文本测量（布局阶段辅助，委托给 FontService）──

    fn measure_text(&self, font: &FontHandle, text: &str, opts: &TextLayoutOptions) -> Size {
        let backend_opts = crate::graphics::text_backend::TextLayoutOptions::from(opts.clone());
        self.font_service.measure_text(font, text, &backend_opts)
    }

    // ── 字形绘制（渲染器唯一需要的字体方法）──

    fn draw_glyph_raster(
        &mut self,
        x: i32,
        y: i32,
        coverage: &[u8],
        width: usize,
        height: usize,
        color: Color,
    ) {
        let premul = self
            .rt
            .apply_opacity(crate::graphics::software_engine::core::RenderTarget::premul(color));
        let c = premul;
        let w = self.rt.width;
        let h = self.rt.height;
        let stride = w as usize;
        let (cx0, cy0, cx1, cy1) = self.rt.clip_int;
        let c_bounds = (cx0.max(0), cy0.max(0), cx1.min(w), cy1.min(h));

        let src_a = (c >> 24) & 0xFF;
        if src_a == 0 {
            return;
        }
        let src_r = (c >> 16) & 0xFF;
        let src_g = (c >> 8) & 0xFF;
        let src_b = c & 0xFF;

        for row in 0..height {
            let sy = y + row as i32;
            if sy < c_bounds.1 || sy >= c_bounds.3 {
                continue;
            }
            let row_start = row * width;
            let row_offset = sy as usize * stride;

            for col in 0..width {
                let cov = coverage[row_start + col];
                if cov == 0 {
                    continue;
                }
                let sx = x + col as i32;
                if sx < c_bounds.0 || sx >= c_bounds.2 {
                    continue;
                }
                let idx = row_offset + sx as usize;
                let cov_u32 = cov as u32;

                // 直接内联 alpha 混合，避免函数调用开销
                let blended_a = (src_a * cov_u32 / 255).min(255);
                if blended_a == 0 {
                    continue;
                }
                let blended_r = (src_r * cov_u32 / 255).min(255);
                let blended_g = (src_g * cov_u32 / 255).min(255);
                let blended_b = (src_b * cov_u32 / 255).min(255);

                let dst = self.rt.pixels[idx];
                let dst_a = (dst >> 24) & 0xFF;
                if dst_a == 0 {
                    // 透明背景：直接写入（最常见的 glyph 渲染场景）
                    self.rt.pixels[idx] = (blended_a << 24) | (blended_r << 16) | (blended_g << 8) | blended_b;
                } else {
                    // 有内容的背景：正确 alpha 混合
                    let out_a = blended_a + dst_a - (blended_a * dst_a / 255);
                    let out_r = blended_r + ((dst >> 16) & 0xFF) * (255 - blended_a) / 255;
                    let out_g = blended_g + ((dst >> 8) & 0xFF) * (255 - blended_a) / 255;
                    let out_b = blended_b + (dst & 0xFF) * (255 - blended_a) / 255;
                    self.rt.pixels[idx] = (out_a.min(255) << 24)
                        | (out_r.min(255) << 16)
                        | (out_g.min(255) << 8)
                        | out_b.min(255);
                }
            }
        }
    }

    fn diagnose_memory(&self) {
        let pixel_buf = self.rt.pixel_buffer_bytes();
        let offscreen = self.assets.offscreen_bytes();
        let images = self.assets.image_bytes();
        let clip_s = self.rt.clip_stack_bytes();
        let state_s = self.rt.state_stack_bytes();
        let font_mem = self.font_service.memory_usage();
        let total = pixel_buf + offscreen + images + clip_s + state_s + font_mem;

        // Windows 进程实际内存
        #[cfg(windows)]
        let (ws, priv_bytes) = crate::platform::windows::system_info::get_process_memory();
        #[cfg(not(windows))]
        let (ws, priv_bytes) = (0, 0);

        log::info!("=== Engine 内存诊断 ===");
        log::info!("  Engine 已知内存:  {:>7} KB ({:.1} MB)",
            total / 1024, total as f64 / 1_048_576.0);
        if ws > 0 {
            let gap = ws - total;
            log::info!("  Windows 工作集:    {:>7} KB ({:.1} MB)",
                ws / 1024, ws as f64 / 1_048_576.0);
            log::info!("  Windows 私有字节:  {:>7} KB ({:.1} MB)",
                priv_bytes / 1024, priv_bytes as f64 / 1_048_576.0);
            log::info!("  ────────────────────────────────────");
            log::info!("  差距(WS - 已知):  {:>7} KB ({:.1} MB)",
                gap / 1024, gap as f64 / 1_048_576.0);
            log::info!("  其中: DIB ~3.8MB, fontdue 解析临时分配 ~50MB");
            log::info!("        Rust 堆分配器缓存 ~剩余");
        }
        log::info!("");
        log::info!("  明细: 像素={}KB 离屏={}KB 图片={}KB 字体={}KB",
            pixel_buf / 1024, offscreen / 1024, images / 1024, font_mem / 1024);
    }

    fn memory_usage(&self) -> usize {
        self.rt.pixel_buffer_bytes()
            + self.assets.offscreen_bytes()
            + self.assets.image_bytes()
    }
}
