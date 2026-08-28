// EGL 的共享 Surface、recipe 与 OpenGL RHI host 生命周期实现。

// 复用父平台模块中的 EGL owner 及其私有原生辅助函数。
use super::EglContext;
// 引入共享 Surface 生命周期契约。
use crate::platform::presentation::{GraphicsContextLifecycle, PresentDamage};
// 引入共享 OpenGL RHI host 生命周期契约。
use crate::native::presentation::graphics::opengl::rhi_host::OpenGlRhiHost;
// 引入共享 Surface 生命周期、resize 与重建事务。
use crate::platform::presentation::rhi::{
    RhiExtent, RhiSurfaceLifecycle, RhiSurfaceRecreateReason, RhiSurfaceRecreateTransaction,
    RhiSurfaceResizeTransaction,
};
// 引入共享 OpenGL raster pipeline 类型。
use crate::native::presentation::graphics::opengl::raster::OpenGlRasterPipeline;
// 引入统一错误与结果类型。
use crate::native::{Error, Result};

// 为 EGL owner 实现共享 graphics context 生命周期。
impl GraphicsContextLifecycle for EglContext {
    // 返回 EGL drawable 的完整 live surface 快照。
    fn present_surface(&self) -> crate::platform::presentation::PresentSurface {
        // generation 只读取共享生命周期，不再混入 windowing revision。
        let generation = self.surface_lifecycle.token().generation;
        // resize 事务开始前必须能读取 windowing 刚发布的新 DPR。
        match self.metrics.snapshot() {
            // 健康快照直接报告同代 drawable 与 DPR。
            Ok(snapshot) => crate::platform::presentation::PresentSurface::identity(
                // 报告目标物理 drawable 宽度。
                snapshot.drawable_width,
                // 报告目标物理 drawable 高度。
                snapshot.drawable_height,
                // Wayland core scale 是当前设备像素比。
                snapshot.scale as f32,
                // 返回唯一共享 Surface generation。
                generation,
            ),
            // trait 无错误通道时回退到最后一次成功应用的 EGL 状态。
            Err(_) => crate::platform::presentation::PresentSurface::identity(
                // 最后成功物理宽度。
                self.width,
                // 最后成功物理高度。
                self.height,
                // 最后成功应用的 DPR。
                self.device_pixel_ratio,
                // 最后成功共享 Surface 代次。
                generation,
            ),
        }
    }

    // 将 checked shutdown 交给 EGL owner 的唯一清理事务。
    fn try_shutdown(&mut self) -> Result<(), Error> {
        // 复用同一关闭状态机，避免生命周期事实分叉。
        self.shutdown_result()
    }
}

// 把 EGL thin RHI 与逻辑 surface resize 收敛到同一 recipe owner。
impl crate::platform::presentation::GpuRecipeContext for EglContext {
    // 借用 EGL owner 已实现的组合 thin RHI。
    fn rhi_context(
        // 借用当前 EGL owner。
        &mut self,
    ) -> Result<&mut dyn crate::platform::presentation::rhi::GraphicsContextRhi, Error> {
        // 在借出 Device/Surface 组合能力前先执行 owner 生命周期门禁。
        self.ensure_rhi_active()?;
        // 同一实例完整实现 GraphicsDevice 与 GraphicsSurface。
        Ok(self)
    }

