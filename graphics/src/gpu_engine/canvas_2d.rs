//! GpuCanvas2D — GPU 加速的 Canvas2D 实现。
//!
//! 覆盖关键绘制方法（fill_rect / blit_image）用 OpenGL shader 加速，
//! 其他方法暂时覆盖为 log::warn + 空操作（避免调用默认实现触发 panic）。
//!
//! 渲染状态栈（save/restore/clip/opacity/blend/transform）用软件实现。

use glow::HasContext as _;
use uix_core::Rect;

use crate::color::Color;
use crate::path::{FillRule, Path};
use crate::stroker::StrokeOptions;
use crate::traits::Canvas2D;
use crate::types::{BlendMode, GradientDirection, Radius, Transform};

/// GPU 2D 绘制上下文。
pub struct GpuCanvas2D {
    /// 底层 GL 上下文的原始指针。
    /// GpuCanvas2D 总是由 GpuEngine 拥有，其生命周期由 GpuEngine 保证。
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

    // Uniform locations
    u_viewport_loc: Option<glow::UniformLocation>,
    u_rect_loc: Option<glow::UniformLocation>,
    u_color_loc: Option<glow::UniformLocation>,
    u_radius_loc: Option<glow::UniformLocation>,

    // ── 表面尺寸（用于 viewport uniform） ──
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

impl GpuCanvas2D {
    /// 获取底层 GL 上下文引用。
    fn gl(&self) -> &glow::Context {
        unsafe { &*self.gl_ptr }
    }

    /// 创建 GPU Canvas2D，编译着色器并初始化 OpenGL 管线。
    pub fn new(gl: &glow::Context, width: i32, height: i32) -> Self {
        let rect_program = unsafe { Self::compile_rect_shader(gl) };
        let (rect_vao, rect_vbo) = unsafe { Self::create_rect_geom(gl) };

        let u_viewport_loc = unsafe { gl.get_uniform_location(rect_program, "u_viewport") };
        let u_rect_loc = unsafe { gl.get_uniform_location(rect_program, "u_rect") };
        let u_color_loc = unsafe { gl.get_uniform_location(rect_program, "u_color") };
        let u_radius_loc = unsafe { gl.get_uniform_location(rect_program, "u_radius") };

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
            surface_w: width,
            surface_h: height,
        }
    }

    /// 编译矩形渲染着色器。
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

    /// 创建单位 quad 几何体（两个三角形，6 顶点）。
    unsafe fn create_rect_geom(gl: &glow::Context) -> (glow::VertexArray, glow::Buffer) {
        let vao = gl.create_vertex_array().unwrap();
        let vbo = gl.create_buffer().unwrap();

        // 单位 quad: (0,0) → (1,1)
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

        // a_pos: vec2
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 8, 0);
        gl.enable_vertex_attrib_array(0);

        gl.bind_vertex_array(None);
        (vao, vbo)
    }

    /// 设置表面尺寸（在 resize 时调用）。
    pub fn resize(&mut self, width: i32, height: i32) {
        self.surface_w = width;
        self.surface_h = height;
        self.clip_rect = Rect::new(0.0, 0.0, width as f32, height as f32);
    }

    /// 将 Clip 矩形转化为整数边界。
    fn clip_int(&self) -> (i32, i32, i32, i32) {
        (
            (self.clip_rect.x + 0.5).floor() as i32,
            (self.clip_rect.y + 0.5).floor() as i32,
            (self.clip_rect.x + self.clip_rect.w + 0.5).floor() as i32,
            (self.clip_rect.y + self.clip_rect.h + 0.5).floor() as i32,
        )
    }

    /// 在 GPU 上绘制一个填充矩形。
    unsafe fn draw_rect_gpu(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
        let (cx0, cy0, cx1, cy1) = self.clip_int();
        if rect.x + rect.w <= cx0 as f32 || rect.y + rect.h <= cy0 as f32
            || rect.x >= cx1 as f32 || rect.y >= cy1 as f32
        {
            return; // 完全在裁剪区域外
        }

        let r = match radius {
            Some(r) => [r.tl, r.tr, r.br, r.bl],
            None => [0.0; 4],
        };

        // Color → normalized float [0..1]
        let cf = [
            color.r as f32 / 255.0,
            color.g as f32 / 255.0,
            color.b as f32 / 255.0,
            color.a as f32 / 255.0,
        ];

        self.gl().use_program(Some(self.rect_program));

        // uniforms
        self.gl().uniform_2_f32(self.u_viewport_loc.as_ref(), self.surface_w as f32, self.surface_h as f32);
        self.gl().uniform_4_f32(self.u_rect_loc.as_ref(), rect.x, rect.y, rect.w, rect.h);
        self.gl().uniform_4_f32(self.u_color_loc.as_ref(), cf[0], cf[1], cf[2], cf[3] * self.opacity);
        self.gl().uniform_4_f32(self.u_radius_loc.as_ref(), r[0], r[1], r[2], r[3]);

        // 绘制 6 个顶点（2 三角形）
        self.gl().bind_vertex_array(Some(self.rect_vao));
        self.gl().draw_arrays(glow::TRIANGLES, 0, 6);
        self.gl().bind_vertex_array(None);
    }

    /// 预乘 RGBA 颜色（用于 SrcOver 混合）。
    fn premul(r: u8, g: u8, b: u8, a: u8) -> u32 {
        let a = a as u32;
        if a == 255 {
            return (a << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
        }
        let r = (r as u32 * a / 255).min(255);
        let g = (g as u32 * a / 255).min(255);
        let b = (b as u32 * a / 255).min(255);
        (a << 24) | (r << 16) | (g << 8) | b
    }
}

