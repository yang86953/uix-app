//! API-neutral OpenGL ES raster implementation.
//!
//! `draw` sends only `IGraphicsContext` DTOs.  This module owns every GL
//! program, buffer, texture and framebuffer used to execute them.

use std::collections::HashMap;
use std::sync::Arc;

use glow::HasContext as _;

use crate::core::{Errc, Error, Rect, Result};
use crate::native::traits::present::{
    GpuGlyphBlit, GpuSolidRect, OffscreenTargetId, SoftFallbackTile,
};

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

impl OpenGlRasterPipeline {
    pub(crate) fn new(
        runtime: NativeOpenGlRuntime,
        logical_width: i32,
        logical_height: i32,
        drawable_width: i32,
        drawable_height: i32,
    ) -> Result<Self> {
        let gl = runtime.context();
        let rect_program =
            unsafe { compile_program(gl, shaders::RECT_VERT, shaders::RECT_FRAG, "rect")? };
        let (rect_vao, rect_vbo) = unsafe { create_quad(gl, &RECT_VERTICES)? };
        let glyph_program =
            unsafe { compile_program(gl, shaders::GLYPH_VERT, shaders::GLYPH_FRAG, "glyph")? };
        let (glyph_vao, glyph_vbo) = unsafe { create_glyph_buffer(gl)? };
        let blit_bgra_program = unsafe {
            compile_program(
                gl,
                shaders::FULLSCREEN_VERT,
                shaders::BLIT_FRAG,
                "bgra blit",
            )?
        };
        let blit_rgba_program = unsafe {
            compile_program(
                gl,
                shaders::FULLSCREEN_VERT,
                shaders::BLIT_RGBA_FRAG,
                "rgba blit",
            )?
        };
        let (blit_vao, blit_vbo) = unsafe { create_quad(gl, &FULLSCREEN_VERTICES)? };
        let swapchain = TargetState::swapchain(
            logical_width,
            logical_height,
            drawable_width,
            drawable_height,
        );

        let pipeline = Self {
            rect_vao,
            rect_vbo,
            rect_program,
            rect_viewport: unsafe { gl.get_uniform_location(rect_program, "u_viewport") },
            rect_rect: unsafe { gl.get_uniform_location(rect_program, "u_rect") },
            rect_color: unsafe { gl.get_uniform_location(rect_program, "u_color") },
            rect_radius: unsafe { gl.get_uniform_location(rect_program, "u_radius") },
            glyph_vao,
            glyph_vbo,
            glyph_program,
            glyph_viewport: unsafe { gl.get_uniform_location(glyph_program, "u_viewport") },
            glyph_texture_uniform: unsafe { gl.get_uniform_location(glyph_program, "u_atlas") },
            glyph_atlas_texture: None,
            glyph_atlas_width: 0,
            glyph_atlas_height: 0,
            glyph_atlas_cursor: GlyphAtlasCursor::default(),
            glyph_atlas_cache: HashMap::new(),
            glyph_vertices: Vec::new(),
            #[cfg(test)]
            glyph_atlas_upload_count: 0,
            blit_vao,
            blit_vbo,
            blit_bgra_program,
            blit_bgra_texture: unsafe { gl.get_uniform_location(blit_bgra_program, "u_tex") },
            blit_bgra_uv: unsafe { gl.get_uniform_location(blit_bgra_program, "u_uv_rect") },
            blit_rgba_program,
            blit_rgba_texture: unsafe { gl.get_uniform_location(blit_rgba_program, "u_tex") },
            blit_rgba_uv: unsafe { gl.get_uniform_location(blit_rgba_program, "u_uv_rect") },
            // Glyph-only/native-only frames must not retain an unused
            // full-target RGBA soft-upload texture.
            soft_texture: None,
            soft_width: 0,
            soft_height: 0,
            runtime,
            swapchain,
            current: swapchain,
            offscreens: Vec::new(),
            free_offscreen_ids: Vec::new(),
            next_offscreen_id: 0,
            released: false,
        };
        pipeline.restore_full_viewport();
        Ok(pipeline)
    }

    fn gl(&self) -> &glow::Context {
        self.runtime.context()
    }

