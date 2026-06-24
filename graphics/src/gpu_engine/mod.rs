// ============================================================================
// graphics/gpu_engine/mod.rs — GPU 渲染引擎
//
// 完整 GraphicsEngine trait 实现：矩形/圆角/渐变/阴影/文字/图片/离屏。
// ============================================================================

use std::cell::RefCell;
use std::ptr;

use glow::HasContext as _;
use uix_core::{Rect, Size};
use uix_diag::Error;
use uix_platform::api::IGraphicsContext;

use crate::font_service::FontService;
use crate::{
    BlendMode, Color, DirtyRegion, FontHandle, GradientDirection, GraphicsEngine,
    ImageHandle, Radius, TextLayoutOptions, Transform,
};
use crate::types::HandleKind;

pub mod shaders;
use self::shaders::{RECT_FRAG, RECT_VERT, BLUR_FRAG, FULLSCREEN_VERT};

// ════════════════════════════════════════════════════════════════════════════
// 实例数据
// ════════════════════════════════════════════════════════════════════════════

#[repr(C)]
#[derive(Clone, Copy)]
struct RectInstance {
    rect: [f32; 4],
    tex_region: [f32; 4],
    color: [f32; 4],
    radius: [f32; 4],
    gradient: [f32; 4],
    gradient_color: [f32; 4],
}

const QUAD: [f32; 12] = [0.,0., 1.,0., 0.,1., 0.,1., 1.,0., 1.,1.];
const FS_TRI: [f32; 6] = [-1.,-1., 3.,-1., -1.,3.];

// ════════════════════════════════════════════════════════════════════════════
// 字形图集分配器
// ════════════════════════════════════════════════════════════════════════════

struct GlyphAtlas {
    tex: glow::Texture,
    w: i32, h: i32,
    cx: i32, cy: i32,
    row_h: i32,
}

