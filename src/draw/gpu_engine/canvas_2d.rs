//! GpuCanvas2D — GPU 加速的 Canvas2D 实现，未实现的方法走 SharedRasterizer。

use crate::core::{Errc, Error, Rect};
use glow::HasContext as _;

use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::primitives::color::Color;
use crate::draw::primitives::path::{FillRule, Path};
use crate::draw::primitives::stroker::StrokeOptions;
use crate::draw::primitives::types::{BlendMode, GradientDirection, Radius, Transform};
use crate::draw::rasterizer::core as rast;
use crate::draw::traits::Canvas2D;

/// GPU 2D 绘制上下文。
pub struct GpuCanvas2D {
    gl_ptr: *const glow::Context,
    resources_released: bool,

    // ── 渲染状态 ──
    clip_rect: Rect,
    clip_stack: Vec<Rect>,
    opacity: f32,
    offset_x: f32,
    offset_y: f32,
    transform: Transform,
    blend_mode: BlendMode,
    state_stack: Vec<StateSnapshot>,

    // ── GPU 管线资源 ──
    rect_vao: glow::VertexArray,
    rect_vbo: glow::Buffer,
    rect_program: glow::Program,
    u_viewport_loc: Option<glow::UniformLocation>,
    u_rect_loc: Option<glow::UniformLocation>,
    u_color_loc: Option<glow::UniformLocation>,
    u_radius_loc: Option<glow::UniformLocation>,

    // ── CPU 回退合成 ──
    blit_vao: glow::VertexArray,
    blit_vbo: glow::Buffer,
    blit_program: glow::Program,
    /// FBO / native texture blit（无 BGRA swizzle）。
    blit_rgba_program: glow::Program,

    // ── 未实现 GPU 路径 → SharedRasterizer CPU 光栅化 ──
    soft_fallback: SharedRasterizer,
    fallback_texture: glow::Texture,
    /// The CPU fallback is an ordered segment, not a persistent overlay.
    soft_dirty: bool,
    /// Additive needs the destination framebuffer. A transparent CPU segment
    /// cannot reproduce it, so keep a typed failure pending instead of alpha-
    /// compositing an approximation at a later boundary.
    soft_uses_destination_blend: bool,

    // ── 表面尺寸（dip / 逻辑坐标）──
    surface_w: i32,
    surface_h: i32,
    /// 逻辑像素 → 帧缓冲像素（HiDPI viewport 缩放）。
    device_pixel_ratio: f32,
    u_tex_loc: Option<glow::UniformLocation>,
    u_tex_uv_rect_loc: Option<glow::UniformLocation>,
    u_tex_rgba_loc: Option<glow::UniformLocation>,
    u_tex_rgba_rect_loc: Option<glow::UniformLocation>,
    #[cfg(test)]
    last_soft_upload_bytes: usize,
}

#[derive(Clone)]
struct StateSnapshot {
    clip_rect: Rect,
    opacity: f32,
    offset_x: f32,
    offset_y: f32,
    transform: Transform,
    blend_mode: BlendMode,
}

impl GpuCanvas2D {
    fn gl(&self) -> &glow::Context {
        unsafe { &*self.gl_ptr }
    }

    /// 将 glow 字符串错误转换为统一 Error。
    fn glow_err(msg: impl Into<String>) -> Error {
        Error::new(Errc::PlatformError, msg)
    }

    fn scissor_for_clip(clip: Rect, surface_h: i32, dpr: f32) -> (i32, i32, i32, i32) {
        if clip.w <= 0.0 || clip.h <= 0.0 {
            return (0, 0, 0, 0);
        }
        let dpr = dpr.max(1.0);
        let x = (clip.x * dpr).floor() as i32;
        let w = (clip.w * dpr).ceil() as i32;
        let h = (clip.h * dpr).ceil() as i32;
        let y = (clip.y * dpr).floor() as i32;
        let fb_h = (surface_h as f32 * dpr).ceil() as i32;
        let sy = (fb_h - y - h).max(0);
        (x, sy, w, h)
    }

