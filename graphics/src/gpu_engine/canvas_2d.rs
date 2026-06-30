//! GpuCanvas2D — GPU 加速的 Canvas2D 实现，未实现的方法用软件回退。

use glow::HasContext as _;
use uix_platform::Rect;

use crate::color::Color;
use crate::engine::cpu::canvas_2d::CpuCanvas2D;
use crate::engine::cpu::pixel_surface::PixelSurface;
use crate::path::{FillRule, Path};
use crate::rasterizer::core as rast;
use crate::stroker::StrokeOptions;
use crate::traits::Canvas2D;
use crate::types::{BlendMode, GradientDirection, Radius, Transform};

/// GPU 2D 绘制上下文。
pub struct GpuCanvas2D {
    gl_ptr: *const glow::Context,

    // ── 渲染状态 ──
    clip_rect: Rect,
    clip_stack: Vec<Rect>,
    opacity: f32,
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

    // ── 软件回退（未实现 GPU 路径的方法走 CPU 渲染 + 上传）──
    soft_fallback: CpuCanvas2D,
    fallback_texture: glow::Texture,
    #[allow(dead_code)]
    tex_program: glow::Program,
    #[allow(dead_code)]
    u_tex_viewport_loc: Option<glow::UniformLocation>,
    #[allow(dead_code)]
    u_tex_sampler_loc: Option<glow::UniformLocation>,

    // ── 表面尺寸 ──
    surface_w: i32,
    surface_h: i32,
}

#[derive(Clone)]
struct StateSnapshot {
    clip_rect: Rect,
    opacity: f32,
    transform: Transform,
    blend_mode: BlendMode,
}

const TEX_VERT: &str = r#"#version 300 es
precision highp float;
in vec2 a_pos;
in vec2 a_uv;
out vec2 v_uv;
void main() {
    gl_Position = vec4(a_pos, 0.0, 1.0);
    v_uv = a_uv;
}
"#;

const TEX_FRAG: &str = r#"#version 300 es
precision highp float;
in vec2 v_uv;
uniform sampler2D u_tex;
uniform float u_opacity;
out vec4 fragColor;
void main() {
    vec4 c = texture(u_tex, v_uv);
    fragColor = vec4(c.rgb, c.a * u_opacity);
}
"#;

impl GpuCanvas2D {
    fn gl(&self) -> &glow::Context {
        unsafe { &*self.gl_ptr }
    }

    pub fn new(gl: &glow::Context, width: i32, height: i32) -> Self {
        let rect_program = unsafe { Self::compile_rect_shader(gl) };
        let (rect_vao, rect_vbo) = unsafe { Self::create_rect_geom(gl) };

        let u_viewport_loc = unsafe { gl.get_uniform_location(rect_program, "u_viewport") };
        let u_rect_loc = unsafe { gl.get_uniform_location(rect_program, "u_rect") };
        let u_color_loc = unsafe { gl.get_uniform_location(rect_program, "u_color") };
        let u_radius_loc = unsafe { gl.get_uniform_location(rect_program, "u_radius") };

        let (tex_program, u_tex_viewport_loc, u_tex_sampler_loc) = unsafe { Self::compile_tex_shader(gl) };
        let fallback_texture = unsafe { Self::create_fallback_texture(gl, width, height) };

        Self {
            gl_ptr: gl as *const glow::Context,
            clip_rect: Rect::new(0.0, 0.0, width as f32, height as f32),
            clip_stack: Vec::new(),
            opacity: 1.0,
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
            soft_fallback: CpuCanvas2D::new(PixelSurface::new(width.max(1), height.max(1))),
            fallback_texture,
            tex_program,
            u_tex_viewport_loc,
            u_tex_sampler_loc,
            surface_w: width,
            surface_h: height,
        }
    }

    #[allow(clippy::unwrap_used)]
    unsafe fn compile_rect_shader(gl: &glow::Context) -> glow::Program {
        let vs = gl.create_shader(glow::VERTEX_SHADER).unwrap();
        gl.shader_source(vs, crate::gpu_engine::RECT_VERT);
        gl.compile_shader(vs);
        let fs = gl.create_shader(glow::FRAGMENT_SHADER).unwrap();
        gl.shader_source(fs, crate::gpu_engine::RECT_FRAG);
        gl.compile_shader(fs);
        let program = gl.create_program().unwrap();
        gl.attach_shader(program, vs);
        gl.attach_shader(program, fs);
        gl.link_program(program);
        gl.delete_shader(vs);
        gl.delete_shader(fs);
        program
    }

