use super::*;
// 引入共享 Surface 生命周期签发的原生重建输入与封闭呈现值。
use crate::platform::presentation::rhi::{
    RhiExtent, RhiSurfaceLifecycle, RhiSurfaceRecreateReason, RhiSurfaceRecreateTransaction,
    ValidatedRhiPresent,
};

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
                // HW→WARP 性能降级事实经边界观察入口记录。
                crate::diagnostics::observe_boundary_error("d3d11/hw_to_warp", &hardware_error);
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
        // 正常 acquire/present 只为共享生命周期已经发布的 extent 建立视图。
        let extent = self.surface_lifecycle.token().extent;
        self.create_rtv_for_extent(extent)
    }

    // 为共享事务已经验证的物理范围机械创建 RTV 与 viewport。
    fn create_rtv_for_extent(&mut self, extent: RhiExtent) -> Result<()> {
        // SAFETY: swap_chain 由本 context 持有且存活，get_buffer 返回的纹理由接口类型接管。
        let back_buffer: ID3D11Texture2D = unsafe {
            self.swap_chain
                .get_buffer(0)
                .map_err(|err| d3d_error("IDXGISwapChain::GetBuffer", err))?
        };
        let mut rtv = None;
        // SAFETY: back_buffer 为刚取得的存活纹理；输出指针指向栈上 Option；device 存活。
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
        // SAFETY: rtv 为刚创建的存活视图且克隆自本 context；depth 传 None。
        unsafe {
            self.context
                .OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
        }
        // 原生重建提交前生命周期仍保留旧 token，因此显式使用事务 extent。
        let (width, height) = extent.native_size_i32().ok_or_else(|| {
            Error::new(
                Errc::InvalidArgument,
                "D3d11 RTV extent must be representable by the native viewport",
            )
        })?;
        self.bind_viewport_size(width, height);
        self.rtv = Some(rtv);
        Ok(())
    }

    // 机械消费共享重建事务，执行 ResizeBuffers/RTV 并报告实际原生 extent。
    pub(super) fn recreate_surface_native(
        &mut self,
        recreate: RhiSurfaceRecreateTransaction,
        previous: RhiExtent,
        logical_width: i32,
        logical_height: i32,
    ) -> Result<RhiExtent> {
        // checked shutdown 后不得重新创建 swapchain surface 资源。
        self.ensure_active()?;
        // 原生宽高只能来自共享生命周期已经验证并封闭的投影。
        let (physical_width, physical_height) = recreate.native_size_i32();
        let requested = recreate.requested();
        // 先解除旧 RTV 绑定，满足 ResizeBuffers 的资源生命周期要求。
        self.release_rtv();
        // 让 DXGI 执行 backbuffer 的原生尺寸重建。
        // SAFETY: 尺寸已在上方验证为正；旧 RTV 已释放，满足 ResizeBuffers 的引用释放要求。
        if let Err(error) = unsafe {
            self.swap_chain
                .resize_buffers(physical_width as u32, physical_height as u32)
        } {
            // 将设备移除、Surface 丢失等 DXGI 结果映射为统一错误。
            let Err(primary) = map_dxgi_resize_result(error.code()) else {
                return Err(Error::new(
                    Errc::PlatformError,
                    "D3d11 ResizeBuffers failed without a typed error",
                ));
            };
            // ResizeBuffers 失败时原 buffer 仍有效；恢复旧 RTV 后再传播主错误。
            return match self.create_rtv_for_extent(previous) {
                Ok(()) => Err(primary),
                Err(rollback) => Err(primary.with_source(rollback)),
            };
        }
        // 新 buffer 建立后必须先恢复 RTV；失败时把原生尺寸回滚到旧 token。
        if let Err(primary) = self.create_rtv_for_extent(requested) {
            // SAFETY: 新 RTV 创建失败且当前没有 back-buffer view；旧 extent 来自共享 token。
            let rollback_resize = unsafe {
                self.swap_chain
                    .resize_buffers(previous.width, previous.height)
            };
            let rollback = match rollback_resize {
                Ok(()) => self.create_rtv_for_extent(previous),
                Err(error) => map_dxgi_resize_result(error.code()),
            };
            return match rollback {
                Ok(()) => Err(primary),
                Err(rollback) => Err(primary.with_source(rollback)),
            };
        }
        // 只有完整原生重建成功后才提交非生命周期逻辑元数据。
        self.logical_width = logical_width;
        self.logical_height = logical_height;
        // Adapter 不拥有 generation，只报告实际采用的事务请求范围。
        Ok(requested)
    }

    pub(super) fn release_rtv(&mut self) {
        // SAFETY: OMSetRenderTargets 传 None 表示清空所有 render target 槽位，不引用任何对象。
        unsafe {
            self.context.OMSetRenderTargets(None, None);
        }
        self.rtv = None;
    }

    pub(super) fn shutdown_result(&mut self) -> Result<()> {
        // 已完成关闭时保持幂等成功，不重复触碰 COM context。
        if self.shutdown {
            // 向重复关闭调用确认稳定结果。
            return Ok(());
        }
        // 先解除并释放当前 backbuffer RTV。
        self.release_rtv();
        // SAFETY: immediate context 由本 D3D11Context 唯一持有，关闭阶段不再接受新提交。
        unsafe {
            // 清除所有 pipeline/resource 绑定，释放 context 持有的内部 COM 引用。
            self.context.ClearState();
            // 把此前已排队命令提交给 driver，避免对象析构时仍残留未刷新的命令引用。
            self.context.Flush();
        }
        // 只有完整完成解绑、清理与 flush 后才提交关闭事实。
        self.shutdown = true;
        // 向调用方确认 checked shutdown 成功。
        Ok(())
    }

    // 拒绝 checked shutdown 之后的 D3D11 native/RHI 工作。
    pub(super) fn ensure_active(&self) -> Result<()> {
        // 关闭事实是 adapter 内唯一的生命周期门禁。
        if self.shutdown {
            // 返回稳定状态错误，不允许重新创建已关闭资源。
            return Err(Error::new(
                // shutdown 后调用属于 owner 生命周期错误。
                Errc::InvalidState,
                // 保留可诊断的 adapter 与阶段。
                "D3d11Context: operation requested after shutdown",
            ));
        }
        // context 仍处于可工作状态。
        Ok(())
    }

    pub(super) fn bind_viewport(&self) {
        // 已发布的物理范围只从共享生命周期读取。
        let extent = self.surface_lifecycle.token().extent;
        self.bind_viewport_size(extent.width as i32, extent.height as i32);
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
        // SAFETY: vp 为栈上完整初始化的 viewport；宽高已做 max(1) 下限保护。
        unsafe {
            self.context.RSSetViewports(Some(&[vp]));
        }
    }

    pub(super) fn ensure_rtv(&mut self) -> Result<()> {
        // checked shutdown 后不得通过 acquire 隐式重建 RTV。
        self.ensure_active()?;
        if self.rtv.is_none() {
            self.create_rtv()?;
        }
        Ok(())
    }

    pub(super) fn bind_current_draw_target(&mut self) -> Result<()> {
        self.ensure_rtv()?;
        if let Some(rtv) = self.rtv.as_ref() {
            // SAFETY: rtv 为本 context 创建且仍存活的视图；depth 传 None。
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

    pub(super) fn present_result(&mut self, present: &ValidatedRhiPresent) -> Result<()> {
        // checked shutdown 后不得继续提交 swapchain present。
        self.ensure_active()?;
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
        // GetBuffer view) across Present 会阻碍 flip-model 正确轮换；legacy
        // DISCARD 还可能分配额外 buffer。Present 后必须为当前 buffer 重建 RTV。
        self.release_rtv();
        // swapchain adapter 按冻结形态选择 Present1 dirty rect 或 legacy 全帧 Present。
        self.swap_chain.present(present)?;
        self.create_rtv()?;
        Ok(())
    }

    // 返回本实例在创建期冻结的 present coherency。
    pub(crate) fn present_coherency(&self) -> crate::platform::presentation::PresentCoherency {
        // capability 与实际 swapchain 形态同源，运行期不再探测或漂移。
        self.swap_chain.contract().present_coherency
    }
}

pub(crate) fn create_with_driver(
    hwnd: HWND_PTR,
    width: i32,
    height: i32,
    feature_levels: &[D3D_FEATURE_LEVEL],
    driver: D3d11DriverKind,
) -> Result<D3d11Context> {
    // 在任何 D3D11/DXGI 创建动作前建立共享初始化事务。
    let initial_extent = RhiExtent::new(width as u32, height as u32);
    let mut surface_lifecycle = RhiSurfaceLifecycle::uninitialized(initial_extent);
    let surface_initialize =
        surface_lifecycle.begin_recreate(initial_extent, RhiSurfaceRecreateReason::Initialize)?;
    // 原生创建阶段仍由局部 COM owner 自动回收，生命周期只决定发布与回滚。
    let native = (|| -> Result<_> {
        let mut device = None;
        let mut context = None;
        let mut selected_level = D3D_FEATURE_LEVEL_10_0;

        // SAFETY: 无指针输入；feature_levels 切片与输出指针在调用期间有效，driver.native() 为合法驱动类型枚举。
        unsafe {
            D3D11CreateDevice(
                None,
                driver.native(),
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(feature_levels),
                D3D11_SDK_VERSION,
                Some(&mut device),
                Some(&mut selected_level),
                Some(&mut context),
            )
        }
        .map_err(|err| d3d_error("D3D11CreateDevice", err))?;

        let device = device.ok_or_else(|| {
            Error::new(
                Errc::PlatformError,
                "D3d11Context: D3D11CreateDevice returned no device",
            )
        })?;
        let context = context.ok_or_else(|| {
            Error::new(
                Errc::PlatformError,
                "D3d11Context: D3D11CreateDevice returned no device context",
            )
        })?;
        // 使用实际 device 和 adapter 选择 tracked flip 主路径或 legacy 回退。
        let swap_chain = create_swap_chain(&device, hwnd, width, height)?;

        let adapter_info = query_adapter_info(&device, driver).unwrap_or_else(|error| {
            // 适配器诊断信息缺失回退占位：经边界观察入口记录。
            crate::diagnostics::observe_boundary_error("d3d11/adapter_info", &error);
            D3d11AdapterInfo::unavailable(driver)
        });
        let pipeline = D3d11Pipeline::new(&device)?;
        // 在 context 移入 owner 前冻结薄 RHI 使用的可选原生接口能力。
        let rhi_device = D3d11RhiDevice::new(&context);
        Ok((
            device,
            context,
            swap_chain,
            adapter_info,
            pipeline,
            rhi_device,
            selected_level,
        ))
    })();
    let (device, context, swap_chain, adapter_info, pipeline, rhi_device, selected_level) =
        match native {
            Ok(native) => native,
            Err(error) => {
                return match surface_lifecycle.abort_recreate(surface_initialize) {
                    Ok(()) => Err(error),
                    Err(lifecycle_error) => Err(lifecycle_error.with_source(error)),
                };
            }
        };
    let mut ctx = D3d11Context {
        device,
        context,
        swap_chain,
        rtv: None,
        pipeline,
        adapter_info,
        logical_width: width,
        logical_height: height,
        // 初始化事务仍处于 Recreating，RTV 成功后才一次发布 token。
        surface_lifecycle,
        // 构造成功后 context 立即处于可工作状态。
        shutdown: false,
        // 接管已经冻结原生能力且尚未创建资源的薄 RHI 状态。
        rhi_device,
        // 默认不安排测试设备丢失。
        #[cfg(feature = "test-harness")]
        rhi_device_lost_for_test: false,
        // 默认不安排测试 surface 丢失。
        #[cfg(feature = "test-harness")]
        rhi_surface_lost_for_test: false,
    };
    // RTV 是初始化事务的一部分，失败时不得发布半初始化 Surface。
    if let Err(error) = ctx.create_rtv_for_extent(initial_extent) {
        return match ctx.surface_lifecycle.abort_recreate(surface_initialize) {
            Ok(()) => Err(error),
            Err(lifecycle_error) => Err(lifecycle_error.with_source(error)),
        };
    }
    // 原生 swapchain 与 RTV 完整可用后，由共享生命周期发布初始 token/extent。
    ctx.surface_lifecycle
        .commit_recreate(surface_initialize, initial_extent)?;
    tracing::info!(
        "D3d11Context: created {width}x{height} swapchain at feature level {:?}; {}",
        selected_level,
        ctx.adapter_info.diagnostic_summary()
    );
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