    /// Returns the tight logical-pixel bounding box of visible CPU fallback
    /// pixels. The soft buffer is cleared after every ordered commit, so its
    /// transparent area cannot affect the current framebuffer and must not be
    /// transferred again.
    fn soft_upload_bounds(pixels: &[u32], width: i32, height: i32) -> Option<(i32, i32, i32, i32)> {
        if width <= 0 || height <= 0 {
            return None;
        }
        let width = width as usize;
        let expected = width.saturating_mul(height as usize);
        let count = pixels.len().min(expected);
        let mut min_x = width;
        let mut min_y = height as usize;
        let mut max_x = 0usize;
        let mut max_y = 0usize;
        let mut any = false;

        for (index, pixel) in pixels.iter().take(count).enumerate() {
            if (pixel >> 24) == 0 {
                continue;
            }
            let x = index % width;
            let y = index / width;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            any = true;
        }

        any.then(|| {
            (
                min_x as i32,
                min_y as i32,
                (max_x - min_x + 1) as i32,
                (max_y - min_y + 1) as i32,
            )
        })
    }

    fn apply_clip_scissor(&self) {
        let (x, y, w, h) =
            Self::scissor_for_clip(self.clip_rect, self.surface_h, self.device_pixel_ratio);
        unsafe {
            self.gl().enable(glow::SCISSOR_TEST);
            self.gl().scissor(x, y, w, h);
        }
    }

    /// 编译单个着色器，失败时返回包含 info log 的错误。
    unsafe fn compile_shader(
        gl: &glow::Context,
        stage: u32,
        source: &str,
        label: &str,
    ) -> Result<glow::Shader, Error> {
        let shader = gl
            .create_shader(stage)
            .map_err(|e| Self::glow_err(format!("{label}: create_shader: {e}")))?;
        gl.shader_source(shader, source);
        gl.compile_shader(shader);
        if !gl.get_shader_compile_status(shader) {
            let log = gl.get_shader_info_log(shader);
            gl.delete_shader(shader);
            return Err(Self::glow_err(format!("{label} 编译失败: {log}")));
        }
        Ok(shader)
    }

    /// 链接 program，失败时返回包含 info log 的错误。
    unsafe fn link_program(
        gl: &glow::Context,
        vs: glow::Shader,
        fs: glow::Shader,
        label: &str,
    ) -> Result<glow::Program, Error> {
        let program = gl
            .create_program()
            .map_err(|e| Self::glow_err(format!("{label}: create_program: {e}")))?;
        gl.attach_shader(program, vs);
        gl.attach_shader(program, fs);
        gl.link_program(program);
        gl.delete_shader(vs);
        gl.delete_shader(fs);
        if !gl.get_program_link_status(program) {
            let log = gl.get_program_info_log(program);
            gl.delete_program(program);
            return Err(Self::glow_err(format!("{label} 链接失败: {log}")));
        }
        Ok(program)
    }

    unsafe fn compile_rect_shader(gl: &glow::Context) -> Result<glow::Program, Error> {
        let vs = Self::compile_shader(
            gl,
            glow::VERTEX_SHADER,
            crate::native::graphics::opengl::shaders::RECT_VERT,
            "RectVS",
        )?;
        let fs = Self::compile_shader(
            gl,
            glow::FRAGMENT_SHADER,
            crate::native::graphics::opengl::shaders::RECT_FRAG,
            "RectFS",
        )?;
        Self::link_program(gl, vs, fs, "RectProgram")
    }

    unsafe fn compile_blit_shader(gl: &glow::Context) -> Result<glow::Program, Error> {
        let vs = Self::compile_shader(
            gl,
            glow::VERTEX_SHADER,
            crate::native::graphics::opengl::shaders::FULLSCREEN_VERT,
            "BlitVS",
        )?;
        let fs = Self::compile_shader(
            gl,
            glow::FRAGMENT_SHADER,
            crate::native::graphics::opengl::shaders::BLIT_FRAG,
            "BlitFS",
        )?;
        Self::link_program(gl, vs, fs, "BlitProgram")
    }

    unsafe fn compile_blit_rgba_shader(gl: &glow::Context) -> Result<glow::Program, Error> {
        let vs = Self::compile_shader(
            gl,
            glow::VERTEX_SHADER,
            crate::native::graphics::opengl::shaders::FULLSCREEN_VERT,
            "BlitRgbaVS",
        )?;
        let fs = Self::compile_shader(
            gl,
            glow::FRAGMENT_SHADER,
            crate::native::graphics::opengl::shaders::BLIT_RGBA_FRAG,
            "BlitRgbaFS",
        )?;
        Self::link_program(gl, vs, fs, "BlitRgbaProgram")
    }

