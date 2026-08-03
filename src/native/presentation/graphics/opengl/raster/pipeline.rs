use super::*;

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

    pub(super) fn gl(&self) -> &glow::Context {
        self.runtime.context()
    }

    pub(super) fn check_gl_error(&self, operation: &str) -> Result<()> {
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

    pub(super) fn ensure_glyph_atlas(&mut self, needed_width: u32, needed_height: u32) -> Result<()> {
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

    pub(super) fn reset_glyph_atlas(&mut self) {
        self.glyph_atlas_cursor = GlyphAtlasCursor::default();
        self.glyph_atlas_cache.clear();
    }

    pub(super) fn glyph_atlas_can_fit(&self, width: u32, height: u32) -> bool {
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

    pub(super) fn pack_glyph(
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

    pub(super) fn push_glyph_quad(
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

    pub(super) fn flush_glyph_batch(
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