impl GlyphAtlas {
    fn new(gl: &glow::Context, w: i32, h: i32) -> Result<Self, String> {
        let tex = unsafe {
            let t = gl.create_texture().map_err(|e| format!("atlas: {}", e))?;
            gl.bind_texture(glow::TEXTURE_2D, Some(t));
            gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::RGBA as i32,
                w, h, 0, glow::RGBA, glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None));
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
            t
        };
        Ok(Self { tex, w, h, cx: 0, cy: 0, row_h: 0 })
    }

    fn alloc(&mut self, gl: &glow::Context, gw: i32, gh: i32) -> (i32, i32) {
        if self.cx + gw > self.w { self.cx = 0; self.cy += self.row_h; self.row_h = 0; }
        if self.cy + gh > self.h {
            self.cx = 0; self.cy = 0; self.row_h = 0;
            let z = vec![0u8; (self.w * self.h * 4) as usize];
            unsafe {
                gl.bind_texture(glow::TEXTURE_2D, Some(self.tex));
                gl.tex_sub_image_2d(glow::TEXTURE_2D, 0, 0, 0, self.w, self.h,
                    glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelUnpackData::Slice(Some(&z)));
            }
        }
        let (x, y) = (self.cx, self.cy);
        self.cx += gw;
        self.row_h = self.row_h.max(gh);
        (x, y)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// GPU 资源存储
// ════════════════════════════════════════════════════════════════════════════

struct GpuImage {
    handle: ImageHandle,
    texture: glow::Texture,
    w: i32, h: i32,
}

struct GpuFbo {
    handle: ImageHandle,
    fbo: glow::Framebuffer,
    texture: glow::Texture,
    w: i32, h: i32,
}

// ════════════════════════════════════════════════════════════════════════════
// GpuEngine
// ════════════════════════════════════════════════════════════════════════════

pub struct GpuEngine {
    gpu_ctx: Box<dyn IGraphicsContext>,
    gl: glow::Context,
    width: i32, height: i32,
    rect_prog: glow::Program,
    blur_prog: glow::Program,
    fs_vao: glow::VertexArray, fs_vbo: glow::Buffer,
    quad_vbo: glow::Buffer, instance_vbo: glow::Buffer, vao: glow::VertexArray,
    instances: Vec<RectInstance>,
    opacity: f32, blend_mode: BlendMode,
    clip_stack: Vec<Rect>, current_clip: Rect,
    frame_begun: bool, clear_color: Color,
    glyph_atlas: Option<GlyphAtlas>,
    images: Vec<GpuImage>,
    offscreens: Vec<GpuFbo>,
    current_offscreen: Option<usize>,
    font_service: FontService,
    readback: RefCell<Vec<u32>>,
}

impl GpuEngine {
    pub fn new(mut gpu_ctx: Box<dyn IGraphicsContext>, w: i32, h: i32) -> Result<Self, Error> {
        gpu_ctx.make_current();
        let gl = {
            let loader = |name: &str| -> *const std::ffi::c_void {
                gpu_ctx.get_proc_address(name).unwrap_or(ptr::null())
            };
            unsafe { glow::Context::from_loader_function(loader) }
        };
        let rect_prog = unsafe { compile_program(&gl, RECT_VERT, RECT_FRAG) }
            .map_err(|e| Error::new(uix_diag::Errc::PlatformError, e))?;
        let blur_prog = unsafe { compile_program(&gl, FULLSCREEN_VERT, BLUR_FRAG) }
            .map_err(|e| Error::new(uix_diag::Errc::PlatformError, e))?;
        let (vao, qvbo, ivbo) = unsafe { create_rect_pipeline(&gl) }
            .map_err(|e| Error::new(uix_diag::Errc::PlatformError, e))?;
        let (fs_vao, fs_vbo) = unsafe { create_fs_pipeline(&gl) }
            .map_err(|e| Error::new(uix_diag::Errc::PlatformError, e))?;
        unsafe { gl.viewport(0, 0, w, h); gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA); }
        Ok(Self { gpu_ctx, gl, width: w, height: h, rect_prog, blur_prog, fs_vao, fs_vbo,
            quad_vbo: qvbo, instance_vbo: ivbo, vao,
            instances: Vec::with_capacity(4096), opacity: 1.0, blend_mode: BlendMode::Alpha,
            clip_stack: Vec::new(), current_clip: Rect::new(0.,0.,w as f32,h as f32),
            frame_begun: false, clear_color: Color::transparent(),
            glyph_atlas: None, images: Vec::new(), offscreens: Vec::new(),
            current_offscreen: None, font_service: FontService::new(),
            readback: RefCell::new(Vec::new()),
        })
    }

    fn flush(&mut self) {
        if self.instances.is_empty() { return; }
        let gl = &self.gl;
        unsafe {
            gl.use_program(Some(self.rect_prog));
            if let Some(l) = gl.get_uniform_location(self.rect_prog, "u_viewport") {
                gl.uniform_2_f32(Some(&l), self.width as f32, self.height as f32);
            }
            gl.active_texture(glow::TEXTURE0);
            let cur = match self.current_offscreen {
                Some(i) => self.offscreens.get(i).map(|o| o.texture),
                None => self.glyph_atlas.as_ref().map(|a| a.tex),
            };
            gl.bind_texture(glow::TEXTURE_2D, cur);
            if let Some(l) = gl.get_uniform_location(self.rect_prog, "u_texture") {
                gl.uniform_1_i32(Some(&l), 0);
            }
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.instance_vbo));
            let d: &[u8] = std::slice::from_raw_parts(
                self.instances.as_ptr() as *const u8,
                self.instances.len() * std::mem::size_of::<RectInstance>());
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, d, glow::DYNAMIC_DRAW);
            gl.bind_vertex_array(Some(self.vao));
            gl.draw_arrays_instanced(glow::TRIANGLES, 0, 6, self.instances.len() as i32);
        }
        self.instances.clear();
    }

    fn push_rect(&mut self, r: Rect, c: Color, radius: Option<Radius>,
                  tex: Option<Rect>, grad: Option<(f32,[f32;3],Color,f32)>)
    {
        let cl = r.intersect(&self.current_clip);
        if cl.is_none() { return; }
        let r = cl.unwrap_or(r);
        if r.w <= 0. || r.h <= 0. { return; }
        let a = (c.a as f32/255. * self.opacity).clamp(0.,1.);
        let rgba = [c.r as f32/255.*a, c.g as f32/255.*a, c.b as f32/255.*a, a];
        let rad = radius.map_or([0.;4], |r| [r.tl,r.tr,r.br,r.bl]);
        let tr = tex.map_or([0.;4], |t| [t.x,t.y,t.w,t.h]);
        let (gm, gp0, gp1, gp2, gc, ge) = match grad {
            Some((m,p,c2,e)) => (m,p[0],p[1],p[2],c2,e),
            None => (0.,0.,0.,0.,Color::transparent(),0.),
        };
        self.instances.push(RectInstance {
            rect: [r.x,r.y,r.w,r.h], tex_region: tr, color: rgba, radius: rad,
            gradient: [gm,gp0,gp1,gp2],
            gradient_color: [gc.r as f32/255.,gc.g as f32/255.,gc.b as f32/255.,ge],
        });
    }

    fn push_simple(&mut self, r: Rect, c: Color, rad: Option<Radius>) {
        self.push_rect(r, c, rad, None, None);
    }
    fn push_grad(&mut self, r: Rect, mode: f32, p: [f32;3], ca: Color, cb: Color, e: f32) {
        self.push_rect(r, ca, None, None, Some((mode,p,cb,e)));
    }
}