    fn check_gl_error(&self, operation: &str) -> Result<()> {
        let error = unsafe { self.gl().get_error() };
        if error == glow::NO_ERROR {
            Ok(())
        } else {
            Err(Error::new(
                Errc::PlatformError,
                format!("OpenGL {operation} failed with error {error:#X}"),
            ))
        }
    }

    pub(crate) fn resize_swapchain(
        &mut self,
        logical_width: i32,
        logical_height: i32,
        drawable_width: i32,
        drawable_height: i32,
    ) {
        self.swapchain = TargetState::swapchain(
            logical_width,
            logical_height,
            drawable_width,
            drawable_height,
        );
        if self.current.framebuffer.is_none() {
            self.current = self.swapchain;
            self.restore_full_viewport();
        }
    }

    pub(crate) fn clear_render_target(&mut self, rgba: [f32; 4]) -> Result<()> {
        unsafe {
            self.gl().disable(glow::SCISSOR_TEST);
            self.gl().clear_color(rgba[0], rgba[1], rgba[2], rgba[3]);
            self.gl().clear(glow::COLOR_BUFFER_BIT);
        }
        self.restore_full_viewport();
        self.check_gl_error("clear_render_target")
    }

    pub(crate) fn current_target_size(&self) -> (i32, i32) {
        (self.current.logical_width, self.current.logical_height)
    }

    pub(crate) fn clear_rects(&mut self, rects: &[GpuSolidRect]) -> Result<()> {
        for rect in rects {
            if rect.w <= 0.0 || rect.h <= 0.0 {
                continue;
            }
            self.apply_scissor(Some((
                rect.x.floor() as i32,
                rect.y.floor() as i32,
                rect.w.ceil() as i32,
                rect.h.ceil() as i32,
            )));
            unsafe {
                self.gl().clear_color(0.0, 0.0, 0.0, 0.0);
                self.gl().clear(glow::COLOR_BUFFER_BIT);
            }
        }
        self.apply_scissor(None);
        self.check_gl_error("clear_rects")
    }

