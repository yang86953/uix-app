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

    fn bind_swapchain_target(&mut self) -> Result<(), Error> {
        self.bind_current_draw_target()
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
}