// ════════════════════════════════════════════════════════════════════════════
// GL helpers
// ════════════════════════════════════════════════════════════════════════════

unsafe fn compile_program(gl: &glow::Context, vs: &str, fs: &str) -> Result<glow::Program,String> {
    let v = compile_shader(gl, glow::VERTEX_SHADER, vs)?;
    let f = compile_shader(gl, glow::FRAGMENT_SHADER, fs)?;
    let p = gl.create_program().map_err(|e| format!("pg: {e}"))?;
    gl.attach_shader(p,v); gl.attach_shader(p,f); gl.link_program(p);
    if !gl.get_program_link_status(p) {
        let l = gl.get_program_info_log(p);
        gl.delete_program(p); gl.delete_shader(v); gl.delete_shader(f);
        return Err(format!("link: {l}"));
    }
    gl.delete_shader(v); gl.delete_shader(f); Ok(p)
}

unsafe fn compile_shader(gl: &glow::Context, ty: u32, s: &str) -> Result<glow::Shader,String> {
    let sh = gl.create_shader(ty).map_err(|e| format!("sh: {e}"))?;
    gl.shader_source(sh, s); gl.compile_shader(sh);
    if !gl.get_shader_compile_status(sh) {
        let l = gl.get_shader_info_log(sh); gl.delete_shader(sh);
        return Err(format!("compile: {l}"));
    }
    Ok(sh)
}

unsafe fn create_rect_pipeline(gl: &glow::Context) -> Result<(glow::VertexArray, glow::Buffer, glow::Buffer), String> {
    let vao = gl.create_vertex_array().map_err(|e| format!("vao: {}", e))?; gl.bind_vertex_array(Some(vao));
    let qvbo = gl.create_buffer().map_err(|e| format!("qvbo: {}", e))?;
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(qvbo));
    gl.buffer_data_u8_slice(glow::ARRAY_BUFFER,
        std::slice::from_raw_parts(QUAD.as_ptr() as *const u8, QUAD.len()*4), glow::STATIC_DRAW);
    gl.enable_vertex_attrib_array(0);
    gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 8, 0);

    let ivbo = gl.create_buffer().map_err(|e| format!("ivbo: {}", e))?;
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(ivbo));
    let str = std::mem::size_of::<RectInstance>() as i32;
    for (i, off) in [(1,0),(2,16),(3,32),(4,48),(5,64),(6,80)] {
        gl.enable_vertex_attrib_array(i);
        gl.vertex_attrib_pointer_f32(i, 4, glow::FLOAT, false, str, off);
        gl.vertex_attrib_divisor(i, 1);
    }
    Ok((vao, qvbo, ivbo))
}

unsafe fn create_fs_pipeline(gl: &glow::Context) -> Result<(glow::VertexArray, glow::Buffer), String> {
    let vao = gl.create_vertex_array().map_err(|e| format!("fs: {}", e))?; gl.bind_vertex_array(Some(vao));
    let vbo = gl.create_buffer().map_err(|e| format!("fsv: {}", e))?;
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
    gl.buffer_data_u8_slice(glow::ARRAY_BUFFER,
        std::slice::from_raw_parts(FS_TRI.as_ptr() as *const u8, FS_TRI.len()*4), glow::STATIC_DRAW);
    gl.enable_vertex_attrib_array(0);
    gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 8, 0);
    Ok((vao, vbo))
}