    pub(crate) fn draw_solid_rects(
        &mut self,
        viewport_width: f32,
        viewport_height: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuSolidRect],
    ) -> Result<()> {
        self.apply_scissor(scissor);
        unsafe {
            self.gl().enable(glow::BLEND);
            self.gl().blend_func_separate(
                glow::ONE,
                glow::ONE_MINUS_SRC_ALPHA,
                glow::ONE,
                glow::ONE_MINUS_SRC_ALPHA,
            );
            self.gl().use_program(Some(self.rect_program));
            self.gl().uniform_2_f32(
                self.rect_viewport.as_ref(),
                viewport_width.max(1.0),
                viewport_height.max(1.0),
            );
            self.gl().bind_vertex_array(Some(self.rect_vao));
            for rect in rects {
                if rect.w <= 0.0 || rect.h <= 0.0 {
                    continue;
                }
                self.gl()
                    .uniform_4_f32(self.rect_rect.as_ref(), rect.x, rect.y, rect.w, rect.h);
                self.gl().uniform_4_f32(
                    self.rect_color.as_ref(),
                    rect.rgba[0],
                    rect.rgba[1],
                    rect.rgba[2],
                    rect.rgba[3],
                );
                self.gl().uniform_4_f32(
                    self.rect_radius.as_ref(),
                    rect.radius[0],
                    rect.radius[1],
                    rect.radius[2],
                    rect.radius[3],
                );
                self.gl().draw_arrays(glow::TRIANGLES, 0, 6);
            }
            self.gl().bind_vertex_array(None);
        }
        self.check_gl_error("draw_solid_rects")
    }

    pub(crate) fn draw_glyphs(
        &mut self,
        viewport_width: f32,
        viewport_height: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        glyphs: &[GpuGlyphBlit],
    ) -> Result<()> {
        if glyphs.is_empty() || viewport_width <= 0.0 || viewport_height <= 0.0 {
            return Ok(());
        }
        self.glyph_vertices.clear();

        for glyph in glyphs {
            if glyph.w <= 0.0 || glyph.h <= 0.0 || glyph.cov_w == 0 || glyph.cov_h == 0 {
                continue;
            }
            let width = glyph.cov_w;
            let height = glyph.cov_h;
            if width > GLYPH_ATLAS_MAX || height > GLYPH_ATLAS_MAX {
                return Err(Error::new(
                    Errc::InvalidArgument,
                    format!("OpenGL glyph {width}x{height} exceeds atlas max {GLYPH_ATLAS_MAX}"),
                ));
            }

            // Only exact payloads enter the persistent cache. Retaining an
            // arbitrary trailing payload could exceed the bounded atlas area.
            let expected = (width as usize).saturating_mul(height as usize);
            let cache_key = (glyph.coverage.len() == expected)
                .then(|| GlyphAtlasKey::new(&glyph.coverage, width, height));
            if let Some(uv) = cache_key
                .as_ref()
                .and_then(|key| self.glyph_atlas_cache.get(key))
                .map(|entry| entry.uv)
            {
                Self::push_glyph_quad(&mut self.glyph_vertices, glyph, uv);
                continue;
            }

            if self.glyph_atlas_texture.is_none() {
                self.ensure_glyph_atlas(width, height)?;
            }
            let wanted_width = next_power_of_two(width).clamp(GLYPH_ATLAS_MIN, GLYPH_ATLAS_MAX);
            let wanted_height = next_power_of_two(height).clamp(GLYPH_ATLAS_MIN, GLYPH_ATLAS_MAX);
            if wanted_width > self.glyph_atlas_width || wanted_height > self.glyph_atlas_height {
                self.flush_glyph_batch(viewport_width, viewport_height, scissor)?;
                self.ensure_glyph_atlas(width, height)?;
            }

            if !self.glyph_atlas_can_fit(width, height) {
                self.flush_glyph_batch(viewport_width, viewport_height, scissor)?;
                let grown_width = next_power_of_two(self.glyph_atlas_width.max(width))
                    .clamp(GLYPH_ATLAS_MIN, GLYPH_ATLAS_MAX);
                let grown_height =
                    next_power_of_two(self.glyph_atlas_height.saturating_add(height))
                        .clamp(GLYPH_ATLAS_MIN, GLYPH_ATLAS_MAX);
                if grown_width > self.glyph_atlas_width || grown_height > self.glyph_atlas_height {
                    self.ensure_glyph_atlas(grown_width, grown_height)?;
                }
                self.reset_glyph_atlas();
                if !self.glyph_atlas_can_fit(width, height) {
                    return Err(Error::new(
                        Errc::PlatformError,
                        "OpenGL glyph does not fit in atlas after grow",
                    ));
                }
            }

            let uv = self.pack_glyph(&glyph.coverage, width, height)?;
            if let Some(cache_key) = cache_key {
                self.glyph_atlas_cache.insert(
                    cache_key,
                    GlyphAtlasEntry {
                        _coverage: Arc::clone(&glyph.coverage),
                        uv,
                    },
                );
            }
            Self::push_glyph_quad(&mut self.glyph_vertices, glyph, uv);
        }

        self.flush_glyph_batch(viewport_width, viewport_height, scissor)
    }

    fn ensure_glyph_atlas(&mut self, needed_width: u32, needed_height: u32) -> Result<()> {
        let width = next_power_of_two(self.glyph_atlas_width.max(needed_width))
            .clamp(GLYPH_ATLAS_MIN, GLYPH_ATLAS_MAX);
        let height = next_power_of_two(self.glyph_atlas_height.max(needed_height))
            .clamp(GLYPH_ATLAS_MIN, GLYPH_ATLAS_MAX);
        if self.glyph_atlas_texture.is_some()
            && width <= self.glyph_atlas_width
            && height <= self.glyph_atlas_height
        {
            return Ok(());
        }

        let texture = unsafe { create_r8_texture(self.gl(), width, height)? };
        if let Some(previous) = self.glyph_atlas_texture.replace(texture) {
            unsafe { self.gl().delete_texture(previous) };
        }
        self.glyph_atlas_width = width;
        self.glyph_atlas_height = height;
        self.reset_glyph_atlas();
        Ok(())
    }

    fn reset_glyph_atlas(&mut self) {
        self.glyph_atlas_cursor = GlyphAtlasCursor::default();
        self.glyph_atlas_cache.clear();
    }

    fn glyph_atlas_can_fit(&self, width: u32, height: u32) -> bool {
        if self.glyph_atlas_texture.is_none()
            || self.glyph_atlas_width == 0
            || self.glyph_atlas_height == 0
        {
            return false;
        }
        let mut x = self.glyph_atlas_cursor.x;
        let mut y = self.glyph_atlas_cursor.y;
        let mut row_height = self.glyph_atlas_cursor.row_h;
        if x.saturating_add(width) > self.glyph_atlas_width {
            x = 0;
            y = y.saturating_add(row_height);
            row_height = 0;
        }
        x.saturating_add(width) <= self.glyph_atlas_width
            && y.saturating_add(height) <= self.glyph_atlas_height
            && row_height.max(height) <= self.glyph_atlas_height
    }

    fn pack_glyph(
        &mut self,
        coverage: &[u8],
        width: u32,
        height: u32,
    ) -> Result<(f32, f32, f32, f32)> {
        if self.glyph_atlas_cursor.x.saturating_add(width) > self.glyph_atlas_width {
            self.glyph_atlas_cursor.x = 0;
            self.glyph_atlas_cursor.y = self
                .glyph_atlas_cursor
                .y
                .saturating_add(self.glyph_atlas_cursor.row_h);
            self.glyph_atlas_cursor.row_h = 0;
        }
        if !self.glyph_atlas_can_fit(width, height) {
            return Err(Error::new(
                Errc::PlatformError,
                "OpenGL pack_glyph called without atlas space",
            ));
        }
        let expected = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| Error::new(Errc::InvalidArgument, "OpenGL glyph extent overflows"))?;
        if coverage.len() < expected {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "OpenGL glyph coverage too small, got {}, need {expected}",
                    coverage.len()
                ),
            ));
        }
        let texture = self.glyph_atlas_texture.ok_or_else(|| {
            Error::new(Errc::PlatformError, "OpenGL glyph atlas texture is missing")
        })?;
        let x = self.glyph_atlas_cursor.x;
        let y = self.glyph_atlas_cursor.y;
        unsafe {
            self.gl().active_texture(glow::TEXTURE0);
            self.gl().bind_texture(glow::TEXTURE_2D, Some(texture));
            self.gl().pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            self.gl().tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                x as i32,
                y as i32,
                width as i32,
                height as i32,
                glow::RED,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(&coverage[..expected])),
            );
            self.gl().pixel_store_i32(glow::UNPACK_ALIGNMENT, 4);
            self.gl().bind_texture(glow::TEXTURE_2D, None);
        }
        #[cfg(test)]
        {
            self.glyph_atlas_upload_count = self.glyph_atlas_upload_count.saturating_add(1);
        }
        self.glyph_atlas_cursor.x = x.saturating_add(width).saturating_add(1);
        self.glyph_atlas_cursor.row_h = self.glyph_atlas_cursor.row_h.max(height.saturating_add(1));

        let inverse_width = 1.0 / self.glyph_atlas_width as f32;
        let inverse_height = 1.0 / self.glyph_atlas_height as f32;
        Ok((
            x as f32 * inverse_width,
            y as f32 * inverse_height,
            x.saturating_add(width) as f32 * inverse_width,
            y.saturating_add(height) as f32 * inverse_height,
        ))
    }

    fn push_glyph_quad(
        vertices: &mut Vec<GlyphVertex>,
        glyph: &GpuGlyphBlit,
        (u0, v0, u1, v1): (f32, f32, f32, f32),
    ) {
        let x0 = glyph.x;
        let y0 = glyph.y;
        let x1 = glyph.x + glyph.w;
        let y1 = glyph.y + glyph.h;
        for (pos, uv) in [
            ([x0, y0], [u0, v0]),
            ([x1, y0], [u1, v0]),
            ([x0, y1], [u0, v1]),
            ([x0, y1], [u0, v1]),
            ([x1, y0], [u1, v0]),
            ([x1, y1], [u1, v1]),
        ] {
            vertices.push(GlyphVertex {
                pos,
                uv,
                color: glyph.rgba,
            });
        }
    }

    fn flush_glyph_batch(
        &mut self,
        viewport_width: f32,
        viewport_height: f32,
        scissor: Option<(i32, i32, i32, i32)>,
    ) -> Result<()> {
        if self.glyph_vertices.is_empty() {
            return Ok(());
        }
        let texture = self.glyph_atlas_texture.ok_or_else(|| {
            Error::new(Errc::PlatformError, "OpenGL glyph atlas texture is missing")
        })?;
        self.apply_scissor(scissor);
        unsafe {
            self.gl().enable(glow::BLEND);
            self.gl().blend_func(glow::ONE, glow::ONE_MINUS_SRC_ALPHA);
            self.gl().use_program(Some(self.glyph_program));
            self.gl().uniform_2_f32(
                self.glyph_viewport.as_ref(),
                viewport_width.max(1.0),
                viewport_height.max(1.0),
            );
            self.gl()
                .uniform_1_i32(self.glyph_texture_uniform.as_ref(), 0);
            self.gl().active_texture(glow::TEXTURE0);
            self.gl().bind_texture(glow::TEXTURE_2D, Some(texture));
            self.gl().bind_vertex_array(Some(self.glyph_vao));
            self.gl()
                .bind_buffer(glow::ARRAY_BUFFER, Some(self.glyph_vbo));
            let bytes = std::slice::from_raw_parts(
                self.glyph_vertices.as_ptr().cast::<u8>(),
                std::mem::size_of_val(self.glyph_vertices.as_slice()),
            );
            self.gl()
                .buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::STREAM_DRAW);
            self.gl()
                .draw_arrays(glow::TRIANGLES, 0, self.glyph_vertices.len() as i32);
            self.gl().bind_vertex_array(None);
            self.gl().bind_texture(glow::TEXTURE_2D, None);
        }
        let result = self.check_gl_error("draw_glyphs");
        self.glyph_vertices.clear();
        result
    }

    #[cfg(test)]
    pub(crate) fn glyph_atlas_upload_count(&self) -> usize {
        self.glyph_atlas_upload_count
    }

    #[cfg(test)]
    pub(crate) fn has_soft_texture(&self) -> bool {
        self.soft_texture.is_some()
    }

    pub(crate) fn blit_soft_fallback_tile(
        &mut self,
        pixels: &[u32],
        target_width: i32,
        target_height: i32,
        tile: SoftFallbackTile,
    ) -> Result<()> {
        validate_tile(pixels, target_width, target_height, tile)?;
        self.ensure_soft_texture(target_width, target_height)?;
        unsafe {
            self.gl().disable(glow::SCISSOR_TEST);
            self.gl().active_texture(glow::TEXTURE0);
            self.gl().bind_texture(glow::TEXTURE_2D, self.soft_texture);
            // GLES has no portable unpack-row-length state. The compact
            // API-neutral tile is therefore uploaded one row at a time; its
            // BGRA bytes are swizzled by BLIT_FRAG.
            for row in 0..tile.height {
                let start = row as usize * tile.width as usize;
                let end = start + tile.width as usize;
                let bytes = std::slice::from_raw_parts(
                    pixels[start..end].as_ptr() as *const u8,
                    tile.width as usize * std::mem::size_of::<u32>(),
                );
                self.gl().tex_sub_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    tile.dst_x,
                    tile.dst_y + row,
                    tile.width,
                    1,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(Some(bytes)),
                );
            }
            self.gl().enable(glow::BLEND);
            self.gl()
                // CPU fallback storage is premultiplied AARRGGBB. Its sampled
                // RGB already contains alpha, so SRC_ALPHA would darken the
                // segment a second time.
                .blend_func(glow::ONE, glow::ONE_MINUS_SRC_ALPHA);
            self.gl().use_program(Some(self.blit_bgra_program));
            self.gl().uniform_1_i32(self.blit_bgra_texture.as_ref(), 0);
            self.gl().uniform_4_f32(
                self.blit_bgra_uv.as_ref(),
                tile.dst_x as f32 / target_width as f32,
                tile.dst_y as f32 / target_height as f32,
                tile.width as f32 / target_width as f32,
                tile.height as f32 / target_height as f32,
            );
            self.set_destination_viewport(
                tile.dst_x as f32,
                tile.dst_y as f32,
                tile.width as f32,
                tile.height as f32,
            );
            self.gl().bind_vertex_array(Some(self.blit_vao));
            self.gl().draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
            self.gl().bind_vertex_array(None);
            self.gl().bind_texture(glow::TEXTURE_2D, None);
        }
        self.restore_full_viewport();
        self.check_gl_error("blit_soft_fallback_tile")
    }

    /// Full-target replace upload of premultiplied AARRGGBB pixels. Used by
    /// destination-dependent FrameEncoder ops (Additive / Scroll) after CPU
    /// reference apply — must not alpha-over the previous RT contents.
    pub(crate) fn upload_surface_pixels(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
    ) -> Result<()> {
        let (target_width, target_height) = self.current_target_size();
        if width != target_width || height != target_height {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "OpenGL upload_surface_pixels extent {width}x{height} does not match target {target_width}x{target_height}"
                ),
            ));
        }
        let expected = (width as usize).saturating_mul(height as usize);
        if pixels.len() < expected {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "OpenGL upload_surface_pixels buffer too small, got {}, need {expected}",
                    pixels.len()
                ),
            ));
        }
        let tile = SoftFallbackTile::at_destination(0, 0, width, height);
        self.ensure_soft_texture(width, height)?;
        unsafe {
            self.gl().disable(glow::SCISSOR_TEST);
            self.gl().disable(glow::BLEND);
            self.gl().active_texture(glow::TEXTURE0);
            self.gl().bind_texture(glow::TEXTURE_2D, self.soft_texture);
            for row in 0..tile.height {
                let start = row as usize * tile.width as usize;
                let end = start + tile.width as usize;
                let bytes = std::slice::from_raw_parts(
                    pixels[start..end].as_ptr() as *const u8,
                    tile.width as usize * std::mem::size_of::<u32>(),
                );
                self.gl().tex_sub_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    tile.dst_x,
                    tile.dst_y + row,
                    tile.width,
                    1,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(Some(bytes)),
                );
            }
            self.gl().use_program(Some(self.blit_bgra_program));
            self.gl().uniform_1_i32(self.blit_bgra_texture.as_ref(), 0);
            self.gl()
                .uniform_4_f32(self.blit_bgra_uv.as_ref(), 0.0, 0.0, 1.0, 1.0);
            self.set_destination_viewport(0.0, 0.0, width as f32, height as f32);
            self.gl().bind_vertex_array(Some(self.blit_vao));
            self.gl().draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
            self.gl().bind_vertex_array(None);
            self.gl().bind_texture(glow::TEXTURE_2D, None);
            self.gl().enable(glow::BLEND);
            self.gl().blend_func(glow::ONE, glow::ONE_MINUS_SRC_ALPHA);
        }
        self.restore_full_viewport();
        self.check_gl_error("upload_surface_pixels")
    }

    pub(crate) fn create_offscreen_target(
        &mut self,
        width: i32,
        height: i32,
    ) -> Result<OffscreenTargetId> {
        if width <= 0 || height <= 0 {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!("OpenGL offscreen extent must be positive, got {width}x{height}"),
            ));
        }
        let gl = self.gl();
        let texture = unsafe { create_texture(gl, width, height)? };
        let framebuffer = match unsafe { gl.create_framebuffer() } {
            Ok(framebuffer) => framebuffer,
            Err(error) => {
                unsafe { gl.delete_texture(texture) };
                return Err(gl_error("create_framebuffer", error));
            }
        };
        unsafe {
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(texture),
                0,
            );
        }
        let complete =
            unsafe { gl.check_framebuffer_status(glow::FRAMEBUFFER) } == glow::FRAMEBUFFER_COMPLETE;
        self.bind_current_framebuffer();
        if !complete {
            unsafe {
                gl.delete_framebuffer(framebuffer);
                gl.delete_texture(texture);
            }
            return Err(Error::new(
                Errc::PlatformError,
                "OpenGL offscreen framebuffer is incomplete",
            ));
        }
        let id = self.free_offscreen_ids.pop().unwrap_or_else(|| {
            let id = self.next_offscreen_id;
            self.next_offscreen_id = self.next_offscreen_id.saturating_add(1);
            id
        });
        while self.offscreens.len() <= id as usize {
            self.offscreens.push(None);
        }
        self.offscreens[id as usize] = Some(OffscreenTarget {
            framebuffer,
            texture,
            width,
            height,
        });
        Ok(OffscreenTargetId(id))
    }

    pub(crate) fn destroy_offscreen_target(&mut self, id: OffscreenTargetId) -> Result<()> {
        let index = id.0 as usize;
        let target = self
            .offscreens
            .get_mut(index)
            .and_then(Option::take)
            .ok_or_else(|| {
                Error::new(Errc::InvalidState, "OpenGL offscreen target does not exist")
            })?;
        if self.current.framebuffer == Some(target.framebuffer) {
            self.current = self.swapchain;
            self.bind_current_framebuffer();
            self.restore_full_viewport();
        }
        unsafe {
            self.gl().delete_framebuffer(target.framebuffer);
            self.gl().delete_texture(target.texture);
        }
        self.free_offscreen_ids.push(id.0);
        Ok(())
    }

    pub(crate) fn bind_offscreen_target(&mut self, id: OffscreenTargetId) -> Result<()> {
        let target = self
            .offscreens
            .get(id.0 as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| {
                Error::new(Errc::InvalidState, "OpenGL offscreen target does not exist")
            })?;
        self.current = TargetState::offscreen(target.framebuffer, target.width, target.height);
        self.bind_current_framebuffer();
        self.restore_full_viewport();
        self.check_gl_error("bind_offscreen_target")
    }

    pub(crate) fn bind_swapchain_target(&mut self) {
        self.current = self.swapchain;
        self.bind_current_framebuffer();
        self.restore_full_viewport();
    }

    pub(crate) fn blit_offscreen_target(
        &mut self,
        id: OffscreenTargetId,
        src: Rect,
        dst: Rect,
    ) -> Result<()> {
        let source = self
            .offscreens
            .get(id.0 as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| {
                Error::new(Errc::InvalidState, "OpenGL offscreen target does not exist")
            })?;
        if self.current.framebuffer == Some(source.framebuffer) {
            return Err(Error::new(
                Errc::InvalidArgument,
                "OpenGL offscreen target cannot blit into itself",
            ));
        }
        if dst.w <= 0.0 || dst.h <= 0.0 {
            return Ok(());
        }
        let max_x = source.width as f32;
        let max_y = source.height as f32;
        let src_x = src.x.clamp(0.0, max_x);
        let src_y = src.y.clamp(0.0, max_y);
        let src_end_x = (src.x + src.w).clamp(src_x, max_x);
        let src_end_y = (src.y + src.h).clamp(src_y, max_y);
        if src_end_x <= src_x || src_end_y <= src_y {
            return Ok(());
        }
        // The source can be sampled while a different Picture FBO is the
        // destination. Rebind the tracked destination explicitly: callers
        // may have submitted the destination segment immediately before this
        // ordered boundary, and texture sampling must never inherit a stale
        // framebuffer binding from that submission.
        self.bind_current_framebuffer();
        self.restore_full_viewport();
        unsafe {
            self.gl().disable(glow::SCISSOR_TEST);
            self.gl().active_texture(glow::TEXTURE0);
            self.gl()
                .bind_texture(glow::TEXTURE_2D, Some(source.texture));
            self.gl().enable(glow::BLEND);
            self.gl()
                .blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            self.gl().use_program(Some(self.blit_rgba_program));
            self.gl().uniform_1_i32(self.blit_rgba_texture.as_ref(), 0);
            self.gl().uniform_4_f32(
                self.blit_rgba_uv.as_ref(),
                src_x / max_x,
                src_y / max_y,
                (src_end_x - src_x) / max_x,
                (src_end_y - src_y) / max_y,
            );
            self.set_destination_viewport(dst.x, dst.y, dst.w, dst.h);
            self.gl().bind_vertex_array(Some(self.blit_vao));
            self.gl().draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
            self.gl().bind_vertex_array(None);
            self.gl().bind_texture(glow::TEXTURE_2D, None);
        }
        self.restore_full_viewport();
        self.check_gl_error("blit_offscreen_target")
    }

    pub(crate) fn read_pixels(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<Vec<u32>> {
        if width <= 0 || height <= 0 {
            return Ok(Vec::new());
        }
        let length = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| Error::new(Errc::InvalidArgument, "OpenGL readback extent overflows"))?;
        let mut pixels = vec![0u32; length];
        unsafe {
            self.gl().read_pixels(
                x,
                y,
                width,
                height,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(std::slice::from_raw_parts_mut(
                    pixels.as_mut_ptr() as *mut u8,
                    length * std::mem::size_of::<u32>(),
                ))),
            );
            let error = self.gl().get_error();
            if error != glow::NO_ERROR {
                return Err(Error::new(
                    Errc::PlatformError,
                    format!("OpenGL read_pixels failed with error {error:#X}"),
                ));
            }
        }
        Ok(pixels)
    }

    pub(crate) fn release(&mut self) {
        if self.released {
            return;
        }
        self.released = true;
        self.bind_swapchain_target();
        let offscreens = std::mem::take(&mut self.offscreens);
        for target in offscreens.into_iter().flatten() {
            unsafe {
                self.gl().delete_framebuffer(target.framebuffer);
                self.gl().delete_texture(target.texture);
            }
        }
        unsafe {
            self.gl().delete_vertex_array(self.rect_vao);
            self.gl().delete_buffer(self.rect_vbo);
            self.gl().delete_program(self.rect_program);
            self.gl().delete_vertex_array(self.glyph_vao);
            self.gl().delete_buffer(self.glyph_vbo);
            self.gl().delete_program(self.glyph_program);
            if let Some(texture) = self.glyph_atlas_texture.take() {
                self.gl().delete_texture(texture);
            }
            self.gl().delete_vertex_array(self.blit_vao);
            self.gl().delete_buffer(self.blit_vbo);
            self.gl().delete_program(self.blit_bgra_program);
            self.gl().delete_program(self.blit_rgba_program);
            if let Some(texture) = self.soft_texture.take() {
                self.gl().delete_texture(texture);
            }
        }
        self.glyph_atlas_cache.clear();
        self.glyph_vertices.clear();
    }

    fn ensure_soft_texture(&mut self, width: i32, height: i32) -> Result<()> {
        if self.soft_texture.is_some() && self.soft_width == width && self.soft_height == height {
            return Ok(());
        }
        let texture = unsafe { create_texture(self.gl(), width, height)? };
        if let Some(previous) = self.soft_texture.replace(texture) {
            unsafe { self.gl().delete_texture(previous) };
        }
        self.soft_width = width;
        self.soft_height = height;
        Ok(())
    }

    fn bind_current_framebuffer(&self) {
        unsafe {
            self.gl()
                .bind_framebuffer(glow::FRAMEBUFFER, self.current.framebuffer);
        }
    }

    fn restore_full_viewport(&self) {
        unsafe {
            self.gl().viewport(
                0,
                0,
                self.current.drawable_width,
                self.current.drawable_height,
            );
            self.gl().disable(glow::SCISSOR_TEST);
        }
    }

    fn apply_scissor(&self, scissor: Option<(i32, i32, i32, i32)>) {
        let Some(scissor) = scissor else {
            unsafe { self.gl().disable(glow::SCISSOR_TEST) };
            return;
        };
        let (x, y, width, height) = logical_scissor_to_drawable(self.current, scissor);
        unsafe {
            self.gl().enable(glow::SCISSOR_TEST);
            self.gl().scissor(x, y, width, height);
        }
    }

    fn set_destination_viewport(&self, x: f32, y: f32, width: f32, height: f32) {
        let dpr = self.current.dpr.max(1.0);
        unsafe {
            self.gl().viewport(
                (x * dpr).floor() as i32,
                ((self.current.logical_height as f32 - y - height).max(0.0) * dpr).floor() as i32,
                (width * dpr).ceil().max(1.0) as i32,
                (height * dpr).ceil().max(1.0) as i32,
            );
        }
    }
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
