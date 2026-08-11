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
        // 先释放 FramePlan/RHI 资源，再恢复默认 framebuffer。
        self.rhi_release();
        self.bind_swapchain_target();
    }

    pub(super) fn bind_current_framebuffer(&self) {
        unsafe {
            self.gl()
                .bind_framebuffer(glow::FRAMEBUFFER, self.current.framebuffer);
        }
    }

    pub(super) fn restore_full_viewport(&self) {
        // viewport 始终使用 GL 自身坐标系；行序差异已由 shader 编译期处理。
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
}