unsafe fn blur_pass(gl: &glow::Context, prog: glow::Program, vao: glow::VertexArray,
                     src: glow::Texture, tmp: glow::Texture, tmp_fbo: glow::Framebuffer,
                     w: i32, h: i32)
{
    gl.use_program(Some(prog)); gl.bind_vertex_array(Some(vao));
    let tl = gl.get_uniform_location(prog, "u_texel_size");
    if let Some(l) = tl { gl.uniform_2_f32(Some(&l), 1./w as f32, 1./h as f32); }
    let dl = gl.get_uniform_location(prog, "u_direction");
    let sl = gl.get_uniform_location(prog, "u_source");

    gl.bind_framebuffer(glow::FRAMEBUFFER, Some(tmp_fbo));
    gl.viewport(0, 0, w, h);
    gl.active_texture(glow::TEXTURE0); gl.bind_texture(glow::TEXTURE_2D, Some(src));
    if let Some(l) = sl { gl.uniform_1_i32(Some(&l), 0); }
    if let Some(l) = dl { gl.uniform_2_f32(Some(&l), 1., 0.); }
    gl.draw_arrays(glow::TRIANGLES, 0, 3);

    gl.bind_framebuffer(glow::FRAMEBUFFER, None);
    gl.active_texture(glow::TEXTURE0); gl.bind_texture(glow::TEXTURE_2D, Some(tmp));
    if let Some(l) = dl { gl.uniform_2_f32(Some(&l), 0., 1.); }
    gl.draw_arrays(glow::TRIANGLES, 0, 3);
}

// ════════════════════════════════════════════════════════════════════════════
// GraphicsEngine impl
// ════════════════════════════════════════════════════════════════════════════

impl GraphicsEngine for GpuEngine {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn initialize(&mut self, w: i32, h: i32, system_info: &dyn uix_platform::ISystemInfo) -> Result<(), Error> {
        self.width=w; self.height=h; self.current_clip=Rect::new(0.,0.,w as f32,h as f32);
        unsafe{self.gl.viewport(0,0,w,h);} self.font_service.load_default_system_font(14.0, system_info); Ok(())
    }
    fn shutdown(&mut self) {
        self.flush();
        unsafe {
            self.gl.delete_program(self.rect_prog); self.gl.delete_program(self.blur_prog);
            self.gl.delete_buffer(self.quad_vbo); self.gl.delete_buffer(self.instance_vbo);
            self.gl.delete_buffer(self.fs_vbo);
            self.gl.delete_vertex_array(self.vao); self.gl.delete_vertex_array(self.fs_vao);
            if let Some(a)=&self.glyph_atlas { self.gl.delete_texture(a.tex); }
            for img in &self.images { self.gl.delete_texture(img.texture); }
            for fo in &self.offscreens { self.gl.delete_framebuffer(fo.fbo); self.gl.delete_texture(fo.texture); }
        }
        self.gpu_ctx.shutdown();
    }
    fn resize(&mut self, w: i32, h: i32) {
        self.width=w; self.height=h; self.current_clip=Rect::new(0.,0.,w as f32,h as f32);
        self.gpu_ctx.resize(w,h); unsafe{self.gl.viewport(0,0,w,h);}
    }
    fn begin_frame(&mut self, d: &DirtyRegion) {
        self.gpu_ctx.make_current(); self.frame_begun=true; self.instances.clear();
        if d.full_frame {
            let c=self.clear_color;
            unsafe{self.gl.clear_color(c.r as f32/255.,c.g as f32/255.,c.b as f32/255.,c.a as f32/255.);}
            unsafe{self.gl.clear(glow::COLOR_BUFFER_BIT);}
        }
    }
    fn end_frame(&mut self, d: &DirtyRegion) {
        self.flush();
        unsafe { self.gl.flush(); }
        let sz=(self.width*self.height) as usize;
        let mut rb=self.readback.borrow_mut();
        if rb.len() != sz { rb.resize(sz,0); }
        if d.full_frame {
            unsafe {
                let bytes: &mut [u8] = std::slice::from_raw_parts_mut(rb.as_mut_ptr() as *mut u8, sz*4);
                self.gl.read_pixels(0,0,self.width,self.height, glow::BGRA, glow::UNSIGNED_BYTE,
                    glow::PixelPackData::Slice(Some(bytes)));
            }
        } else {
            let bounds = d.bounds();
            if bounds.w > 0.0 && bounds.h > 0.0 {
            let bx = bounds.x as i32;
            let by = bounds.y as i32;
            let bw = (bounds.w as i32).min(self.width - bx);
            let bh = (bounds.h as i32).min(self.height - by);
            if bw > 0 && bh > 0 {
                let mut row_buf = vec![0u32; bw as usize];
                unsafe {
                    for row in 0..bh {
                        self.gl.read_pixels(
                            bx, by + row, bw, 1,
                            glow::BGRA, glow::UNSIGNED_BYTE,
                            glow::PixelPackData::Slice(Some(
                                std::slice::from_raw_parts_mut(
                                    row_buf.as_mut_ptr() as *mut u8,
                                    bw as usize * 4,
                                )
                            )),
                        );
                        let dst_start = ((by + row) * self.width + bx) as usize;
                        let copy_len = row_buf.len().min(rb.len().saturating_sub(dst_start));
                        rb[dst_start..dst_start + copy_len].copy_from_slice(&row_buf[..copy_len]);
                    }
                }
            }
        }
        }
        self.frame_begun=false;
    }

