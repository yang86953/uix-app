use super::*;

impl IGraphicsContext for D3d12Context {
    fn caps(&self) -> GraphicsContextCaps {
        GraphicsContextCaps::gpu_native_swapchain(
            GraphicsBackend::D3d12,
            PresentCoherency::FullOnly,
            self.device_pixel_ratio(),
        )
    }

    fn native_raster_caps(&self) -> NativeRasterCaps {
        NativeRasterCaps {
            clear_target: true,
            soft_blit: true,
            solid_rects: true,
            glyphs: true,
            ..NativeRasterCaps::default()
        }
    }

    fn initialize(&mut self, _native_window: *mut c_void, _width: i32, _height: i32) -> Result<()> {
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<()> {
        self.resize_result(width, height)
    }

    fn make_current(&mut self) -> Result<()> {
        self.begin_commands()
    }

    fn swap_buffers(&mut self, _damage: PresentDamage) -> Result<()> {
        self.present_result()
    }

    fn present(&mut self, frame: &PresentFrame<'_>) -> Result<()> {
        match frame {
            PresentFrame::Swapchain { .. } => self.present_result(),
            PresentFrame::PixelBuffer { .. } => Err(Error::new(
                Errc::InvalidArgument,
                "D3d12Context: PixelBuffer present is not supported",
            )),
        }
    }

    fn try_shutdown(&mut self) -> Result<()> {
        self.shutdown_result()
    }

    fn read_pixels(&mut self, x: i32, y: i32, width: i32, height: i32) -> Result<Vec<u32>> {
        self.read_pixels_result(x, y, width, height)
    }

    fn width(&self) -> i32 {
        self.width
    }

    fn height(&self) -> i32 {
        self.height
    }

    fn device_pixel_ratio(&self) -> f32 {
        self.width as f32 / self.logical_width.max(1) as f32
    }

    fn clear_render_target(&mut self, r: f32, g: f32, b: f32, a: f32) -> Result<()> {
        self.begin_commands()?;
        unsafe {
            self.command_list.ClearRenderTargetView(
                self.rtv_handle(self.frame_index),
                &[r, g, b, a],
                None,
            );
        }
        Ok(())
    }

    fn draw_solid_rects(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuSolidRect],
    ) -> Result<()> {
        if rects.is_empty() {
            return Ok(());
        }
        self.begin_commands()?;
        self.pipeline
            .as_ref()
            .ok_or_else(|| platform_error("D3d12Context: raster pipeline is shut down"))?
            .draw_solid_rects(&self.command_list, viewport_w, viewport_h, scissor, rects)
    }

    fn draw_glyphs(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        glyphs: &[GpuGlyphBlit],
    ) -> Result<()> {
        if glyphs.is_empty() {
            return Ok(());
        }
        self.begin_commands()?;
        self.pipeline
            .as_mut()
            .ok_or_else(|| platform_error("D3d12Context: raster pipeline is shut down"))?
            .draw_glyphs(
                &self.device,
                &self.command_list,
                self.frame_index,
                viewport_w,
                viewport_h,
                scissor,
                glyphs,
            )
    }

