//! API-neutral OpenGL ES raster implementation.
//!
//! `draw` sends only `IGraphicsContext` DTOs.  This module owns every GL
//! program, buffer, texture and framebuffer used to execute them.

use std::collections::HashMap;
use std::sync::Arc;

use glow::HasContext as _;

use crate::core::{Errc, Error, Rect, Result};
use crate::native::present::{GpuGlyphBlit, GpuSolidRect, OffscreenTargetId, SoftFallbackTile};

use super::{shaders, NativeOpenGlRuntime};

#[derive(Clone, Copy)]
pub(crate) struct TargetState {
    pub(crate) framebuffer: Option<glow::Framebuffer>,
    pub(crate) logical_width: i32,
    pub(crate) logical_height: i32,
    pub(crate) drawable_width: i32,
    pub(crate) drawable_height: i32,
    pub(crate) dpr: f32,
}

impl TargetState {
    pub(crate) fn swapchain(
        logical_width: i32,
        logical_height: i32,
        drawable_width: i32,
        drawable_height: i32,
    ) -> Self {
        let logical_width = logical_width.max(1);
        let logical_height = logical_height.max(1);
        let drawable_width = drawable_width.max(1);
        let drawable_height = drawable_height.max(1);
        Self {
            framebuffer: None,
            logical_width,
            logical_height,
            drawable_width,
            drawable_height,
            dpr: drawable_width as f32 / logical_width as f32,
        }
    }

    fn offscreen(framebuffer: glow::Framebuffer, width: i32, height: i32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        Self {
            framebuffer: Some(framebuffer),
            logical_width: width,
            logical_height: height,
            drawable_width: width,
            drawable_height: height,
            dpr: 1.0,
        }
    }
}

struct OffscreenTarget {
    framebuffer: glow::Framebuffer,
    texture: glow::Texture,
    width: i32,
    height: i32,
}

const GLYPH_ATLAS_MIN: u32 = 256;
const GLYPH_ATLAS_MAX: u32 = 2048;