    fn scroll_region(&mut self, vp: Rect, dx: f32, dy: f32) {
        if dx.abs()<0.5 && dy.abs()<0.5 { return; }
        unsafe {
            self.gl.copy_tex_sub_image_2d(
                glow::TEXTURE_2D, 0, (vp.x+dx) as i32, (vp.y+dy) as i32,
                0, 0, vp.w as i32, vp.h as i32);
        }
    }

    fn push_clip_rect(&mut self, r: Rect) {
        let p=self.current_clip; self.clip_stack.push(p);
        self.current_clip=p.intersect(&r).unwrap_or(Rect::zero());
    }
    fn pop_clip_rect(&mut self) {
        self.current_clip=self.clip_stack.pop().unwrap_or_else(||
            Rect::new(0.,0.,self.width as f32,self.height as f32));
    }
    fn set_opacity(&mut self, o: f32) { self.opacity=o; }
    fn opacity(&self) -> f32 { self.opacity }
    fn save(&mut self){} fn restore(&mut self){}
    fn set_transform(&mut self, _: Transform){} fn reset_transform(&mut self){}
    fn set_blend_mode(&mut self, m: BlendMode) {
        self.blend_mode=m;
        let (s,d)=match m { BlendMode::Alpha=>(glow::SRC_ALPHA,glow::ONE_MINUS_SRC_ALPHA),
            BlendMode::SrcOver=>(glow::ONE,glow::ONE_MINUS_SRC_ALPHA),
            BlendMode::Additive=>(glow::SRC_ALPHA,glow::ONE), };
        unsafe{self.gl.blend_func(s,d);}
    }