impl Canvas2D for GpuCanvas2D {
    // ═══════════════════════════════════════════
    // 矢量填充 — GPU 加速
    // ═══════════════════════════════════════════

    fn fill_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
        unsafe { self.draw_rect_gpu(rect, color, radius); }
    }

    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        // GPU 圆角矩形近似圆形
        let rect = Rect::new(cx - r, cy - r, r * 2.0, r * 2.0);
        unsafe { self.draw_rect_gpu(rect, color, Some(Radius::uniform(r))); }
    }

    fn fill_ellipse(&mut self, _rect: Rect, _color: Color) {
        log::warn!("[GpuCanvas2D] fill_ellipse 暂未实现");
    }

    fn fill_sector(&mut self, _cx: f32, _cy: f32, _r: f32, _sa: f32, _ea: f32, _color: Color) {
        log::warn!("[GpuCanvas2D] fill_sector 暂未实现");
    }

    fn fill_path(&mut self, _path: &Path, _color: Color, _fill_rule: FillRule) {
        log::warn!("[GpuCanvas2D] fill_path 暂未实现");
    }

    // ═══════════════════════════════════════════
    // 矢量描边 — 暂未实现
    // ═══════════════════════════════════════════

    fn stroke_rect(&mut self, _rect: Rect, _color: Color, _lw: f32, _radius: Option<Radius>) {
        log::warn!("[GpuCanvas2D] stroke_rect 暂未实现");
    }
    fn stroke_circle(&mut self, _cx: f32, _cy: f32, _r: f32, _color: Color, _lw: f32) {
        log::warn!("[GpuCanvas2D] stroke_circle 暂未实现");
    }
    fn stroke_path(&mut self, _path: &Path, _color: Color, _opts: &StrokeOptions) {
        log::warn!("[GpuCanvas2D] stroke_path 暂未实现");
    }
    fn draw_line(&mut self, _x1: f32, _y1: f32, _x2: f32, _y2: f32, _color: Color, _w: f32) {
        log::warn!("[GpuCanvas2D] draw_line 暂未实现");
    }

    // ═══════════════════════════════════════════
    // 渐变 — 暂未实现
    // ═══════════════════════════════════════════

    fn fill_linear_gradient(&mut self, _r: Rect, _ca: Color, _cb: Color, _dir: GradientDirection) {
        log::warn!("[GpuCanvas2D] fill_linear_gradient 暂未实现");
    }
    fn fill_radial_gradient(&mut self, _cx: f32, _cy: f32, _ir: f32, _or: f32, _ic: Color, _oc: Color) {
        log::warn!("[GpuCanvas2D] fill_radial_gradient 暂未实现");
    }

    // ═══════════════════════════════════════════
    // 阴影 — 暂未实现
    // ═══════════════════════════════════════════

    fn draw_box_shadow(&mut self, _r: Rect, _blur: f32, _ox: f32, _oy: f32, _c: Color, _rad: Option<Radius>) {
        log::warn!("[GpuCanvas2D] draw_box_shadow 暂未实现");
    }
    fn draw_box_shadow_ambient(&mut self, _r: Rect, _blur: f32, _ox: f32, _oy: f32, _c: Color, _rad: Option<Radius>) {
        log::warn!("[GpuCanvas2D] draw_box_shadow_ambient 暂未实现");
    }

    // ═══════════════════════════════════════════
    // 图像/字形 — 暂未实现
    // ═══════════════════════════════════════════

    fn blit_image(&mut self, _src: &[u32], _src_w: i32, _src_rect: Rect, _dst_rect: Rect) {
        log::warn!("[GpuCanvas2D] blit_image 暂未实现");
    }
    fn blit_glyph(&mut self, _x: i32, _y: i32, _coverage: &[u8], _w: usize, _h: usize, _color: Color) {
        log::warn!("[GpuCanvas2D] blit_glyph 暂未实现");
    }

    // ═══════════════════════════════════════════
    // 渲染状态栈
    // ═══════════════════════════════════════════

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
        self.opacity = opacity.max(0.0).min(1.0);
    }

    fn opacity(&self) -> f32 {
        self.opacity
    }

    fn set_transform(&mut self, t: Transform) {
        self.transform = t;
    }

    fn reset_transform(&mut self) {
        self.transform = Transform::identity();
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

    fn push_clip_path(&mut self, _path: &Path) {
        log::warn!("[GpuCanvas2D] push_clip_path 暂未实现");
    }

    // ═══════════════════════════════════════════
    // 像素访问（GPU 引擎不可用，返回空切片）
    // ═══════════════════════════════════════════

    fn pixels_mut(&mut self) -> &mut [u32] {
        &mut []
    }

    fn surface_size(&self) -> uix_core::Size {
        uix_core::Size::new(self.surface_w as f32, self.surface_h as f32)
    }

    fn current_clip(&self) -> Rect {
        self.clip_rect
    }
}
