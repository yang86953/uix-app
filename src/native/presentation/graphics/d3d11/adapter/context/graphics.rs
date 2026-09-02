use super::*;

impl GraphicsContextLifecycle for D3d11Context {
    // 把 D3D11 当前 drawable 元数据提供给共享生命周期边界。
    fn present_surface(&self) -> crate::platform::presentation::PresentSurface {
        // 物理范围与 generation 必须来自同一个共享 token 快照。
        let token = self.surface_lifecycle.token();
        let width = token.extent.width as i32;
        let height = token.extent.height as i32;
        crate::platform::presentation::PresentSurface::identity(
            width,
            height,
            // 从同一 context 状态计算本次快照的 drawable 比例。
            width as f32 / self.logical_width.max(1) as f32,
            token.generation,
        )
    }

    fn try_shutdown(&mut self) -> Result<()> {
        self.shutdown_result()
    }
}

// 把 D3D11 thin RHI 与逻辑 surface resize 收敛到同一 recipe owner。
impl crate::platform::presentation::GpuRecipeContext for D3d11Context {
    // 返回当前可写 swapchain image 身份，供 graphics damage history 规划。
    fn present_image(&self) -> Option<crate::platform::presentation::PresentImage> {
        // 只有构造期冻结的 SwapChain3 主路径返回真实 image index。
        self.swap_chain.present_image()
    }

    // 借用 D3D11 owner 已实现的组合 thin RHI。
    fn rhi_context(
        // 借用当前 D3D11 owner。
        &mut self,
    ) -> Result<&mut dyn crate::platform::presentation::rhi::GraphicsContextRhi> {
        // checked shutdown 后不得重新借出 thin RHI owner。
        self.ensure_active()?;
        // 同一实例完整实现 GraphicsDevice 与 GraphicsSurface。
        Ok(self)
    }

    // 复用统一 DPR、范围检查与 GraphicsSurface::resize 调用。
    fn resize_surface(&mut self, width: i32, height: i32) -> Result<()> {
        // 在可变借用前取得当前完整 surface 快照。
        let present_surface = GraphicsContextLifecycle::present_surface(self);
        // 直接借用当前原子 recipe owner，不经过分裂兼容视图。
        crate::platform::presentation::resize_native_rhi_surface(
            self,
            present_surface,
            width,
            height,
        )
    }
}

// 让直接析构 D3D11 adapter 时也执行同一 checked shutdown。
impl Drop for D3d11Context {
    // Drop 没有错误返回通道，因此必须记录关闭失败诊断。
    fn drop(&mut self) {
        // 显式观察 checked shutdown 结果，禁止静默丢弃未来新增的失败。
        if let Err(error) = self.shutdown_result() {
            // Drop 关闭失败经边界观察入口记录。
            crate::diagnostics::observe_boundary_error("d3d11/adapter", &error);
        }
    }
}

// 为 D3D11 surface adapter 保留私有回读实现，不再扩张兼容门面。
impl D3d11Context {
    // 通过 staging texture 同步读取当前 swapchain backbuffer。
    pub(super) fn read_surface_pixels_result(
        // 借用当前 D3D11 owner context。
        &mut self,
        // 接收回读区域左上角横坐标。
        x: i32,
        // 接收回读区域左上角纵坐标。
        y: i32,
        // 接收回读区域宽度。
        width: i32,
        // 接收回读区域高度。
        height: i32,
    ) -> Result<Vec<u32>> {
        // 回读范围与 staging 描述共享同一个已发布物理 extent。
        let surface_extent = self.surface_lifecycle.token().extent;
        let surface_width = surface_extent.width as i32;
        let surface_height = surface_extent.height as i32;
        let x0 = x.clamp(0, surface_width);
        let y0 = y.clamp(0, surface_height);
        let x1 = x.saturating_add(width).clamp(x0, surface_width);
        let y1 = y.saturating_add(height).clamp(y0, surface_height);
        let read_w = x1 - x0;
        let read_h = y1 - y0;
        if read_w <= 0 || read_h <= 0 {
            return Ok(Vec::new());
        }

        (|| -> Result<Vec<u32>> {
            // SAFETY: swap_chain 由当前 context 拥有且索引 0 是当前存活的 D3D11 back buffer。
            let back_buffer: ID3D11Texture2D = unsafe {
                self.swap_chain
                    .get_buffer(0)
                    .map_err(|err| d3d_error("IDXGISwapChain::GetBuffer(read_pixels)", err))?
            };
            let desc = D3D11_TEXTURE2D_DESC {
                Width: surface_extent.width,
                Height: surface_extent.height,
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
            // SAFETY: device 属于当前 owner thread，desc 完整初始化，输出槽在同步调用期间有效。
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
            // SAFETY: staging 与 back_buffer 由同一 D3D11 device 创建，复制在 immediate context owner thread 执行。
            unsafe {
                self.context.CopyResource(&staging, &back_buffer);
            }
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            // SAFETY: staging 以 CPU_READ staging usage 创建，mapped 输出槽在同步 Map 调用期间有效。
            unsafe {
                self.context
                    .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                    .map_err(|err| d3d_error("ID3D11DeviceContext::Map(read_pixels)", err))?;
            }
            let mut pixels = vec![0u32; (read_w as usize).saturating_mul(read_h as usize)];
            for row in 0..read_h as usize {
                // SAFETY: Map 成功后 pData 非空，读区与 RowPitch 偏移已由裁剪后的 x/y/read_w/read_h 限制。
                let src = unsafe {
                    mapped
                        .pData
                        .cast::<u8>()
                        .add((y0 as usize + row) * mapped.RowPitch as usize + x0 as usize * 4)
                        .cast::<u32>()
                };
                let dst = pixels[row * read_w as usize..].as_mut_ptr();
                // SAFETY: src 和 dst 各自至少覆盖 read_w 个 u32 且位于不同资源，目标切片容量由上方分配保证。
                unsafe {
                    std::ptr::copy_nonoverlapping(src, dst, read_w as usize);
                }
            }
            // SAFETY: staging 在本函数中只成功 Map 一次，此处在返回前于同一 immediate context 上配对 Unmap。
            unsafe {
                self.context.Unmap(&staging, 0);
            }
            Ok(pixels)
        })()
    }
}