    #[allow(clippy::unwrap_used)]
    unsafe fn create_rect_geom(gl: &glow::Context) -> (glow::VertexArray, glow::Buffer) {
        let vao = gl.create_vertex_array().unwrap();
        let vbo = gl.create_buffer().unwrap();
        let vertices: [f32; 12] = [
            0.0, 0.0,  1.0, 0.0,  0.0, 1.0,
            0.0, 1.0,  1.0, 0.0,  1.0, 1.0,
        ];
        gl.bind_vertex_array(Some(vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        let data_bytes = std::slice::from_raw_parts(
            vertices.as_ptr() as *const u8,
            vertices.len() * 4,
        );
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, data_bytes, glow::STATIC_DRAW);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 8, 0);
        gl.enable_vertex_attrib_array(0);
        gl.bind_vertex_array(None);
        (vao, vbo)
    }

    #[allow(clippy::unwrap_used)]
    unsafe fn create_fallback_texture(gl: &glow::Context, w: i32, h: i32) -> glow::Texture {
        let tex = gl.create_texture().unwrap();
        gl.bind_texture(glow::TEXTURE_2D, Some(tex));
        gl.tex_image_2d(
            glow::TEXTURE_2D, 0,
            glow::RGBA as i32,
            w.max(1), h.max(1),
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(None),
        );
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::NEAREST as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
        gl.bind_texture(glow::TEXTURE_2D, None);
        tex
    }

    #[allow(clippy::unwrap_used)]
    unsafe fn compile_tex_shader(gl: &glow::Context) -> (glow::Program, Option<glow::UniformLocation>, Option<glow::UniformLocation>) {
        let vs = gl.create_shader(glow::VERTEX_SHADER).unwrap();
        gl.shader_source(vs, TEX_VERT);
        gl.compile_shader(vs);
        let fs = gl.create_shader(glow::FRAGMENT_SHADER).unwrap();
        gl.shader_source(fs, TEX_FRAG);
        gl.compile_shader(fs);
        let program = gl.create_program().unwrap();
        gl.attach_shader(program, vs);
        gl.attach_shader(program, fs);
        gl.link_program(program);
        gl.delete_shader(vs);
        gl.delete_shader(fs);
        let u_vp = gl.get_uniform_location(program, "u_viewport");
        let u_tex = gl.get_uniform_location(program, "u_tex");
        (program, u_vp, u_tex)
    }

    /// 上传软件回退像素到 GL 纹理并绘制全屏（在 end_frame 被调用）。
    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn flush_fallback(&mut self) {
        let (ptr, len) = {
            let p = self.soft_fallback.pixels_mut();
            (p.as_ptr() as *const u8, p.len())
        };
        self.gl().bind_texture(glow::TEXTURE_2D, Some(self.fallback_texture));
        self.gl().tex_sub_image_2d(
            glow::TEXTURE_2D, 0,
            0, 0,
            self.surface_w.max(1), self.surface_h.max(1),
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(Some(std::slice::from_raw_parts(ptr, len * 4))),
        );
    }

    pub fn resize(&mut self, width: i32, height: i32) {
        self.surface_w = width;
        self.surface_h = height;
        self.clip_rect = Rect::new(0.0, 0.0, width as f32, height as f32);
        self.soft_fallback = CpuCanvas2D::new(PixelSurface::new(width.max(1), height.max(1)));
        unsafe {
            self.gl().delete_texture(self.fallback_texture);
            self.fallback_texture = Self::create_fallback_texture(self.gl(), width, height);
        }
    }

    fn clip_int(&self) -> (i32, i32, i32, i32) {
        rast::clip_to_int(&self.clip_rect)
    }

    unsafe fn draw_rect_gpu(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
        let (cx0, cy0, cx1, cy1) = self.clip_int();
        if rect.x + rect.w <= cx0 as f32 || rect.y + rect.h <= cy0 as f32
            || rect.x >= cx1 as f32 || rect.y >= cy1 as f32
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
        self.gl().uniform_2_f32(self.u_viewport_loc.as_ref(), self.surface_w as f32, self.surface_h as f32);
        self.gl().uniform_4_f32(self.u_rect_loc.as_ref(), rect.x, rect.y, rect.w, rect.h);
        self.gl().uniform_4_f32(self.u_color_loc.as_ref(), cf[0], cf[1], cf[2], cf[3] * self.opacity);
        self.gl().uniform_4_f32(self.u_radius_loc.as_ref(), r[0], r[1], r[2], r[3]);
        self.gl().bind_vertex_array(Some(self.rect_vao));
        self.gl().draw_arrays(glow::TRIANGLES, 0, 6);
        self.gl().bind_vertex_array(None);
    }

    fn sync_fallback_state(&mut self) {
        self.soft_fallback.set_transform(self.transform);
        self.soft_fallback.set_opacity(self.opacity);
        self.soft_fallback.set_blend_mode(self.blend_mode);
    }
}

