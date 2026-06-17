use std::cell::RefCell;

use super::engine::SoftwareEngine;
use crate::base::{Rect, Size};
use crate::diag::Error;
use crate::graphics::{
    BlendMode, Color, DirtyRegion, FontHandle, GraphicsEngine, ImageHandle, Radius, RenderOutcome,
    TextLayoutOptions,
};
use crate::graphics::{GradientDirection, Transform};
use crate::ui::render_context::RenderContext;
use crate::ui::theme::Theme;
use crate::ui::widget::WidgetTree;

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
        self.saved_pixels.clear();
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
        } else {
            *self.rt.clip_rect_mut() = Rect::new(0.0, 0.0, tw as f32, th as f32);
        }
    }

    fn end_frame(&mut self, _dirty: &DirtyRegion) {
        *self.rt.clip_rect_mut() = self.pre_frame_clip;
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
        // 阴影需要溢出父级 clip（如 ScrollView 视口），但必须限制在
        // begin_frame 的脏区域内，避免像素叠加拖影。
        let saved_rect = self.rt.clip_rect;
        let saved_stack = std::mem::take(self.rt.clip_stack_mut());
        if let Some(&pre_parent) = saved_stack.first() {
            *self.rt.clip_rect_mut() = pre_parent;
        }
        self.rt
            .draw_box_shadow(rect, blur_radius, offset_x, offset_y, color, corner_radius);
        *self.rt.clip_rect_mut() = saved_rect;
        *self.rt.clip_stack_mut() = saved_stack;
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
        let saved_rect = self.rt.clip_rect;
        let saved_stack = std::mem::take(self.rt.clip_stack_mut());
        if let Some(&pre_parent) = saved_stack.first() {
            *self.rt.clip_rect_mut() = pre_parent;
        }
        self.rt.draw_box_shadow_ambient(
            rect,
            blur_radius,
            offset_x,
            offset_y,
            color,
            corner_radius,
        );
        *self.rt.clip_rect_mut() = saved_rect;
        *self.rt.clip_stack_mut() = saved_stack;
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

    fn load_image(&mut self, data: &[u8]) -> Result<&mut ImageHandle, Error> {
        let img = image::load_from_memory(data).map_err(|e| {
            Error::new(
                crate::diag::Errc::FormatError,
                format!("cannot decode image: {}", e),
            )
        })?;
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

    // ── 离屏渲染 ──

    fn create_offscreen(&mut self, w: i32, h: i32) -> Result<&mut ImageHandle, Error> {
        self.assets.create_offscreen(w, h)
    }

    fn destroy_offscreen(&mut self, _offscreen: &ImageHandle) {}

    fn begin_offscreen(&mut self, offscreen: &ImageHandle) {
        if let Some(idx) = self.assets.offscreen_slot_index(offscreen) {
            self.saved_pixels = self.rt.take_pixels();
            let (pixels, w, h) = self.assets.take_offscreen_pixels(idx);
            self.rt.set_pixels(pixels, w, h);
            self.active_target = super::engine::ActiveTarget::Offscreen(idx);
        }
    }

    fn end_offscreen(&mut self) {
        if let super::engine::ActiveTarget::Offscreen(idx) = self.active_target {
            let pixels = self.rt.take_pixels();
            self.assets.return_offscreen_pixels(idx, pixels);
            self.rt.set_pixels(
                std::mem::take(&mut self.saved_pixels),
                self.main_width,
                self.main_height,
            );
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

    fn render_frame(
        &mut self,
        tree: &mut WidgetTree,
        theme: &RefCell<Theme>,
        first_frame: bool,
        keep_polling: bool,
    ) -> RenderOutcome {
        // ── 脏状态判断 ────────────────────────────────────────────
        let dirty = tree.dirty_region();
        let need_render = first_frame
            || !self.rendered_first
            || dirty.full_frame
            || dirty.clear_required
            || keep_polling;

        if !need_render {
            return RenderOutcome::Idle;
        }

        // ── 脏区域 → clip + 平台损伤矩形 ──────────────────────
        let region = if !self.rendered_first || tree.dirty_region().full_frame {
            DirtyRegion::full()
        } else {
            tree.dirty_region().clone()
        };

        let scroll_deltas = tree.drain_scroll_deltas();

        let damage: Option<(i32, i32, i32, i32)> = if region.full_frame || !scroll_deltas.is_empty()
        {
            None
        } else {
            let bounds = region.bounds();
            Some((
                bounds.x as i32,
                bounds.y as i32,
                bounds.w as i32,
                bounds.h as i32,
            ))
        };

        // ── 滚动偏移（先于清理，避免滚动携带清除后的透明像素） ──
        for &(viewport, dx, dy) in &scroll_deltas {
            GraphicsEngine::scroll_region(self, viewport, dx, dy);
        }

        // ── Pass 1: Clear + Geometry ───────────────────────────
        GraphicsEngine::begin_frame(self, &region);
        tree.layout();

        // LayerTree 构建与更新
        if self.layer_tree_stale {
            self.layer_tree.build(tree);
            self.layer_tree_stale = false;
        }
        self.layer_tree.update_dirty(tree);

        // SAFETY: font_service 字段与 engine 使用的字段（rt/assets）物理分离。
        // 先取 font_service 的裸指针，再创建 engine 引用，避免 Rust 的借用检查器
        // 将 &mut self（作为 engine）视为占用了所有字段。
        let fs_ptr = &self.font_service as *const crate::graphics::font_service::FontService;
        let theme_ref = theme.borrow();
        let tokens = theme_ref.tokens();
        let mut rctx = RenderContext::new(self, FontHandle::default(), unsafe { &*fs_ptr }, tokens);
        tree.render_geometry(&mut rctx);
        drop(rctx);
        drop(theme_ref);

        // LayerTree 渲染：委托给本引擎独立方法避免借用冲突
        let lt_tokens;
        let lt_ref = theme.borrow();
        lt_tokens = lt_ref.tokens();
        self.paint_layer_tree(tree, lt_tokens);
        drop(lt_ref);

        GraphicsEngine::end_frame(self, &region);

        // ── Pass 2: Overlay ─────────────────────────────────────
        let overlay_region = DirtyRegion::empty();
        GraphicsEngine::begin_frame(self, &overlay_region);
        let theme_ref = theme.borrow();
        let tokens = theme_ref.tokens();
        let fs_ptr = &self.font_service as *const crate::graphics::font_service::FontService;
        let mut rctx = RenderContext::new(self, FontHandle::default(), unsafe { &*fs_ptr }, tokens);
        tree.render_overlays(&mut rctx);
        drop(rctx);
        drop(theme_ref);
        GraphicsEngine::end_frame(self, &region);

        // ── 完成：reset dirty ───────────────────────────────────
        tree.reset_dirty();
        self.rendered_first = true;

        RenderOutcome::Present(damage)
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
        for row in 0..height {
            let sy = y + row as i32;
            for col in 0..width {
                let cov = coverage[row * width + col];
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
                self.rt.put_pixel_raw(x + col as i32, sy, pixel);
            }
        }
    }
}