#[repr(C)]
#[derive(Clone, Copy)]
struct GlyphVertex {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

#[derive(Default)]
struct GlyphAtlasCursor {
    x: u32,
    y: u32,
    row_h: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct GlyphAtlasKey {
    allocation: usize,
    len: usize,
    width: u32,
    height: u32,
}

impl GlyphAtlasKey {
    fn new(coverage: &Arc<[u8]>, width: u32, height: u32) -> Self {
        Self {
            allocation: Arc::as_ptr(coverage) as *const u8 as usize,
            len: coverage.len(),
            width,
            height,
        }
    }
}

struct GlyphAtlasEntry {
    // Keep the allocation alive so its pointer cannot be reused while this
    // atlas entry is valid. Exact payloads keep retained bytes bounded by the
    // packed atlas area.
    _coverage: Arc<[u8]>,
    uv: (f32, f32, f32, f32),
}

/// Native OpenGL ES pipeline for the current graphics context.
pub(crate) struct OpenGlRasterPipeline {
    runtime: NativeOpenGlRuntime,
    rect_vao: glow::VertexArray,
    rect_vbo: glow::Buffer,
    rect_program: glow::Program,
    rect_viewport: Option<glow::UniformLocation>,
    rect_rect: Option<glow::UniformLocation>,
    rect_color: Option<glow::UniformLocation>,
    rect_radius: Option<glow::UniformLocation>,
    glyph_vao: glow::VertexArray,
    glyph_vbo: glow::Buffer,
    glyph_program: glow::Program,
    glyph_viewport: Option<glow::UniformLocation>,
    glyph_texture_uniform: Option<glow::UniformLocation>,
    glyph_atlas_texture: Option<glow::Texture>,
    glyph_atlas_width: u32,
    glyph_atlas_height: u32,
    glyph_atlas_cursor: GlyphAtlasCursor,
    glyph_atlas_cache: HashMap<GlyphAtlasKey, GlyphAtlasEntry>,
    glyph_vertices: Vec<GlyphVertex>,
    #[cfg(test)]
    glyph_atlas_upload_count: usize,
    blit_vao: glow::VertexArray,
    blit_vbo: glow::Buffer,
    blit_bgra_program: glow::Program,
    blit_bgra_texture: Option<glow::UniformLocation>,
    blit_bgra_uv: Option<glow::UniformLocation>,
    blit_rgba_program: glow::Program,
    blit_rgba_texture: Option<glow::UniformLocation>,
    blit_rgba_uv: Option<glow::UniformLocation>,
    soft_texture: Option<glow::Texture>,
    soft_width: i32,
    soft_height: i32,
    swapchain: TargetState,
    current: TargetState,
    offscreens: Vec<Option<OffscreenTarget>>,
    free_offscreen_ids: Vec<u32>,
    next_offscreen_id: u32,
    released: bool,
}

pub(crate) fn logical_scissor_to_drawable(
    target: TargetState,
    (x, y, width, height): (i32, i32, i32, i32),
) -> (i32, i32, i32, i32) {
    let dpr = f64::from(target.dpr.max(1.0));
    let logical_right = i64::from(x).saturating_add(i64::from(width.max(0)));
    let logical_bottom = i64::from(y).saturating_add(i64::from(height.max(0)));
    let left = i64::from(x).clamp(0, i64::from(target.logical_width));
    let top = i64::from(y).clamp(0, i64::from(target.logical_height));
    let right = logical_right.clamp(left, i64::from(target.logical_width));
    let bottom = logical_bottom.clamp(top, i64::from(target.logical_height));

    // Convert both endpoints, then subtract. Independent `ceil(width * dpr)`
    // loses or gains a drawable pixel when the logical origin is fractional
    // in drawable space (for example x=1,w=2 at DPR 1.5).
    let drawable_left =
        ((left as f64 * dpr).floor() as i64).clamp(0, i64::from(target.drawable_width));
    let drawable_right =
        ((right as f64 * dpr).ceil() as i64).clamp(drawable_left, i64::from(target.drawable_width));
    let drawable_top =
        ((top as f64 * dpr).floor() as i64).clamp(0, i64::from(target.drawable_height));
    let drawable_bottom = ((bottom as f64 * dpr).ceil() as i64)
        .clamp(drawable_top, i64::from(target.drawable_height));

    (
        drawable_left as i32,
        (i64::from(target.drawable_height) - drawable_bottom) as i32,
        (drawable_right - drawable_left) as i32,
        (drawable_bottom - drawable_top) as i32,
    )
}

impl Drop for OpenGlRasterPipeline {
    fn drop(&mut self) {
        // Context implementations call `release` while their native context is
        // current. Drop deliberately does not issue GL commands after a failed
        // or already-complete native shutdown.
    }
}

const RECT_VERTICES: [f32; 12] = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 1.0];
const FULLSCREEN_VERTICES: [f32; 8] = [-1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0];

fn next_power_of_two(value: u32) -> u32 {
    value.max(1).checked_next_power_of_two().unwrap_or(u32::MAX)
}

unsafe fn create_glyph_buffer(gl: &glow::Context) -> Result<(glow::VertexArray, glow::Buffer)> {
    let vao = gl
        .create_vertex_array()
        .map_err(|error| gl_error("create glyph vertex array", error))?;
    let vbo = match gl.create_buffer() {
        Ok(vbo) => vbo,
        Err(error) => {
            gl.delete_vertex_array(vao);
            return Err(gl_error("create glyph buffer", error));
        }
    };
    let stride = std::mem::size_of::<GlyphVertex>() as i32;
    gl.bind_vertex_array(Some(vao));
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
    gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);
    gl.enable_vertex_attrib_array(0);
    gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, stride, 2 * 4);
    gl.enable_vertex_attrib_array(1);
    gl.vertex_attrib_pointer_f32(2, 4, glow::FLOAT, false, stride, 4 * 4);
    gl.enable_vertex_attrib_array(2);
    gl.bind_vertex_array(None);
    gl.bind_buffer(glow::ARRAY_BUFFER, None);
    Ok((vao, vbo))
}