    unsafe fn create_fullscreen_quad(
        gl: &glow::Context,
    ) -> Result<(glow::VertexArray, glow::Buffer), Error> {
        let vao = gl
            .create_vertex_array()
            .map_err(|e| Self::glow_err(format!("create_vertex_array: {e}")))?;
        let vbo = gl
            .create_buffer()
            .map_err(|e| Self::glow_err(format!("create_buffer: {e}")))?;
        let vertices: [f32; 8] = [-1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0];
        gl.bind_vertex_array(Some(vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        let data_bytes =
            std::slice::from_raw_parts(vertices.as_ptr() as *const u8, vertices.len() * 4);
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, data_bytes, glow::STATIC_DRAW);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 8, 0);
        gl.enable_vertex_attrib_array(0);
        gl.bind_vertex_array(None);
        Ok((vao, vbo))
    }

    unsafe fn create_rect_geom(
        gl: &glow::Context,
    ) -> Result<(glow::VertexArray, glow::Buffer), Error> {
        let vao = gl
            .create_vertex_array()
            .map_err(|e| Self::glow_err(format!("create_vertex_array: {e}")))?;
        let vbo = gl
            .create_buffer()
            .map_err(|e| Self::glow_err(format!("create_buffer: {e}")))?;
        let vertices: [f32; 12] = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 1.0];
        gl.bind_vertex_array(Some(vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        let data_bytes =
            std::slice::from_raw_parts(vertices.as_ptr() as *const u8, vertices.len() * 4);
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, data_bytes, glow::STATIC_DRAW);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 8, 0);
        gl.enable_vertex_attrib_array(0);
        gl.bind_vertex_array(None);
        Ok((vao, vbo))
    }

    unsafe fn create_fallback_texture(
        gl: &glow::Context,
        w: i32,
        h: i32,
    ) -> Result<glow::Texture, Error> {
        let tex = gl
            .create_texture()
            .map_err(|e| Self::glow_err(format!("create_texture: {e}")))?;
        gl.bind_texture(glow::TEXTURE_2D, Some(tex));
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA as i32,
            w.max(1),
            h.max(1),
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(None),
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::NEAREST as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::NEAREST as i32,
        );
        gl.bind_texture(glow::TEXTURE_2D, None);
        Ok(tex)
    }

    pub fn new(gl: &glow::Context, width: i32, height: i32) -> Result<Self, Error> {
        let rect_program = unsafe { Self::compile_rect_shader(gl)? };
        let (rect_vao, rect_vbo) = unsafe { Self::create_rect_geom(gl)? };
        let blit_program = unsafe { Self::compile_blit_shader(gl)? };
        let blit_rgba_program = unsafe { Self::compile_blit_rgba_shader(gl)? };
        let (blit_vao, blit_vbo) = unsafe { Self::create_fullscreen_quad(gl)? };

        let u_viewport_loc = unsafe { gl.get_uniform_location(rect_program, "u_viewport") };
        let u_rect_loc = unsafe { gl.get_uniform_location(rect_program, "u_rect") };
        let u_color_loc = unsafe { gl.get_uniform_location(rect_program, "u_color") };
        let u_radius_loc = unsafe { gl.get_uniform_location(rect_program, "u_radius") };

        let fallback_texture = unsafe { Self::create_fallback_texture(gl, width, height)? };
        let u_tex_loc = unsafe { gl.get_uniform_location(blit_program, "u_tex") };
        let u_tex_uv_rect_loc = unsafe { gl.get_uniform_location(blit_program, "u_uv_rect") };
        let u_tex_rgba_loc = unsafe { gl.get_uniform_location(blit_rgba_program, "u_tex") };
        let u_tex_rgba_rect_loc =
            unsafe { gl.get_uniform_location(blit_rgba_program, "u_uv_rect") };

        Ok(Self {
            gl_ptr: gl as *const glow::Context,
            resources_released: false,
            clip_rect: Rect::new(0.0, 0.0, width as f32, height as f32),
            clip_stack: Vec::new(),
            opacity: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
            transform: Transform::identity(),
            blend_mode: BlendMode::default(),
            state_stack: Vec::new(),
            rect_vao,
            rect_vbo,
            rect_program,
            u_viewport_loc,
            u_rect_loc,
            u_color_loc,
            u_radius_loc,
            blit_vao,
            blit_vbo,
            blit_program,
            blit_rgba_program,
            soft_fallback: SharedRasterizer::new(PixelSurface::new(width.max(1), height.max(1))),
            fallback_texture,
            soft_dirty: false,
            soft_uses_destination_blend: false,
            surface_w: width,
            surface_h: height,
            device_pixel_ratio: 1.0,
            u_tex_loc,
            u_tex_uv_rect_loc,
            u_tex_rgba_loc,
            u_tex_rgba_rect_loc,
            #[cfg(test)]
            last_soft_upload_bytes: 0,
        })
    }

    /// 在 GL context 仍然有效且 current 时释放本画布持有的 GPU 资源。
    pub(crate) fn release_gpu_resources(&mut self) {
        if self.resources_released {
            return;
        }
        self.resources_released = true;
        unsafe {
            self.gl().delete_vertex_array(self.rect_vao);
            self.gl().delete_buffer(self.rect_vbo);
            self.gl().delete_program(self.rect_program);
            self.gl().delete_vertex_array(self.blit_vao);
            self.gl().delete_buffer(self.blit_vbo);
            self.gl().delete_program(self.blit_program);
            self.gl().delete_program(self.blit_rgba_program);
            self.gl().delete_texture(self.fallback_texture);
        }
    }

    pub(crate) fn set_device_pixel_ratio(&mut self, dpr: f32) {
        self.device_pixel_ratio = dpr.max(1.0);
        self.apply_clip_scissor();
    }

    /// 清除 CPU 回退缓冲（与 GL clear 同步）。
    pub(crate) fn clear_soft_fallback(&mut self) {
        self.soft_fallback.surface_mut().clear_all();
        self.soft_dirty = false;
        self.soft_uses_destination_blend = false;
    }

    #[cfg(test)]
    pub(crate) fn last_soft_upload_bytes(&self) -> usize {
        self.last_soft_upload_bytes
    }

    /// 将 CPU 回退像素合成到当前 GL framebuffer。
    pub(crate) fn flush_soft_fallback(&mut self) -> Result<(), Error> {
        if !self.soft_dirty {
            return Ok(());
        }
        if self.soft_uses_destination_blend {
            return Err(Error::new(
                Errc::NotImplemented,
                "GpuCanvas2D: CPU fallback cannot emulate destination-dependent Additive blend",
            ));
        }
        let pixels = self.soft_fallback.surface().pixels();
        if pixels.is_empty() || self.surface_w <= 0 || self.surface_h <= 0 {
            return Ok(());
        }
        let Some((x, y, width, height)) =
            Self::soft_upload_bounds(pixels, self.surface_w, self.surface_h)
        else {
            self.clear_soft_fallback();
            return Ok(());
        };
        let dpr = self.device_pixel_ratio.max(1.0);
        #[cfg(test)]
        {
            self.last_soft_upload_bytes = (width as usize)
                .saturating_mul(height as usize)
                .saturating_mul(4);
        }

        unsafe {
            self.gl().disable(glow::SCISSOR_TEST);
            self.gl().active_texture(glow::TEXTURE0);
            self.gl()
                .bind_texture(glow::TEXTURE_2D, Some(self.fallback_texture));
            // GLES has no portable unpack-row-length setting. Upload the
            // tight rectangle row-by-row so each source slice is contiguous.
            for row in y..(y + height) {
                let start = (row as usize)
                    .saturating_mul(self.surface_w as usize)
                    .saturating_add(x as usize);
                let end = start.saturating_add(width as usize).min(pixels.len());
                let row_bytes = std::slice::from_raw_parts(
                    pixels[start..end].as_ptr() as *const u8,
                    (end - start).saturating_mul(4),
                );
                self.gl().tex_sub_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    x,
                    row,
                    width,
                    1,
                    // CPU pixels are AARRGGBB u32 values. On the supported
                    // little-endian desktop targets their bytes are BGRA;
                    // upload through GLES-core RGBA and swap R/B in shader.
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(Some(row_bytes)),
                );
            }
            self.gl().enable(glow::BLEND);
            self.gl()
                .blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            self.gl().use_program(Some(self.blit_program));
            self.gl().uniform_1_i32(self.u_tex_loc.as_ref(), 0);
            self.gl().uniform_4_f32(
                self.u_tex_uv_rect_loc.as_ref(),
                x as f32 / self.surface_w as f32,
                y as f32 / self.surface_h as f32,
                width as f32 / self.surface_w as f32,
                height as f32 / self.surface_h as f32,
            );
            self.gl().viewport(
                (x as f32 * dpr).floor() as i32,
                ((self.surface_h - y - height) as f32 * dpr).floor() as i32,
                (width as f32 * dpr).ceil() as i32,
                (height as f32 * dpr).ceil() as i32,
            );
            self.gl().bind_vertex_array(Some(self.blit_vao));
            self.gl().draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
            self.gl().bind_vertex_array(None);
            self.gl().bind_texture(glow::TEXTURE_2D, None);
            self.gl().viewport(
                0,
                0,
                (self.surface_w as f32 * dpr).ceil() as i32,
                (self.surface_h as f32 * dpr).ceil() as i32,
            );
        }
        self.apply_clip_scissor();
        // This segment has been committed at the current painter-order
        // boundary. Keeping it would draw it again at the final present and
        // would move it across a later Picture/native operation.
        self.clear_soft_fallback();
        Ok(())
    }

    /// Sample an external GL texture into the current framebuffer covering `dst`
    /// (logical pixels). Uses a temporary viewport; restores clip scissor after.
    pub(crate) fn blit_external_texture(
        &self,
        texture: glow::Texture,
        src: Rect,
        source_width: i32,
        source_height: i32,
        dst: Rect,
    ) {
        if dst.w <= 0.0 || dst.h <= 0.0 || source_width <= 0 || source_height <= 0 {
            return;
        }
        let max_x = source_width as f32;
        let max_y = source_height as f32;
        let src_x = src.x.clamp(0.0, max_x);
        let src_y = src.y.clamp(0.0, max_y);
        let src_end_x = (src.x + src.w).clamp(src_x, max_x);
        let src_end_y = (src.y + src.h).clamp(src_y, max_y);
        if src_end_x <= src_x || src_end_y <= src_y {
            return;
        }
        let uv_rect = [
            src_x / max_x,
            src_y / max_y,
            (src_end_x - src_x) / max_x,
            (src_end_y - src_y) / max_y,
        ];
        let dpr = self.device_pixel_ratio.max(1.0);
        let vx = (dst.x * dpr).floor() as i32;
        let vy = ((self.surface_h as f32 - dst.y - dst.h).max(0.0) * dpr).floor() as i32;
        let vw = (dst.w * dpr).ceil().max(1.0) as i32;
        let vh = (dst.h * dpr).ceil().max(1.0) as i32;
        unsafe {
            self.gl().disable(glow::SCISSOR_TEST);
            self.gl().viewport(vx, vy, vw, vh);
            self.gl().active_texture(glow::TEXTURE0);
            self.gl().bind_texture(glow::TEXTURE_2D, Some(texture));
            self.gl().enable(glow::BLEND);
            self.gl()
                .blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            self.gl().use_program(Some(self.blit_rgba_program));
            self.gl().uniform_1_i32(self.u_tex_rgba_loc.as_ref(), 0);
            self.gl()
                .uniform_4_f32_slice(self.u_tex_rgba_rect_loc.as_ref(), &uv_rect);
            self.gl().bind_vertex_array(Some(self.blit_vao));
            self.gl().draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
            self.gl().bind_vertex_array(None);
            self.gl().bind_texture(glow::TEXTURE_2D, None);
            self.gl().viewport(
                0,
                0,
                (self.surface_w as f32 * dpr).ceil() as i32,
                (self.surface_h as f32 * dpr).ceil() as i32,
            );
        }
        // Caller restores scissor if needed; main path re-applies via surface.
    }

    /// 用 GL scissor 限定范围清屏，再恢复当前 clip scissor。
    pub(crate) fn clear_rect_raw(&mut self, x: i32, y: i32, w: i32, h: i32) {
        if w <= 0 || h <= 0 {
            return;
        }
        self.flush_soft_before_immediate();
        self.soft_fallback.surface_mut().clear_rect_raw(x, y, w, h);
        let dpr = self.device_pixel_ratio.max(1.0);
        let sx = (x as f32 * dpr).floor() as i32;
        let sy = ((self.surface_h - y - h).max(0) as f32 * dpr).floor() as i32;
        let sw = (w as f32 * dpr).ceil() as i32;
        let sh = (h as f32 * dpr).ceil() as i32;
        unsafe {
            self.gl().enable(glow::SCISSOR_TEST);
            self.gl().scissor(sx, sy, sw, sh);
            self.gl().clear_color(0.0, 0.0, 0.0, 0.0);
            self.gl().clear(glow::COLOR_BUFFER_BIT);
        }
        self.apply_clip_scissor();
    }

    pub fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.surface_w = width.max(1);
        self.surface_h = height.max(1);
        self.clip_rect = Rect::new(0.0, 0.0, width as f32, height as f32);
        self.clip_stack.clear();
        self.state_stack.clear();
        self.soft_fallback = SharedRasterizer::new(PixelSurface::new(width.max(1), height.max(1)));
        self.soft_dirty = false;
        self.soft_uses_destination_blend = false;
        let new_tex = unsafe { Self::create_fallback_texture(self.gl(), width, height)? };
        unsafe {
            self.gl().delete_texture(self.fallback_texture);
        }
        self.fallback_texture = new_tex;
        self.apply_clip_scissor();
        Ok(())
    }

    fn clip_int(&self) -> (i32, i32, i32, i32) {
        rast::clip_to_int(&self.clip_rect)
    }

    unsafe fn draw_rect_gpu(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
        let (cx0, cy0, cx1, cy1) = self.clip_int();
        if rect.x + rect.w <= cx0 as f32
            || rect.y + rect.h <= cy0 as f32
            || rect.x >= cx1 as f32
            || rect.y >= cy1 as f32
        {
            return;
        }
        let r = match radius {
            Some(r) => [r.tl, r.tr, r.br, r.bl],
            None => [0.0; 4],
        };
        let cf = [
            color.r as f32 / 255.0,
            color.g as f32 / 255.0,
            color.b as f32 / 255.0,
            color.a as f32 / 255.0,
        ];
        self.gl().use_program(Some(self.rect_program));
        self.gl().uniform_2_f32(
            self.u_viewport_loc.as_ref(),
            self.surface_w as f32,
            self.surface_h as f32,
        );
        self.gl()
            .uniform_4_f32(self.u_rect_loc.as_ref(), rect.x, rect.y, rect.w, rect.h);
        self.gl().uniform_4_f32(
            self.u_color_loc.as_ref(),
            cf[0],
            cf[1],
            cf[2],
            cf[3] * self.opacity,
        );
        self.gl()
            .uniform_4_f32(self.u_radius_loc.as_ref(), r[0], r[1], r[2], r[3]);
        self.gl().bind_vertex_array(Some(self.rect_vao));
        self.gl().draw_arrays(glow::TRIANGLES, 0, 6);
        self.gl().bind_vertex_array(None);
    }

    fn sync_fallback_state(&mut self) {
        self.soft_fallback.set_transform(self.transform);
        self.soft_fallback.set_opacity(self.opacity);
        self.soft_fallback.set_blend_mode(self.blend_mode);
        self.soft_fallback.set_offset(self.offset_x, self.offset_y);
        self.soft_dirty = true;
        self.soft_uses_destination_blend |= matches!(self.blend_mode, BlendMode::Additive);
    }

    /// Immediate GL operations form a command boundary.  Canvas2D cannot
    /// return `Result`, so an impossible CPU blend remains pending and will
    /// turn the enclosing present into a typed failure; successful segments
    /// are committed before the immediate operation.
    fn flush_soft_before_immediate(&mut self) {
        if let Err(error) = self.flush_soft_fallback() {
            crate::core::log::error_fn(format!(
                "GpuCanvas2D: ordered CPU segment cannot be committed: {}",
                error.short_what()
            ));
        }
    }
}

