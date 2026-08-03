use super::*;

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
        opacity: f32,
    ) -> Result<()> {
        if !opacity.is_finite() || opacity <= 0.0 {
            return Ok(());
        }
        if opacity < 1.0 - 1e-6 {
            return Err(Error::new(
                Errc::NotImplemented,
                "OpenGL blit_offscreen_target opacity < 1 requires wgpu",
            ));
        }
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

    pub(super) fn ensure_soft_texture(&mut self, width: i32, height: i32) -> Result<()> {
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

    pub(super) fn bind_current_framebuffer(&self) {
        unsafe {
            self.gl()
                .bind_framebuffer(glow::FRAMEBUFFER, self.current.framebuffer);
        }
    }

    pub(super) fn restore_full_viewport(&self) {
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

    pub(super) fn apply_scissor(&self, scissor: Option<(i32, i32, i32, i32)>) {
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

    pub(super) fn set_destination_viewport(&self, x: f32, y: f32, width: f32, height: f32) {
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

