use super::*;

impl D3d11Context {
    // 该诊断计数仅供 crate 内测试读取，生产 context 不暴露此辅助入口。
    #[cfg(test)]
    #[allow(dead_code)]
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
    // 测试目标保留确定性 WARP 构造器，供显式后端像素测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
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

    // 把逻辑尺寸转换和原生 swapchain 重建统一收敛到 context 底层。
    pub(super) fn resize_surface_logical(&mut self, width: i32, height: i32) -> Result<()> {
        // 使用平台 drawable 规则得到实际物理尺寸和逻辑元数据。
        let drawable = win_surface::drawable_size(self.hwnd, width, height);
        // 由同一条原生路径执行 ResizeBuffers、代际推进和 RTV 重建。
        self.resize_surface_extent(
            drawable.width,
            drawable.height,
            drawable.logical_width,
            drawable.logical_height,
        )
    }

    // 直接执行 D3D11 surface 的物理尺寸重建，不再依赖兼容 resize 入口。
    pub(super) fn resize_surface_extent(
        &mut self,
        physical_width: i32,
        physical_height: i32,
        logical_width: i32,
        logical_height: i32,
    ) -> Result<()> {
        // 拒绝无效尺寸，避免把非法参数传给 DXGI。
        if physical_width <= 0 || physical_height <= 0 {
            // 返回稳定的参数错误，保持 RHI 和兼容入口一致。
            return Err(Error::new(
                Errc::InvalidArgument,
                "D3d11 surface extent must be positive",
            ));
        }
        // 尺寸没有变化时无需释放和重建现有 RTV。
        if physical_width == self.width && physical_height == self.height {
            // 仍然同步逻辑尺寸，覆盖同物理尺寸下的逻辑元数据变化。
            self.logical_width = logical_width;
            self.logical_height = logical_height;
            // 返回当前 surface 状态。
            return Ok(());
        }
        // 先解除旧 RTV 绑定，满足 ResizeBuffers 的资源生命周期要求。
        self.release_rtv();
        // 让 DXGI 执行 backbuffer 的原生尺寸重建。
        if let Err(error) = unsafe {
            self.swap_chain.ResizeBuffers(
                0,
                physical_width as u32,
                physical_height as u32,
                DXGI_FORMAT_B8G8R8A8_UNORM,
                DXGI_SWAP_CHAIN_FLAG(0),
            )
        } {
            // 将设备移除、无效参数等 DXGI 结果映射为统一错误。
            map_dxgi_resize_result(error.code())?;
        }
        // 提交成功后更新逻辑尺寸元数据。
        self.logical_width = logical_width;
        self.logical_height = logical_height;
        // 保存新的物理 drawable 尺寸。
        self.width = physical_width;
        self.height = physical_height;
        // ResizeBuffers 成功后推进 surface 代际，隔离旧帧和旧 view。
        self.surface_generation = self.surface_generation.saturating_add(1);
        // 重新创建 RTV 并恢复默认 swapchain target 绑定。
        self.create_rtv()
    }

    pub(super) fn release_rtv(&mut self) {
        unsafe {
            self.context.OMSetRenderTargets(None, None);
        }
        self.rtv = None;
    }

    pub(super) fn shutdown_result(&mut self) -> Result<()> {
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

    pub(super) fn bind_current_draw_target(&mut self) -> Result<()> {
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

    // 为 thin RHI 与最终 present 保留低层 swapchain target 绑定。
    pub(super) fn bind_swapchain_target(&mut self) -> Result<()> {
        // 复用 owner-thread RTV 与 viewport 的统一恢复逻辑。
        self.bind_current_draw_target()
    }

    pub(super) fn present_result(&mut self) -> Result<()> {
        // 兼容 presenter 也必须消费同一 lower surface-lost 注入，避免故障
        // 因本帧没有进入 RHI acquire 而被静默跳过。
        #[cfg(feature = "test-harness")]
        if std::mem::take(&mut self.rhi_surface_lost_for_test) {
            // 只在 test-harness 记录共同 adapter present 边界。
            tracing::warn!("D3d11 RHI test surface lost");
            return Err(Error::new(
                Errc::GraphicsSurfaceLost,
                "D3d11 RHI test surface lost before present",
            ));
        }
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
        // 初始 swapchain 属于第一代 surface。
        surface_generation: 0,
        // 初始化尚未创建资源的薄 RHI 状态。
        rhi_device: D3d11RhiDevice::new(),
        // 默认不安排测试设备丢失。
        #[cfg(feature = "test-harness")]
        rhi_device_lost_for_test: false,
        // 默认不安排测试 surface 丢失。
        #[cfg(feature = "test-harness")]
        rhi_surface_lost_for_test: false,
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
