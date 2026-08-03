use super::*;

impl D3d11Context {
    pub(crate) fn glyph_atlas_upload_count(&self) -> usize {
        self.pipeline.glyph_atlas_upload_count()
    }
}

impl D3d11Context {
    pub(crate) fn new(native_window: *mut c_void, width: i32, height: i32) -> Result<Self> {
        if native_window.is_null() {
            return Err(Error::new(
                Errc::PlatformError,
                "D3d11Context: native window handle is null",
            ));
        }
        let drawable = win_surface::drawable_size(native_window, width, height);
        match create_with_drawable(
            native_window,
            drawable,
            &D3D11_FEATURE_LEVELS,
            D3d11DriverKind::Hardware,
        ) {
            Ok(context) => Ok(context),
            Err(hardware_error) => {
                tracing::warn!(
                    "D3d11Context: hardware device unavailable; retrying with WARP: {}",
                    hardware_error.what()
                );
                create_with_drawable(
                    native_window,
                    drawable,
                    &D3D11_FEATURE_LEVELS,
                    D3d11DriverKind::Warp,
                )
                .map_err(|warp_error| {
                    Error::new(
                        Errc::PlatformError,
                        format!(
                            "D3d11Context: hardware and WARP creation failed; hardware=[{}]; warp=[{}]",
                            hardware_error.what(),
                            warp_error.what()
                        ),
                    )
                })
            }
        }
    }

    /// Deterministic WARP-only constructor for crate tests.
    ///
    /// Production construction deliberately keeps the hardware-then-WARP
    /// policy in [`Self::new`]. Tests that compare backend pixels must not
    /// inherit a machine-specific hardware adapter instead.
    #[cfg(test)]
    pub(crate) fn new_warp_test_context(
        native_window: *mut c_void,
        width: i32,
        height: i32,
    ) -> Result<Self> {
        if native_window.is_null() {
            return Err(Error::new(
                Errc::PlatformError,
                "D3d11Context: native window handle is null",
            ));
        }
        let drawable = win_surface::drawable_size(native_window, width, height);
        create_with_drawable(
            native_window,
            drawable,
            &D3D11_FEATURE_LEVELS,
            D3d11DriverKind::Warp,
        )
    }

    pub(super) fn create_rtv(&mut self) -> Result<()> {
        let back_buffer: ID3D11Texture2D = unsafe {
            self.swap_chain
                .GetBuffer(0)
                .map_err(|err| d3d_error("IDXGISwapChain::GetBuffer", err))?
        };
        let mut rtv = None;
        unsafe {
            self.device
                .CreateRenderTargetView(&back_buffer, None, Some(&mut rtv))
                .map_err(|err| d3d_error("ID3D11Device::CreateRenderTargetView", err))?;
        }
        let rtv = rtv.ok_or_else(|| {
            Error::new(
                Errc::PlatformError,
                "D3d11Context: CreateRenderTargetView returned no RTV",
            )
        })?;
        unsafe {
            self.context
                .OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
        }
        self.bind_viewport();
        self.rtv = Some(rtv);
        Ok(())
    }

    pub(super) fn release_rtv(&mut self) {
        unsafe {
            self.context.OMSetRenderTargets(None, None);
        }
        self.rtv = None;
    }

    pub(super) fn shutdown_result(&mut self) -> Result<()> {
        self.bound_offscreen = None;
        self.offscreens.clear();
        self.free_offscreen_ids.clear();
        self.next_offscreen_id = 0;
        self.release_rtv();
        Ok(())
    }

    pub(super) fn bind_viewport(&self) {
        self.bind_viewport_size(self.width, self.height);
    }