unsafe fn create_r8_texture(gl: &glow::Context, width: u32, height: u32) -> Result<glow::Texture> {
    let texture = gl
        .create_texture()
        .map_err(|error| gl_error("create glyph atlas texture", error))?;
    gl.bind_texture(glow::TEXTURE_2D, Some(texture));
    gl.tex_image_2d(
        glow::TEXTURE_2D,
        0,
        glow::R8 as i32,
        width as i32,
        height as i32,
        0,
        glow::RED,
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
    gl.tex_parameter_i32(
        glow::TEXTURE_2D,
        glow::TEXTURE_WRAP_S,
        glow::CLAMP_TO_EDGE as i32,
    );
    gl.tex_parameter_i32(
        glow::TEXTURE_2D,
        glow::TEXTURE_WRAP_T,
        glow::CLAMP_TO_EDGE as i32,
    );
    gl.bind_texture(glow::TEXTURE_2D, None);
    Ok(texture)
}

unsafe fn create_quad(
    gl: &glow::Context,
    vertices: &[f32],
) -> Result<(glow::VertexArray, glow::Buffer)> {
    let vao = gl
        .create_vertex_array()
        .map_err(|error| gl_error("create_vertex_array", error))?;
    let vbo = match gl.create_buffer() {
        Ok(vbo) => vbo,
        Err(error) => {
            gl.delete_vertex_array(vao);
            return Err(gl_error("create_buffer", error));
        }
    };
    gl.bind_vertex_array(Some(vao));
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
    let bytes = std::slice::from_raw_parts(
        vertices.as_ptr() as *const u8,
        std::mem::size_of_val(vertices),
    );
    gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::STATIC_DRAW);
    gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 8, 0);
    gl.enable_vertex_attrib_array(0);
    gl.bind_vertex_array(None);
    Ok((vao, vbo))
}

unsafe fn create_texture(gl: &glow::Context, width: i32, height: i32) -> Result<glow::Texture> {
    let texture = gl
        .create_texture()
        .map_err(|error| gl_error("create_texture", error))?;
    gl.bind_texture(glow::TEXTURE_2D, Some(texture));
    gl.tex_image_2d(
        glow::TEXTURE_2D,
        0,
        glow::RGBA as i32,
        width.max(1),
        height.max(1),
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
    Ok(texture)
}

unsafe fn compile_program(
    gl: &glow::Context,
    vertex_source: &str,
    fragment_source: &str,
    label: &str,
) -> Result<glow::Program> {
    let vertex = compile_shader(gl, glow::VERTEX_SHADER, vertex_source, label)?;
    let fragment = match compile_shader(gl, glow::FRAGMENT_SHADER, fragment_source, label) {
        Ok(fragment) => fragment,
        Err(error) => {
            gl.delete_shader(vertex);
            return Err(error);
        }
    };
    let program = match gl.create_program() {
        Ok(program) => program,
        Err(error) => {
            gl.delete_shader(vertex);
            gl.delete_shader(fragment);
            return Err(gl_error("create_program", error));
        }
    };
    gl.attach_shader(program, vertex);
    gl.attach_shader(program, fragment);
    gl.link_program(program);
    gl.delete_shader(vertex);
    gl.delete_shader(fragment);
    if !gl.get_program_link_status(program) {
        let log = gl.get_program_info_log(program);
        gl.delete_program(program);
        return Err(Error::new(
            Errc::PlatformError,
            format!("OpenGL {label} program link failed: {log}"),
        ));
    }
    Ok(program)
}

unsafe fn compile_shader(
    gl: &glow::Context,
    stage: u32,
    source: &str,
    label: &str,
) -> Result<glow::Shader> {
    let shader = gl
        .create_shader(stage)
        .map_err(|error| gl_error("create_shader", error))?;
    gl.shader_source(shader, source);
    gl.compile_shader(shader);
    if !gl.get_shader_compile_status(shader) {
        let log = gl.get_shader_info_log(shader);
        gl.delete_shader(shader);
        return Err(Error::new(
            Errc::PlatformError,
            format!("OpenGL {label} shader compile failed: {log}"),
        ));
    }
    Ok(shader)
}

pub(crate) fn validate_tile(
    pixels: &[u32],
    target_width: i32,
    target_height: i32,
    tile: SoftFallbackTile,
) -> Result<()> {
    if target_width <= 0 || target_height <= 0 {
        return Err(Error::new(
            Errc::InvalidArgument,
            "OpenGL soft target extent must be positive",
        ));
    }
    tile.validate_payload(pixels)?;
    if tile.dst_x.saturating_add(tile.width) > target_width
        || tile.dst_y.saturating_add(tile.height) > target_height
    {
        return Err(Error::new(
            Errc::InvalidArgument,
            "OpenGL soft fallback tile is outside its destination target",
        ));
    }
    Ok(())
}

fn gl_error(operation: &str, error: String) -> Error {
    Error::new(
        Errc::PlatformError,
        format!("OpenGL {operation} failed: {error}"),
    )
}

mod pipeline;
mod pipeline2;
