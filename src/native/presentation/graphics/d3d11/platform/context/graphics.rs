use super::*;

impl IGraphicsContext for D3d11Context {
    fn caps(&self) -> crate::native::present::GraphicsContextCaps {
        GraphicsContextCaps::gpu_native_swapchain(
            GraphicsApi::D3d11,
            PresentCoherency::FullOnly,
            self.device_pixel_ratio(),
        )
        .with_present_occlusion(PresentOcclusionSupport::PresentStatusAndTest)
    }

    // 暴露同一 owner-thread context 上的薄 RHI device/surface 组合视图。
    fn rhi_context(&mut self) -> Option<&mut dyn crate::native::present::rhi::GraphicsContextRhi> {
        // D3D11 是当前参考 adapter；其他 backend 继续走兼容接口。
        Some(self)
    }

    fn graphics_backend(&self) -> GraphicsApi {
        GraphicsApi::D3d11
    }

    fn native_raster_caps(&self) -> NativeRasterCaps {
        NativeRasterCaps::d3d11_full()
    }

    fn initialize(&mut self, _native_window: *mut c_void, _width: i32, _height: i32) -> Result<()> {
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<()> {
        self.resize_surface_logical(width, height)
    }

    // 把 D3D11 当前 drawable 元数据提供给兼容 present 边界。
    fn present_surface(&self) -> crate::native::present::PresentSurface {
        // 使用同一代际值保证旧 damage 不会跨 swapchain 重建复用。
        crate::native::present::PresentSurface::identity(
            self.width,
            self.height,
            self.device_pixel_ratio(),
            self.surface_generation,
        )
    }

    fn make_current(&mut self) -> Result<()> {
        self.bind_current_draw_target()
    }

    fn swap_buffers(&mut self, _damage: PresentDamage) -> Result<()> {
        self.present_result()
    }

    fn test_present(&mut self) -> Result<PresentTestResult> {
        // DXGI_PRESENT_TEST is the documented exit probe for an already-idle
        // bitblt swapchain. It submits no frame data and must use sync 0.
        map_dxgi_present_test_result(unsafe { self.swap_chain.Present(0, DXGI_PRESENT_TEST) })
    }

    fn try_shutdown(&mut self) -> Result<()> {
        self.shutdown_result()
    }

    fn read_pixels(&mut self, x: i32, y: i32, width: i32, height: i32) -> Result<Vec<u32>> {
        let x0 = x.clamp(0, self.width);
        let y0 = y.clamp(0, self.height);
        let x1 = x.saturating_add(width).clamp(x0, self.width);
        let y1 = y.saturating_add(height).clamp(y0, self.height);
        let read_w = x1 - x0;
        let read_h = y1 - y0;
        if read_w <= 0 || read_h <= 0 {
            return Ok(Vec::new());
        }

        (|| -> Result<Vec<u32>> {
            let back_buffer: ID3D11Texture2D = unsafe {
                self.swap_chain
                    .GetBuffer(0)
                    .map_err(|err| d3d_error("IDXGISwapChain::GetBuffer(read_pixels)", err))?
            };
            let desc = D3D11_TEXTURE2D_DESC {
                Width: self.width as u32,
                Height: self.height as u32,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
            };
            let mut staging = None;
            unsafe {
                self.device
                    .CreateTexture2D(&desc, None, Some(&mut staging))
                    .map_err(|err| d3d_error("ID3D11Device::CreateTexture2D(read_pixels)", err))?;
            }
            let staging = staging.ok_or_else(|| {
                Error::new(
                    Errc::PlatformError,
                    "D3d11Context: read_pixels staging texture was not created",
                )
            })?;
            unsafe {
                self.context.CopyResource(&staging, &back_buffer);
            }
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            unsafe {
                self.context
                    .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                    .map_err(|err| d3d_error("ID3D11DeviceContext::Map(read_pixels)", err))?;
            }
            let mut pixels = vec![0u32; (read_w as usize).saturating_mul(read_h as usize)];
            for row in 0..read_h as usize {
                let src = unsafe {
                    mapped
                        .pData
                        .cast::<u8>()
                        .add((y0 as usize + row) * mapped.RowPitch as usize + x0 as usize * 4)
                        .cast::<u32>()
                };
                let dst = pixels[row * read_w as usize..].as_mut_ptr();
                unsafe {
                    std::ptr::copy_nonoverlapping(src, dst, read_w as usize);
                }
            }
            unsafe {
                self.context.Unmap(&staging, 0);
            }
            Ok(pixels)
        })()
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
        self.bind_current_draw_target()?;
        if let Some(id) = self.bound_offscreen {
            let Some(Some(target)) = self.offscreens.get(id as usize) else {
                return Err(Error::new(
                    Errc::InvalidArgument,
                    format!("D3d11Context: clear_render_target unknown offscreen {id}"),
                ));
            };
            unsafe {
                self.context
                    .ClearRenderTargetView(&target.rtv, &[r, g, b, a]);
            }
            return Ok(());
        }
        let Some(rtv) = self.rtv.as_ref() else {
            return Err(Error::new(
                Errc::PlatformError,
                "D3d11Context: clear_render_target without RTV",
            ));
        };
        unsafe {
            self.context.ClearRenderTargetView(rtv, &[r, g, b, a]);
        }
        Ok(())
    }

    fn upload_surface_pixels(&mut self, pixels: &[u32], width: i32, height: i32) -> Result<()> {
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        if width != self.width || height != self.height {
            self.resize(width, height)?;
        }
        let expected = (width as usize).saturating_mul(height as usize);
        if pixels.len() < expected {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "D3d11Context: pixel buffer too small, got {}, need {expected}",
                    pixels.len()
                ),
            ));
        }
        self.ensure_rtv()?;
        let back_buffer: ID3D11Texture2D = unsafe {
            self.swap_chain
                .GetBuffer(0)
                .map_err(|err| d3d_error("IDXGISwapChain::GetBuffer", err))?
        };
        unsafe {
            self.context.UpdateSubresource(
                &back_buffer,
                0,
                None,
                pixels.as_ptr().cast(),
                (width as u32) * 4,
                0,
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
        self.ensure_rtv()?;
        self.make_current()?;
        self.pipeline
            .draw_solid_rects(&self.context, viewport_w, viewport_h, scissor, rects)
    }

    fn draw_stroke_rects(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuStrokeRect],
    ) -> Result<()> {
        self.ensure_rtv()?;
        self.make_current()?;
        self.pipeline
            .draw_stroke_rects(&self.context, viewport_w, viewport_h, scissor, rects)
    }

    fn draw_glyphs(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        glyphs: &[GpuGlyphBlit],
    ) -> Result<()> {
        self.ensure_rtv()?;
        self.make_current()?;
        self.pipeline.draw_glyphs(
            &self.device,
            &self.context,
            viewport_w,
            viewport_h,
            scissor,
            glyphs,
        )
    }

    fn draw_linear_gradients(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuLinearGradientRect],
    ) -> Result<()> {
        self.ensure_rtv()?;
        self.make_current()?;
        self.pipeline
            .draw_linear_gradients(&self.context, viewport_w, viewport_h, scissor, rects)
    }

    fn draw_radial_gradients(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        grads: &[GpuRadialGradient],
    ) -> Result<()> {
        self.ensure_rtv()?;
        self.make_current()?;
        self.pipeline
            .draw_radial_gradients(&self.context, viewport_w, viewport_h, scissor, grads)
    }

    // 将兼容层 sector batch 转交 D3D11 原生 pipeline。
    fn draw_sectors(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        sectors: &[GpuSector],
    ) -> Result<()> {
        // 先确保当前 render target 与 owner-thread context 有效。
        self.ensure_rtv()?;
        // 绑定当前 D3D11 context，统一处理 surface 重建边界。
        self.make_current()?;
        // 使用与薄 RHI 相同的 sector shader 和 blend ABI。
        self.pipeline
            .draw_sectors(&self.context, viewport_w, viewport_h, scissor, sectors)
    }

    // 将兼容层 BGRA 图片 affine batch 转交 D3D11 原生 pipeline。
    fn draw_image_blits(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        blits: &[GpuImageBlit],
    ) -> Result<()> {
        // 先确保当前 render target 与 owner-thread context 有效。
        self.ensure_rtv()?;
        // 绑定当前 D3D11 context，统一处理 surface 重建边界。
        self.make_current()?;
        // 使用兼容图片 owner 执行 payload 上传与 affine draw。
        self.pipeline.draw_image_blits(
            &self.device,
            &self.context,
            viewport_w,
            viewport_h,
            scissor,
            blits,
        )
    }

    fn draw_solid_meshes(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        meshes: &[GpuSolidMesh],
    ) -> Result<()> {
        self.ensure_rtv()?;
        self.make_current()?;
        self.pipeline.draw_solid_meshes(
            &self.device,
            &self.context,
            viewport_w,
            viewport_h,
            scissor,
            meshes,
        )
    }

    fn draw_box_shadows(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        shadows: &[GpuBoxShadow],
    ) -> Result<()> {
        self.ensure_rtv()?;
        self.make_current()?;
        self.pipeline
            .draw_box_shadows(&self.context, viewport_w, viewport_h, scissor, shadows)
    }

    fn blit_soft_fallback(&mut self, pixels: &[u32], width: i32, height: i32) -> Result<()> {
        self.ensure_rtv()?;
        self.make_current()?;
        self.pipeline
            .blit_soft_fallback(&self.device, &self.context, pixels, width, height)
    }

    fn blit_soft_fallback_tile(&mut self, pixels: &[u32], tile: SoftFallbackTile) -> Result<()> {
        self.bind_current_draw_target()?;
        self.make_current()?;
        let (target_width, target_height) = self.current_target_size();
        self.pipeline.blit_soft_fallback_tile(
            &self.device,
            &self.context,
            pixels,
            target_width,
            target_height,
            tile,
        )
    }

    fn clear_rects(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        rects: &[GpuSolidRect],
    ) -> Result<()> {
        self.bind_current_draw_target()?;
        self.pipeline
            .clear_rects(&self.context, viewport_w, viewport_h, rects)
    }

    fn create_offscreen_target(
        &mut self,
        width: i32,
        height: i32,
    ) -> Result<OffscreenTargetId, Error> {
        let w = width.max(1);
        let h = height.max(1);
        let desc = D3D11_TEXTURE2D_DESC {
            Width: w as u32,
            Height: h as u32,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let mut texture = None;
        unsafe {
            self.device
                .CreateTexture2D(&desc, None, Some(&mut texture))
                .map_err(|err| d3d_error("CreateTexture2D(offscreen)", err))?;
        }
        let texture = texture.ok_or_else(|| {
            Error::new(
                Errc::PlatformError,
                "D3d11Context: CreateTexture2D(offscreen) returned no texture",
            )
        })?;
        let mut rtv = None;
        unsafe {
            self.device
                .CreateRenderTargetView(&texture, None, Some(&mut rtv))
                .map_err(|err| d3d_error("CreateRenderTargetView(offscreen)", err))?;
        }
        let rtv = rtv.ok_or_else(|| {
            Error::new(
                Errc::PlatformError,
                "D3d11Context: CreateRenderTargetView(offscreen) returned no RTV",
            )
        })?;
        let mut srv = None;
        unsafe {
            self.device
                .CreateShaderResourceView(&texture, None, Some(&mut srv))
                .map_err(|err| d3d_error("CreateShaderResourceView(offscreen)", err))?;
        }
        let srv = srv.ok_or_else(|| {
            Error::new(
                Errc::PlatformError,
                "D3d11Context: CreateShaderResourceView(offscreen) returned no SRV",
            )
        })?;

        let id = if let Some(id) = self.free_offscreen_ids.pop() {
            id
        } else {
            let id = self.next_offscreen_id;
            self.next_offscreen_id = self.next_offscreen_id.saturating_add(1);
            id
        };
        let idx = id as usize;
        while self.offscreens.len() <= idx {
            self.offscreens.push(None);
        }
        self.offscreens[idx] = Some(OffscreenTarget {
            texture,
            rtv,
            srv,
            width: w,
            height: h,
        });
        Ok(OffscreenTargetId(id))
    }

    fn try_destroy_offscreen_target(&mut self, id: OffscreenTargetId) -> Result<(), Error> {
        // Do this for every checked destroy, not only when our cached marker
        // says the target is bound: a failed previous restore leaves the real
        // D3D target unknown. Do not release the offscreen slot until the
        // swapchain target is confirmed, so callers can retry safely.
        self.bind_swapchain_target()?;
        let idx = id.0 as usize;
        if idx < self.offscreens.len() && self.offscreens[idx].take().is_some() {
            self.free_offscreen_ids.push(id.0);
        }
        Ok(())
    }

    fn destroy_offscreen_target(&mut self, id: OffscreenTargetId) {
        if let Err(error) = self.try_destroy_offscreen_target(id) {
            tracing::error!(
                "D3d11Context: destroy offscreen target failed: {}",
                error.short_what()
            );
        }
    }

    fn bind_offscreen_target(&mut self, id: OffscreenTargetId) -> Result<(), Error> {
        let idx = id.0 as usize;
        if self.offscreens.get(idx).and_then(|o| o.as_ref()).is_none() {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!("D3d11Context: bind_offscreen_target unknown id {}", id.0),
            ));
        }
        self.bound_offscreen = Some(id.0);
        self.bind_current_draw_target()
    }

    fn bind_swapchain_target(&mut self) -> Result<(), Error> {
        self.bound_offscreen = None;
        self.bind_current_draw_target()
    }

    fn blit_offscreen_target(
        &mut self,
        id: OffscreenTargetId,
        src: crate::core::Rect,
        dst: crate::core::Rect,
        opacity: f32,
        additive: bool,
    ) -> Result<(), Error> {
        if !opacity.is_finite() || opacity <= 0.0 {
            return Ok(());
        }
        let idx = id.0 as usize;
        let Some(Some(target)) = self.offscreens.get(idx) else {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!("D3d11Context: blit_offscreen_target unknown id {}", id.0),
            ));
        };
        if self.bound_offscreen == Some(id.0) {
            return Err(Error::new(
                Errc::InvalidState,
                "D3d11Context: cannot blit offscreen while it is the bound RT",
            ));
        }
        let srv = target.srv.clone();
        let source_w = target.width as f32;
        let source_h = target.height as f32;
        let (tw, th) = self.current_target_size();
        self.bind_current_draw_target()?;
        self.pipeline.blit_srv_to_rect(
            &self.context,
            &srv,
            source_w,
            source_h,
            tw as f32,
            th as f32,
            src,
            dst,
            opacity,
            additive,
        )
    }

    fn present(&mut self, frame: &PresentFrame<'_>) -> Result<()> {
        match frame {
            PresentFrame::Swapchain { .. } => {
                self.bind_swapchain_target()?;
                self.present_result()
            }
            PresentFrame::PixelBuffer {
                pixels,
                width,
                height,
                damage,
            } => self.present_pixels(pixels, *width, *height, damage.clone()),
        }
    }

    /// Kept for low-level tests; not advertised via caps (`PresentMode::Swapchain`).
    fn present_pixels(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        _damage: PresentDamage,
    ) -> Result<()> {
        self.upload_surface_pixels(pixels, width, height)?;
        self.present_result()
    }
}