// ── Drop 实现：释放 GL 资源 ──

impl Drop for GpuCanvas2D {
    fn drop(&mut self) {
        self.release_gpu_resources();
    }
}

impl Canvas2D for GpuCanvas2D {
    fn offset(&self) -> (f32, f32) {
        (self.offset_x, self.offset_y)
    }

    fn set_offset(&mut self, dx: f32, dy: f32) {
        self.offset_x = dx;
        self.offset_y = dy;
    }

    fn fill_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
        self.flush_soft_before_immediate();
        let (ox, oy) = (self.offset_x, self.offset_y);
        let rect = if ox != 0.0 || oy != 0.0 {
            Rect::new(rect.x + ox, rect.y + oy, rect.w, rect.h)
        } else {
            rect
        };
        unsafe {
            self.draw_rect_gpu(rect, color, radius);
        }
    }

    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        self.flush_soft_before_immediate();
        let (ox, oy) = (self.offset_x, self.offset_y);
        let rect = Rect::new(cx - r + ox, cy - r + oy, r * 2.0, r * 2.0);
        unsafe {
            self.draw_rect_gpu(rect, color, Some(Radius::uniform(r)));
        }
    }

    // ── 未实现 GPU 路径 → 软件回退 ──

    fn fill_ellipse(&mut self, rect: Rect, color: Color) {
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback.fill_ellipse(rect, color);
        self.soft_fallback.pop_clip();
    }

    fn fill_sector(&mut self, cx: f32, cy: f32, r: f32, sa: f32, ea: f32, color: Color) {
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback.fill_sector(cx, cy, r, sa, ea, color);
        self.soft_fallback.pop_clip();
    }

    fn fill_path(&mut self, path: &Path, color: Color, fill_rule: FillRule) {
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback.fill_path(path, color, fill_rule);
        self.soft_fallback.pop_clip();
    }

    fn stroke_rect(&mut self, rect: Rect, color: Color, lw: f32, radius: Option<Radius>) {
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback.stroke_rect(rect, color, lw, radius);
        self.soft_fallback.pop_clip();
    }

    fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color, lw: f32) {
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback.stroke_circle(cx, cy, r, color, lw);
        self.soft_fallback.pop_clip();
    }

    fn stroke_path(&mut self, path: &Path, color: Color, opts: &StrokeOptions) {
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback.stroke_path(path, color, opts);
        self.soft_fallback.pop_clip();
    }

    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, w: f32) {
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback.draw_line(x1, y1, x2, y2, color, w);
        self.soft_fallback.pop_clip();
    }

    fn fill_linear_gradient(&mut self, rect: Rect, ca: Color, cb: Color, dir: GradientDirection) {
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback.fill_linear_gradient(rect, ca, cb, dir);
        self.soft_fallback.pop_clip();
    }

    fn fill_radial_gradient(&mut self, cx: f32, cy: f32, ir: f32, or: f32, ic: Color, oc: Color) {
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback
            .fill_radial_gradient(cx, cy, ir, or, ic, oc);
        self.soft_fallback.pop_clip();
    }

    fn draw_box_shadow(
        &mut self,
        rect: Rect,
        blur: f32,
        ox: f32,
        oy: f32,
        color: Color,
        rad: Option<Radius>,
    ) {
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback
            .draw_box_shadow(rect, blur, ox, oy, color, rad);
        self.soft_fallback.pop_clip();
    }

    fn draw_box_shadow_ambient(
        &mut self,
        rect: Rect,
        blur: f32,
        ox: f32,
        oy: f32,
        color: Color,
        rad: Option<Radius>,
    ) {
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback
            .draw_box_shadow_ambient(rect, blur, ox, oy, color, rad);
        self.soft_fallback.pop_clip();
    }

    fn blit_image(&mut self, src: &[u32], src_w: i32, src_rect: Rect, dst_rect: Rect) {
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback
            .blit_image(src, src_w, src_rect, dst_rect);
        self.soft_fallback.pop_clip();
    }

    fn blit_glyph(&mut self, x: i32, y: i32, coverage: &[u8], w: usize, h: usize, color: Color) {
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback.blit_glyph(x, y, coverage, w, h, color);
        self.soft_fallback.pop_clip();
    }

    fn push_clip_path(&mut self, _path: &Path) {
        // GPU 路径裁剪尚未支持；与 CpuCanvas2D 一致，当前为空操作。
        // 调用方应使用矩形裁剪（push_clip_rect）作为替代。
    }

    // ── 渲染状态栈 ──

    fn save(&mut self) {
        self.state_stack.push(StateSnapshot {
            clip_rect: self.clip_rect,
            opacity: self.opacity,
            offset_x: self.offset_x,
            offset_y: self.offset_y,
            transform: self.transform,
            blend_mode: self.blend_mode,
        });
    }

    fn restore(&mut self) {
        if let Some(state) = self.state_stack.pop() {
            self.clip_rect = state.clip_rect;
            self.opacity = state.opacity;
            self.offset_x = state.offset_x;
            self.offset_y = state.offset_y;
            self.transform = state.transform;
            self.blend_mode = state.blend_mode;
            self.apply_clip_scissor();
        }
    }

    fn push_clip(&mut self, rect: Rect) {
        let rect = Rect::new(
            rect.x + self.offset_x,
            rect.y + self.offset_y,
            rect.w,
            rect.h,
        );
        self.clip_stack.push(self.clip_rect);
        if let Some(intersection) = self.clip_rect.intersect(&rect) {
            self.clip_rect = intersection;
        } else {
            self.clip_rect = Rect::zero();
        }
        self.apply_clip_scissor();
    }

    fn pop_clip(&mut self) {
        if let Some(prev) = self.clip_stack.pop() {
            self.clip_rect = prev;
            self.apply_clip_scissor();
        }
    }

    fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.0, 1.0);
    }

    fn opacity(&self) -> f32 {
        self.opacity
    }

    fn set_blend_mode(&mut self, mode: BlendMode) {
        self.blend_mode = mode;
        unsafe {
            match mode {
                BlendMode::Alpha | BlendMode::SrcOver => {
                    self.gl().enable(glow::BLEND);
                    self.gl()
                        .blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
                }
                BlendMode::Additive => {
                    self.gl().enable(glow::BLEND);
                    self.gl().blend_func(glow::SRC_ALPHA, glow::ONE);
                }
            }
        }
    }

    fn pixels_mut(&mut self) -> &mut [u32] {
        self.soft_dirty = true;
        self.soft_uses_destination_blend |= matches!(self.blend_mode, BlendMode::Additive);
        self.soft_fallback.pixels_mut()
    }

    fn surface_size(&self) -> crate::core::Size {
        crate::core::Size::new(self.surface_w as f32, self.surface_h as f32)
    }

    fn current_clip(&self) -> Rect {
        self.clip_rect
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scissor_for_clip_flips_y_to_gl_coordinates() {
        assert_eq!(
            GpuCanvas2D::scissor_for_clip(Rect::new(10.0, 20.0, 30.0, 40.0), 100, 1.0),
            (10, 40, 30, 40)
        );
    }

    #[test]
    fn scissor_for_clip_scales_to_framebuffer_pixels_at_hidpi() {
        assert_eq!(
            GpuCanvas2D::scissor_for_clip(Rect::new(10.0, 20.0, 30.0, 40.0), 100, 2.0),
            (20, 80, 60, 80)
        );
    }

    #[test]
    fn scissor_for_empty_clip_disables_area() {
        assert_eq!(
            GpuCanvas2D::scissor_for_clip(Rect::new(0.0, 0.0, 0.0, 10.0), 100, 1.0),
            (0, 0, 0, 0)
        );
    }

    #[test]
    fn soft_upload_bounds_skips_transparent_pixels_and_is_tight() {
        let mut pixels = vec![0u32; 8 * 6];
        pixels[8 + 2] = 0xFF00_0001;
        pixels[3 * 8 + 5] = 0x8000_0002;
        // RGB without alpha is transparent and must not increase transfer.
        pixels[5 * 8 + 7] = 0x0000_00FF;

        assert_eq!(
            GpuCanvas2D::soft_upload_bounds(&pixels, 8, 6),
            Some((2, 1, 4, 3))
        );
        assert_eq!(GpuCanvas2D::soft_upload_bounds(&[0; 4], 2, 2), None);
    }
}
