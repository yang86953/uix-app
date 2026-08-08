use super::*;

// 复用主 raster 类型承载状态恢复与 readback 操作。
impl OpenGlRasterPipeline {
    pub(crate) fn bind_swapchain_target(&mut self) {
        self.current = self.swapchain;
        self.bind_current_framebuffer();
        self.restore_full_viewport();
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
        // 先释放 FramePlan/RHI 资源，再释放 legacy raster 对象。
        self.rhi_release();
        self.bind_swapchain_target();
        unsafe {
            self.gl().delete_vertex_array(self.rect_vao);
            self.gl().delete_buffer(self.rect_vbo);
            self.gl().delete_program(self.rect_program);
            // 释放 legacy queue 复用的仿射 shadow program。
            self.gl().delete_program(self.shadow_program);
            // 释放 legacy queue 的渐变、mesh、sector、图片资源。
            let gl = self.runtime.context();
            self.legacy.release(gl);
            self.gl().delete_vertex_array(self.glyph_vao);
            self.gl().delete_buffer(self.glyph_vbo);
            self.gl().delete_program(self.glyph_program);
            if let Some(texture) = self.glyph_atlas_texture.take() {
                self.gl().delete_texture(texture);
            }
        }
        self.glyph_atlas_cache.clear();
        self.glyph_vertices.clear();
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
}