    // 复用统一 DPR、范围检查与 GraphicsSurface::resize 调用。
    fn resize_surface(&mut self, width: i32, height: i32) -> Result<(), Error> {
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

// 将 EGL 原生生命周期接入共享 OpenGL RHI host。
impl OpenGlRhiHost for EglContext {
    // 检查 EGL owner 的统一 active 状态。
    fn rhi_ensure_active(&self) -> Result<()> {
        // 复用 EGL inherent helper，避免 shared host 猜测句柄状态。
        self.ensure_rhi_active()
    }

    // 借用可变 raster/RHI owner。
    fn rhi_pipeline_mut(&mut self) -> &mut OpenGlRasterPipeline {
        &mut self.pipeline
    }

    // 借用只读 raster/RHI owner。
    fn rhi_pipeline(&self) -> &OpenGlRasterPipeline {
        &self.pipeline
    }

    // 切换到 EGL owner-thread context。
    fn rhi_make_current(&mut self) -> Result<(), Error> {
        // current 只作为 adapter 私有 RHI host 操作存在。
        self.make_current_result()
    }

    // 借用唯一共享 Surface 生命周期。
    fn rhi_surface_lifecycle(&self) -> &RhiSurfaceLifecycle {
        &self.surface_lifecycle
    }

    // 可变借用同一个共享 Surface 生命周期。
    fn rhi_surface_lifecycle_mut(&mut self) -> &mut RhiSurfaceLifecycle {
        &mut self.surface_lifecycle
    }

    // Wayland windowing 与 EGL 共享同一份客户端装饰外观事实。
    fn rhi_surface_corner_radius(&self) -> f32 {
        self.metrics
            .snapshot()
            .map_or(0.0, |snapshot| snapshot.corner_radius as f32)
    }

    // Wayland windowing 与 EGL 共享同一份客户端阴影环外观事实。
    fn rhi_surface_shadow_fill(&self) -> (f32, f32) {
        self.metrics
            .snapshot()
            .map_or((0.0, 0.0), |snapshot| {
                (snapshot.shadow_alpha, snapshot.shadow_size as f32)
            })
    }

    // 判断 EGL drawable、逻辑尺寸与 DPR 是否已满足 resize 请求。
    fn rhi_surface_matches(&self, resize: RhiSurfaceResizeTransaction) -> Result<bool> {
        // 读取共享 resize 事务冻结的请求 extent。
        let extent = resize.extent();
        // 物理 extent 相同但 logical extent 或 DPR 改变时仍须进入共享重建事务。
        let snapshot = self.metrics.snapshot()?;
        // 只有四项原生事实全都一致才可跳过。
        Ok(self.pipeline.rhi_surface_extent() == extent
            && self.logical_width == snapshot.logical_width
            && self.logical_height == snapshot.logical_height
            && self.device_pixel_ratio == snapshot.scale as f32)
    }

    // 机械消费共享事务并完成 EGL 原生 Surface 操作。
    fn rhi_recreate_surface(
        &mut self,
        recreate: RhiSurfaceRecreateTransaction,
    ) -> Result<RhiExtent, Error> {
        // 原生宽高只能来自共享生命周期已验证的封闭投影。
        let (width, height) = recreate.native_size_i32();
        match recreate.reason() {
            // Initialize 已在构造期完成，不允许第二次进入原生初始化。
            RhiSurfaceRecreateReason::Initialize => {
                return Err(crate::native::Error::new(
                    crate::native::Errc::InvalidState,
                    "EglContext: initialize transaction reached active host",
                ));
            }
            // resize 只调整 wl_egl_window 与 pipeline，不复制 generation 逻辑。
            RhiSurfaceRecreateReason::Resize => self.rhi_make_current()?,
            // acquire/present 丢失必须先替换真正的 EGLSurface。
            RhiSurfaceRecreateReason::AcquisitionRejected
            | RhiSurfaceRecreateReason::PresentationRejected
            | RhiSurfaceRecreateReason::PresentedNeedsRecreate => {
                self.recreate_window_surface()?;
            }
        }
        // 所有原因最后都把事务请求同步到唯一 drawable/pipeline 元数据。
        self.resize_surface_extent(width, height)?;
        // Adapter 只报告原生操作实际产生的正 extent。
        Ok(RhiExtent::new(self.width as u32, self.height as u32))
    }

    // 交换 EGL window surface。
    fn rhi_swap_buffers(&mut self, _damage: PresentDamage) -> Result<(), Error> {
        // 业务门禁已在 shared host present 中完成，cleanup 不复用该门禁。
        self.egl
            .swap_buffers(self.display, self.surface)
            .map_err(super::map_egl_swap_error)
    }
}