// ── Drop 实现：释放 GL 资源 ──

impl Drop for GpuCanvas2D {
    fn drop(&mut self) {
        unsafe {
            self.gl().delete_vertex_array(self.rect_vao);
            self.gl().delete_buffer(self.rect_vbo);
            self.gl().delete_program(self.rect_program);
            self.gl().delete_program(self.tex_program);
            self.gl().delete_texture(self.fallback_texture);
        }
    }
}

impl Canvas2D for GpuCanvas2D {
    fn fill_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
        unsafe { self.draw_rect_gpu(rect, color, radius); }
    }

    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        let rect = Rect::new(cx - r, cy - r, r * 2.0, r * 2.0);
        unsafe { self.draw_rect_gpu(rect, color, Some(Radius::uniform(r))); }
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
        self.soft_fallback.fill_radial_gradient(cx, cy, ir, or, ic, oc);
        self.soft_fallback.pop_clip();
    }

    fn draw_box_shadow(&mut self, rect: Rect, blur: f32, ox: f32, oy: f32, color: Color, rad: Option<Radius>) {
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback.draw_box_shadow(rect, blur, ox, oy, color, rad);
        self.soft_fallback.pop_clip();
    }

    fn draw_box_shadow_ambient(&mut self, rect: Rect, blur: f32, ox: f32, oy: f32, color: Color, rad: Option<Radius>) {
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback.draw_box_shadow_ambient(rect, blur, ox, oy, color, rad);
        self.soft_fallback.pop_clip();
    }

    fn blit_image(&mut self, src: &[u32], src_w: i32, src_rect: Rect, dst_rect: Rect) {
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback.blit_image(src, src_w, src_rect, dst_rect);
        self.soft_fallback.pop_clip();
    }

    fn blit_glyph(&mut self, x: i32, y: i32, coverage: &[u8], w: usize, h: usize, color: Color) {
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback.blit_glyph(x, y, coverage, w, h, color);
        self.soft_fallback.pop_clip();
    }

    fn push_clip_path(&mut self, _path: &Path) {
        // TODO: 路径裁剪尚未实现。
        // CpuCanvas2D 中的实现也是空操作，Gpu 路径不受影响。
        // 未来实现时需：
        //   1. push clip_stack 记录当前 clip_rect
        //   2. 用 path 的 bounding box 更新 clip_rect
        //   3. 同步 GL scissor test
    }

    // ── 渲染状态栈 ──

    fn save(&mut self) {
        self.state_stack.push(StateSnapshot {
            clip_rect: self.clip_rect,
            opacity: self.opacity,
            transform: self.transform,
            blend_mode: self.blend_mode,
        });
    }

    fn restore(&mut self) {
        if let Some(state) = self.state_stack.pop() {
            self.clip_rect = state.clip_rect;
            self.opacity = state.opacity;
            self.transform = state.transform;
            self.blend_mode = state.blend_mode;
        }
    }

    fn push_clip(&mut self, rect: Rect) {
        self.clip_stack.push(self.clip_rect);
        if let Some(intersection) = self.clip_rect.intersect(&rect) {
            self.clip_rect = intersection;
            unsafe {
                self.gl().enable(glow::SCISSOR_TEST);
                self.gl().scissor(
                    self.clip_rect.x as i32,
                    (self.surface_h as f32 - self.clip_rect.y - self.clip_rect.h) as i32,
                    self.clip_rect.w as i32,
                    self.clip_rect.h as i32,
                );
            }
        } else {
            self.clip_rect = Rect::zero();
            unsafe { self.gl().scissor(0, 0, 0, 0); }
        }
    }

    fn pop_clip(&mut self) {
        if let Some(prev) = self.clip_stack.pop() {
            self.clip_rect = prev;
            unsafe {
                self.gl().scissor(
                    self.clip_rect.x as i32,
                    (self.surface_h as f32 - self.clip_rect.y - self.clip_rect.h) as i32,
                    self.clip_rect.w as i32,
                    self.clip_rect.h as i32,
                );
            }
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
                    self.gl().blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
                }
                BlendMode::Additive => {
                    self.gl().enable(glow::BLEND);
                    self.gl().blend_func(glow::SRC_ALPHA, glow::ONE);
                }
            }
        }
    }

    fn pixels_mut(&mut self) -> &mut [u32] {
        self.soft_fallback.pixels_mut()
    }

    fn surface_size(&self) -> uix_platform::Size {
        uix_platform::Size::new(self.surface_w as f32, self.surface_h as f32)
    }

    fn current_clip(&self) -> Rect {
        self.clip_rect
    }
}