    fn fill_rect(&mut self, r: Rect, c: Color, rad: Option<Radius>) { self.push_simple(r,c,rad); }
    fn stroke_rect(&mut self, r: Rect, c: Color, lw: f32, _: Option<Radius>) {
        let w=lw.max(0.5);
        self.push_simple(Rect::new(r.x,r.y,r.w,w),c,None);
        self.push_simple(Rect::new(r.x,r.y+r.h-w,r.w,w),c,None);
        self.push_simple(Rect::new(r.x,r.y,w,r.h),c,None);
        self.push_simple(Rect::new(r.x+r.w-w,r.y,w,r.h),c,None);
    }
    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, c: Color) {
        self.push_simple(Rect::new(cx-r,cy-r,r*2.,r*2.),c,Some(Radius::uniform(r)));
    }
    fn fill_circle_radial(&mut self, cx: f32, cy: f32, r: f32, c: Color) { self.fill_circle(cx,cy,r,c); }
    fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, c: Color, lw: f32) {
        let o=r+lw*0.5;
        self.push_simple(Rect::new(cx-o,cy-o,o*2.,o*2.),c,Some(Radius::uniform(o)));
    }
    fn fill_sector(&mut self, cx: f32, cy: f32, r: f32, sa: f32, ea: f32, c: Color) {
        // 扇区近似：用三角形逼近弧线（4 分段）
        let steps=4; let span=(ea-sa)/steps as f32;
        for i in 0..steps {
            let a1=sa+span*i as f32; let a2=a1+span;
            let x1=cx+r*a1.cos(); let y1=cy+r*a1.sin();
            let x2=cx+r*a2.cos(); let y2=cy+r*a2.sin();
            let minx=cx.min(x1).min(x2); let miny=cy.min(y1).min(y2);
            let maxx=cx.max(x1).max(x2); let maxy=cy.max(y1).max(y2);
            self.push_simple(Rect::new(minx,miny,maxx-minx,maxy-miny),c,None);
        }
    }
    fn fill_ellipse(&mut self, r: Rect, c: Color) {
        // 椭圆用非均匀圆角近似：用包围盒的中心为圆心，半轴=w/2, h/2
        let rx=r.w*0.5; let ry=r.h*0.5;
        self.push_simple(r, c, Some(Radius { tl: rx.min(ry), tr: rx.min(ry), br: rx.min(ry), bl: rx.min(ry) }));
    }
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, c: Color, w: f32) {
        let (dx,dy)=(x2-x1,y2-y1); let l=(dx*dx+dy*dy).sqrt();
        if l<0.5{return;} let hw=w*0.5;
        self.push_simple(Rect::new(x1.min(x2)-hw,y1.min(y2)-hw,(x1.max(x2)-x1.min(x2))+hw*2.,(y1.max(y2)-y1.min(y2))+hw*2.),c,None);
    }

    fn draw_box_shadow(&mut self, rect: Rect, blur: f32, ox: f32, oy: f32, c: Color, rad: Option<Radius>) {
        let pad=blur*2.; let sr=Rect::new(rect.x+ox-pad,rect.y+oy-pad,rect.w+pad*2.,rect.h+pad*2.);
        if blur<2. { self.push_simple(sr,Color::from_rgba(c.r,c.g,c.b,(c.a as f32*0.3)as u8),rad); return; }
        let (sw,sh)=(sr.w.ceil()as i32+4,sr.h.ceil()as i32+4);
        let ttex=match unsafe{self.gl.create_texture()} {
            Ok(t) => t,
            Err(e) => { log::error!("draw_box_shadow: create texture failed: {}", e); return; }
        };
        let tfbo=match unsafe{self.gl.create_framebuffer()} {
            Ok(f) => f,
            Err(e) => { log::error!("draw_box_shadow: create framebuffer failed: {}", e); return; }
        };
        unsafe {
            self.gl.bind_texture(glow::TEXTURE_2D,Some(ttex));
            self.gl.tex_image_2d(glow::TEXTURE_2D,0,glow::RGBA as i32,sw,sh,0,glow::RGBA,glow::UNSIGNED_BYTE,glow::PixelUnpackData::Slice(None));
            self.gl.tex_parameter_i32(glow::TEXTURE_2D,glow::TEXTURE_MIN_FILTER,glow::LINEAR as i32);
            self.gl.tex_parameter_i32(glow::TEXTURE_2D,glow::TEXTURE_MAG_FILTER,glow::LINEAR as i32);
            self.gl.bind_framebuffer(glow::FRAMEBUFFER,Some(tfbo));
            self.gl.framebuffer_texture_2d(glow::FRAMEBUFFER,glow::COLOR_ATTACHMENT0,glow::TEXTURE_2D,Some(ttex),0);
            self.gl.viewport(0,0,sw,sh); self.gl.clear_color(0.,0.,0.,0.); self.gl.clear(glow::COLOR_BUFFER_BIT);
        }
        let (ow,oh)=(self.width,self.height);
        self.width=sw; self.height=sh; self.current_clip=Rect::new(0.,0.,sw as f32,sh as f32);
        self.push_simple(Rect::new(pad+2.,pad+2.,rect.w+blur*2.,rect.h+blur*2.),c,rad);
        self.flush();
        self.width=ow; self.height=oh; self.current_clip=Rect::new(0.,0.,ow as f32,oh as f32);
        unsafe {
            blur_pass(&self.gl,self.blur_prog,self.fs_vao,ttex,ttex,tfbo,sw,sh);
            self.gl.bind_framebuffer(glow::FRAMEBUFFER,None); self.gl.viewport(0,0,ow,oh);
            self.gl.delete_framebuffer(tfbo); self.gl.delete_texture(ttex);
        }
        self.push_simple(sr,Color::from_rgba(c.r,c.g,c.b,(c.a as f32*0.3)as u8),rad);
    }
    fn draw_box_shadow_ambient(&mut self, r: Rect, blur: f32, ox: f32, oy: f32, c: Color, rad: Option<Radius>) {
        let sr=Rect::new(r.x+ox-blur*2.,r.y+oy-blur*2.,r.w+blur*4.,r.h+blur*4.);
        self.push_simple(sr,Color::from_rgba(c.r,c.g,c.b,(c.a as f32*0.15)as u8),rad);
    }

    fn fill_linear_gradient(&mut self, r: Rect, ca: Color, cb: Color, dir: GradientDirection) {
        let ang=match dir { GradientDirection::Vertical=>std::f32::consts::FRAC_PI_2,
            GradientDirection::Horizontal=>0., GradientDirection::DiagonalTLBR=>std::f32::consts::FRAC_PI_4,
            GradientDirection::DiagonalBLTR=>-std::f32::consts::FRAC_PI_4, };
        self.push_grad(r,1.,[ang,0.,0.],ca,cb,0.);
    }
    fn fill_radial_gradient(&mut self, cx: f32, cy: f32, ir: f32, or: f32, ic: Color, oc: Color) {
        let d=or*2.; let r=Rect::new(cx-or,cy-or,d,d);
        self.push_grad(r,2.,[0.5,0.5,ir],ic,oc,or);
    }

    fn measure_text(&self, font: &FontHandle, text: &str, opts: &TextLayoutOptions) -> Size {
        let be_opts: crate::text_backend::TextLayoutOptions = (*opts).into();
        self.font_service.measure_text(font, text, &be_opts)
    }
    fn draw_glyph_raster(&mut self, x: i32, y: i32, cov: &[u8], w: usize, h: usize, c: Color) {
        if self.glyph_atlas.is_none() { self.glyph_atlas=GlyphAtlas::new(&self.gl,2048,2048).ok(); }
        let atlas = match self.glyph_atlas.as_mut() {
            Some(a) => a,
            None => { log::warn!("[GpuEngine] glyph atlas unavailable, skipping glyph"); return; }
        };
        let (ax,ay)=atlas.alloc(&self.gl,w as i32,h as i32);
        let rgba: Vec<u8>=cov.iter().flat_map(|&x| [255,255,255,x]).collect();
        unsafe {
            self.gl.bind_texture(glow::TEXTURE_2D,Some(atlas.tex));
            self.gl.tex_sub_image_2d(glow::TEXTURE_2D,0,ax,ay,w as i32,h as i32,
                glow::RGBA,glow::UNSIGNED_BYTE,glow::PixelUnpackData::Slice(Some(&rgba)));
        }
        let aw=atlas.w as f32; let ah=atlas.h as f32;
        self.push_rect(
            Rect::new(x as f32,y as f32,w as f32,h as f32), c, None,
            Some(Rect::new(ax as f32/aw, ay as f32/ah, w as f32/aw, h as f32/ah)),
            None);
    }

    fn load_image(&mut self, pixels: Vec<u32>, w: i32, h: i32) -> Result<&mut ImageHandle, Error> {
        let tex=unsafe{self.gl.create_texture().map_err(|e| Error::new(uix_diag::Errc::PlatformError, format!("img_tex: {}", e)))}?;
        unsafe {
            self.gl.bind_texture(glow::TEXTURE_2D,Some(tex));
            let bytes: &[u8]=std::slice::from_raw_parts(pixels.as_ptr() as *const u8, pixels.len()*4);
            self.gl.tex_image_2d(glow::TEXTURE_2D,0,glow::RGBA as i32,w,h,0,
                glow::BGRA,glow::UNSIGNED_BYTE,glow::PixelUnpackData::Slice(Some(bytes)));
            self.gl.tex_parameter_i32(glow::TEXTURE_2D,glow::TEXTURE_MIN_FILTER,glow::LINEAR as i32);
            self.gl.tex_parameter_i32(glow::TEXTURE_2D,glow::TEXTURE_MAG_FILTER,glow::LINEAR as i32);
        }
        let idx=self.images.len() as u32;
        self.images.push(GpuImage{handle:ImageHandle::new(idx,HandleKind::Image),texture:tex,w,h});
        Ok(&mut self.images[idx as usize].handle)
    }
    fn unload_image(&mut self, h: &ImageHandle) {
        if let Some(pos)=self.images.iter().position(|x| x.handle.index==h.index&&x.handle.kind==h.kind) {
            let img=self.images.remove(pos);
            unsafe{self.gl.delete_texture(img.texture);}
        }
    }
    fn image_size(&self, h: &ImageHandle) -> Size {
        self.images.iter().find(|x| x.handle.index==h.index).map_or(Size::zero(),|x| Size::new(x.w as f32,x.h as f32))
    }
    fn draw_image(&mut self, h: &ImageHandle, src: Rect, dst: Rect) {
        if let Some(img)=self.images.iter().find(|x| x.handle.index==h.index) {
            self.push_rect(dst, Color::white(), None,
                Some(Rect::new(src.x/img.w as f32,src.y/img.h as f32,src.w/img.w as f32,src.h/img.h as f32)),
                None);
        }
    }

    fn create_offscreen(&mut self, w: i32, h: i32) -> Result<&mut ImageHandle, Error> {
        let tex=unsafe{self.gl.create_texture().map_err(|e| Error::new(uix_diag::Errc::PlatformError, format!("off_tex: {}", e)))}?;
        let fbo=unsafe{self.gl.create_framebuffer().map_err(|e| Error::new(uix_diag::Errc::PlatformError, format!("off_fbo: {}", e)))}?;
        unsafe {
            self.gl.bind_texture(glow::TEXTURE_2D,Some(tex));
            self.gl.tex_image_2d(glow::TEXTURE_2D,0,glow::RGBA as i32,w,h,0,glow::RGBA,glow::UNSIGNED_BYTE,glow::PixelUnpackData::Slice(None));
            self.gl.tex_parameter_i32(glow::TEXTURE_2D,glow::TEXTURE_MIN_FILTER,glow::LINEAR as i32);
            self.gl.tex_parameter_i32(glow::TEXTURE_2D,glow::TEXTURE_MAG_FILTER,glow::LINEAR as i32);
            self.gl.bind_framebuffer(glow::FRAMEBUFFER,Some(fbo));
            self.gl.framebuffer_texture_2d(glow::FRAMEBUFFER,glow::COLOR_ATTACHMENT0,glow::TEXTURE_2D,Some(tex),0);
            self.gl.bind_framebuffer(glow::FRAMEBUFFER,None);
        }
        let idx=self.offscreens.len() as u32;
        self.offscreens.push(GpuFbo{handle:ImageHandle::new(idx,HandleKind::Offscreen),fbo,texture:tex,w,h});
        Ok(&mut self.offscreens[idx as usize].handle)
    }
    fn destroy_offscreen(&mut self, h: &ImageHandle) {
        if let Some(p)=self.offscreens.iter().position(|x| x.handle.index==h.index) {
            let fo=self.offscreens.remove(p);
            unsafe{self.gl.delete_framebuffer(fo.fbo); self.gl.delete_texture(fo.texture);}
        }
    }
    fn begin_offscreen(&mut self, h: &ImageHandle) {
        if let Some(p)=self.offscreens.iter().position(|x| x.handle.index==h.index) {
            let fo=&self.offscreens[p];
            unsafe {
                self.gl.bind_framebuffer(glow::FRAMEBUFFER,Some(fo.fbo));
                self.gl.viewport(0,0,fo.w,fo.h);
                self.gl.clear_color(0.,0.,0.,0.); self.gl.clear(glow::COLOR_BUFFER_BIT);
            }
            self.current_offscreen=Some(p);
        }
    }
    fn end_offscreen(&mut self) {
        unsafe{self.gl.bind_framebuffer(glow::FRAMEBUFFER,None); self.gl.viewport(0,0,self.width,self.height);}
        self.current_offscreen=None;
    }

    fn pixels(&self) -> &[u32] {
        let rb=self.readback.borrow();
        if rb.is_empty() { return &[]; }
        let ptr=rb.as_ptr(); let len=rb.len(); drop(rb);
        unsafe{std::slice::from_raw_parts(ptr,len)}
    }
    fn width(&self) -> i32 { self.width }
    fn height(&self) -> i32 { self.height }
    fn load_font(&mut self, data: &[u8]) -> Result<FontHandle, Error> { self.font_service.load_font(data) }
    fn font_service(&self) -> &FontService { &self.font_service }
}

impl Drop for GpuEngine { fn drop(&mut self) { self.shutdown(); } }