    pub(super) fn bind_viewport_size(&self, width: i32, height: i32) {
        let vp = D3D11_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: width.max(1) as f32,
            Height: height.max(1) as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        };
        unsafe {
            self.context.RSSetViewports(Some(&[vp]));
        }
    }

    pub(super) fn ensure_rtv(&mut self) -> Result<()> {
        if self.rtv.is_none() {
            self.create_rtv()?;
        }
        Ok(())
    }

    pub(super) fn current_target_size(&self) -> (i32, i32) {
        if let Some(id) = self.bound_offscreen {
            if let Some(Some(t)) = self.offscreens.get(id as usize) {
                return (t.width, t.height);
            }
        }
        (self.width, self.height)
    }

    pub(super) fn bind_current_draw_target(&mut self) -> Result<()> {
        if let Some(id) = self.bound_offscreen {
            let Some(Some(target)) = self.offscreens.get(id as usize) else {
                return Err(Error::new(
                    Errc::InvalidArgument,
                    format!("D3d11Context: bind_current_draw_target unknown offscreen {id}"),
                ));
            };
            unsafe {
                self.context
                    .OMSetRenderTargets(Some(&[Some(target.rtv.clone())]), None);
            }
            self.bind_viewport_size(target.width, target.height);
            return Ok(());
        }
        self.ensure_rtv()?;
        if let Some(rtv) = self.rtv.as_ref() {
            unsafe {
                self.context
                    .OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
            }
            self.bind_viewport();
        }
        Ok(())
    }

    /// 确保模糊水平 pass 的中间纹理存在且与 `width`×`height` 匹配，
    /// 返回其 (RTV, SRV)。尺寸变化时重建。
    pub(super) fn ensure_blur_scratch(
        &mut self,
        width: i32,
        height: i32,
    ) -> Result<(ID3D11RenderTargetView, ID3D11ShaderResourceView), Error> {
        if let Some((_, rtv, srv, w, h)) = &self.blur_scratch {
            if *w == width && *h == height {
                return Ok((rtv.clone(), srv.clone()));
            }
        }
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width.max(1) as u32,
            Height: height.max(1) as u32,
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
        let mut tex = None;
        unsafe {
            self.device
                .CreateTexture2D(&desc, None, Some(&mut tex))
                .map_err(|e| d3d_error("CreateTexture2D(blur scratch)", e))?;
        }
        let tex =
            tex.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Context: no blur scratch"))?;
        let mut rtv = None;
        unsafe {
            self.device
                .CreateRenderTargetView(&tex, None, Some(&mut rtv))
                .map_err(|e| d3d_error("CreateRenderTargetView(blur scratch)", e))?;
        }
        let rtv = rtv.ok_or_else(|| {
            Error::new(Errc::PlatformError, "D3d11Context: no blur scratch RTV")
        })?;
        let mut srv = None;
        let srv_desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            ViewDimension: D3D11_SRV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_SRV {
                    MostDetailedMip: 0,
                    MipLevels: 1,
                },
            },
        };
        unsafe {
            self.device
                .CreateShaderResourceView(&tex, Some(&srv_desc), Some(&mut srv))
                .map_err(|e| d3d_error("CreateShaderResourceView(blur scratch)", e))?;
        }
        let srv = srv.ok_or_else(|| {
            Error::new(Errc::PlatformError, "D3d11Context: no blur scratch SRV")
        })?;
        self.blur_scratch = Some((tex, rtv.clone(), srv.clone(), width, height));
        Ok((rtv, srv))
    }

    pub(super) fn present_result(&mut self) -> Result<()> {
        // SAFETY: the swap chain belongs to this context and is used only on
        // its owning UI thread while the context remains alive.
        //
        // Release all back-buffer refs *before* Present. Holding an RTV (or any
        // GetBuffer view) across Present with DXGI_SWAP_EFFECT_DISCARD lets DXGI
        // allocate extra swapchain buffers — unbounded GPU/system memory growth.
        // Recreate RTV *after* Present so the next frame targets the current
        // back buffer (stale RTV → alternating good/black frames, BUG-001).
        self.release_rtv();
        map_dxgi_present_result(unsafe { self.swap_chain.Present(1, DXGI_PRESENT(0)) })?;
        self.create_rtv()?;
        Ok(())
    }
}

pub(crate) fn create_with_driver(
    hwnd: HWND_PTR,
    width: i32,
    height: i32,
    feature_levels: &[D3D_FEATURE_LEVEL],
    driver: D3d11DriverKind,
) -> Result<D3d11Context> {
    let mut swap_chain = None;
    let mut device = None;
    let mut context = None;
    let mut selected_level = D3D_FEATURE_LEVEL_10_0;
    let desc = swap_chain_desc(hwnd, width, height);

    unsafe {
        D3D11CreateDeviceAndSwapChain(
            None,
            driver.native(),
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(feature_levels),
            D3D11_SDK_VERSION,
            Some(&desc),
            Some(&mut swap_chain),
            Some(&mut device),
            Some(&mut selected_level),
            Some(&mut context),
        )
    }
    .map_err(|err| d3d_error("D3D11CreateDeviceAndSwapChain", err))?;

    let device = device.ok_or_else(|| {
        Error::new(
            Errc::PlatformError,
            "D3d11Context: D3D11CreateDeviceAndSwapChain returned no device",
        )
    })?;
    let context = context.ok_or_else(|| {
        Error::new(
            Errc::PlatformError,
            "D3d11Context: D3D11CreateDeviceAndSwapChain returned no device context",
        )
    })?;
    let swap_chain = swap_chain.ok_or_else(|| {
        Error::new(
            Errc::PlatformError,
            "D3d11Context: D3D11CreateDeviceAndSwapChain returned no swapchain",
        )
    })?;

    let adapter_info = query_adapter_info(&device, driver).unwrap_or_else(|error| {
        tracing::warn!(
            "D3d11Context: adapter diagnostics unavailable: {}",
            error.what()
        );
        D3d11AdapterInfo::unavailable(driver)
    });
    let pipeline = D3d11Pipeline::new(&device)?;
    let mut ctx = D3d11Context {
        hwnd,
        device,
        context,
        swap_chain,
        rtv: None,
        pipeline,
        adapter_info,
        logical_width: width,
        logical_height: height,
        width,
        height,
        offscreens: Vec::new(),
        free_offscreen_ids: Vec::new(),
        next_offscreen_id: 0,
        bound_offscreen: None,
        blur_scratch: None,
    };
    tracing::info!(
        "D3d11Context: created {width}x{height} swapchain at feature level {:?}; {}",
        selected_level,
        ctx.adapter_info.diagnostic_summary()
    );
    ctx.create_rtv()?;
    Ok(ctx)
}

fn create_with_drawable(
    hwnd: HWND_PTR,
    drawable: win_surface::DrawableSize,
    feature_levels: &[D3D_FEATURE_LEVEL],
    driver: D3d11DriverKind,
) -> Result<D3d11Context> {
    let mut context = create_with_driver(
        hwnd,
        drawable.width,
        drawable.height,
        feature_levels,
        driver,
    )?;
    context.logical_width = drawable.logical_width;
    context.logical_height = drawable.logical_height;
    Ok(context)
}