    fn blit_soft_fallback(&mut self, pixels: &[u32], width: i32, height: i32) -> Result<()> {
        self.ensure_healthy()?;
        if width != self.width || height != self.height {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "D3d12Context: soft blit dimensions {width}x{height} do not match drawable {}x{}",
                    self.width, self.height
                ),
            ));
        }
        let expected = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| Error::new(Errc::InvalidArgument, "soft blit pixel count overflow"))?;
        if pixels.len() < expected {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "D3d12Context: soft blit buffer too small, got {}, need {expected}",
                    pixels.len()
                ),
            ));
        }
        self.begin_commands()?;
        self.pipeline
            .as_mut()
            .ok_or_else(|| platform_error("D3d12Context: raster pipeline is shut down"))?
            .blit_soft_fallback(
                &self.device,
                &self.command_list,
                self.frame_index,
                pixels,
                width,
                height,
            )
    }

    /// Full-target replace upload of premultiplied AARRGGBB pixels. Used by
    /// destination-dependent FrameEncoder ops after CPU reference apply — must
    /// not alpha-over the previous RT contents.
    fn upload_surface_pixels(&mut self, pixels: &[u32], width: i32, height: i32) -> Result<()> {
        self.ensure_healthy()?;
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        if width != self.width || height != self.height {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "D3d12Context: upload_surface_pixels {width}x{height} does not match drawable {}x{}",
                    self.width, self.height
                ),
            ));
        }
        let expected = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| {
                Error::new(
                    Errc::InvalidArgument,
                    "D3d12Context: upload_surface_pixels pixel count overflow",
                )
            })?;
        if pixels.len() < expected {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "D3d12Context: upload_surface_pixels buffer too small, got {}, need {expected}",
                    pixels.len()
                ),
            ));
        }
        if self.frame_index >= self.back_buffers.len() {
            return Err(platform_error(format!(
                "D3d12Context: upload frame index {} has only {} buffers",
                self.frame_index,
                self.back_buffers.len()
            )));
        }
        let buffer = self.back_buffers[self.frame_index].clone();
        let desc = unsafe { buffer.GetDesc() };
        let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
        let mut total_bytes = 0u64;
        unsafe {
            self.device.GetCopyableFootprints(
                &desc,
                0,
                1,
                0,
                Some(&mut footprint),
                None,
                None,
                Some(&mut total_bytes),
            );
        }
        let upload = create_upload_buffer(&self.device, total_bytes)?;
        let empty_read = D3D12_RANGE { Begin: 0, End: 0 };
        let mut mapped = std::ptr::null_mut();
        unsafe { upload.Map(0, Some(&empty_read), Some(&mut mapped)) }
            .map_err(|error| d3d12_error("ID3D12Resource::Map(upload)", error))?;
        if mapped.is_null() {
            unsafe { upload.Unmap(0, None) };
            return Err(platform_error("D3d12Context: upload Map returned null"));
        }
        let row_bytes = width as usize * std::mem::size_of::<u32>();
        let row_pitch = footprint.Footprint.RowPitch as usize;
        let offset = footprint.Offset as usize;
        for row in 0..height as usize {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    pixels.as_ptr().add(row * width as usize).cast::<u8>(),
                    mapped.cast::<u8>().add(offset + row * row_pitch),
                    row_bytes,
                );
            }
        }
        let written = D3D12_RANGE {
            Begin: 0,
            End: total_bytes as usize,
        };
        unsafe { upload.Unmap(0, Some(&written)) };

        self.begin_commands()?;
        let previous_state = self.back_buffer_states[self.frame_index];
        record_transition(
            &self.command_list,
            &buffer,
            previous_state,
            D3D12_RESOURCE_STATE_COPY_DEST,
        );
        self.back_buffer_states[self.frame_index] = D3D12_RESOURCE_STATE_COPY_DEST;
        let mut destination = texture_copy_location_subresource(&buffer);
        let mut source = texture_copy_location_footprint(&upload, footprint);
        unsafe {
            self.command_list
                .CopyTextureRegion(&destination, 0, 0, 0, &source, None);
        }
        release_copy_location(&mut destination);
        release_copy_location(&mut source);
        record_transition(
            &self.command_list,
            &buffer,
            D3D12_RESOURCE_STATE_COPY_DEST,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
        );
        self.back_buffer_states[self.frame_index] = D3D12_RESOURCE_STATE_RENDER_TARGET;
        self.pending_gpu_resources.push(upload);
        self.execute_recording_and_wait()?;
        self.pending_gpu_resources.pop();
        Ok(())
    }

    fn blit_soft_fallback_tile(&mut self, pixels: &[u32], tile: SoftFallbackTile) -> Result<()> {
        self.ensure_healthy()?;
        self.begin_commands()?;
        self.pipeline
            .as_mut()
            .ok_or_else(|| platform_error("D3d12Context: raster pipeline is shut down"))?
            .blit_soft_fallback_tile(
                &self.device,
                &self.command_list,
                self.frame_index,
                pixels,
                self.width,
                self.height,
                tile,
            )
    }
}

impl Drop for D3d12Context {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown_result() {
            tracing::error!(
                "D3d12Context: undrained Drop retained GPU COM objects: {}",
                error.short_what()
            );
            self.retain_gpu_objects_after_undrained_drop();
        }
    }
}
