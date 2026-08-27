//! 绘图层会话 — Pipeline + Backend 组合入口（Phase 1 骨架）。

use crate::core::Error;
use crate::draw::backend::GpuBackend;
use crate::draw::backend::{
    BackendCapabilities, BackendKind, CpuBackend, RenderBackend, create_backend,
};
// 测试读回只通过 Drawing 层规范快照跨越会话边界。
#[cfg(feature = "test-harness")]
use crate::draw::backend::SurfaceReadback;
use crate::draw::outcome::RenderOutcome;
use crate::draw::{Canvas2D, GraphicsCapabilities, UpdateStrategy};
use std::thread::ThreadId;

/// 绘图层会话：持有可切换后端与共享帧逻辑。
pub struct RenderSession {
    backend: Box<dyn RenderBackend>,
    width: i32,
    height: i32,
    /// 后端切换后下一帧强制全帧重绘。
    force_full_frame: bool,
    /// The construction thread owns every live backend/context beneath this
    /// session. Backends are non-Send; this makes the affinity explicit at
    /// lifecycle boundaries as well.
    owner_thread: ThreadId,
}

impl RenderSession {
    /// 以指定后端种类创建会话。
    pub fn new(kind: BackendKind) -> Result<Self, Error> {
        let resolved = resolve_kind(kind);
        // 通用会话构造只委托不依赖原生 recipe 的 backend kind factory。
        let backend = create_backend(resolved)?;
        Ok(Self {
            backend,
            width: 0,
            height: 0,
            force_full_frame: false,
            owner_thread: std::thread::current().id(),
        })
    }

    /// 注入已有后端实例。
    pub fn with_backend(backend: Box<dyn RenderBackend>) -> Self {
        Self {
            backend,
            width: 0,
            height: 0,
            force_full_frame: false,
            owner_thread: std::thread::current().id(),
        }
    }

    /// 返回当前后端种类。
    pub fn backend_kind(&self) -> BackendKind {
        self.backend.kind()
    }

    /// 返回当前后端的内部能力集合。
    pub fn capabilities(&self) -> BackendCapabilities {
        self.backend.capabilities()
    }

    /// 返回当前后端面向应用的图形能力投影。
    pub fn graphics_capabilities(&self) -> GraphicsCapabilities {
        self.capabilities().into()
    }

    /// 返回后端实际采用的可绘制宽度。
    pub fn width(&self) -> i32 {
        self.width
    }

    /// 返回后端实际采用的可绘制高度。
    pub fn height(&self) -> i32 {
        self.height
    }

    /// 在所有者线程上初始化后端尺寸并同步实际可绘制范围。
    pub fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.require_owner("initialize")?;
        self.backend.resize(width, height)?;
        self.sync_extent_from_surface();
        Ok(())
    }

    /// 在工厂已经准备好的原生目标上启动会话。
    /// 后端回报实际采用的可绘制范围，避免把请求的逻辑尺寸误当成修正后的交换链客户区。
    pub(crate) fn initialize_prepared(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.require_owner("initialize_prepared")?;
        let _ = self.backend.initialize_prepared(width, height)?;
        self.sync_extent_from_surface();
        Ok(())
    }

    /// 在所有者线程上关闭后端；失败会记录错误并保留可重试所有权。
    pub fn shutdown(&mut self) {
        if let Err(error) = self.try_shutdown() {
            tracing::error!("RenderSession: {}", error.short_what());
        }
    }

    /// 在 owner thread 上关闭当前后端。
    /// 失败时保留仍存活的 owner，供恢复层或 Drop 路径稍后重试。
    pub(crate) fn try_shutdown(&mut self) -> Result<(), Error> {
        self.require_owner("shutdown")?;
        self.backend.try_shutdown()?;
        self.width = 0;
        self.height = 0;
        Ok(())
    }

    /// 在所有者线程上调整后端尺寸，并要求下一帧完整重绘。
    pub fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.require_owner("resize")?;
        self.backend.resize(width, height)?;
        // Native GPU 后端会按 HWND 实际客户区校正 drawable；帧 clip 必须跟
        // surface 一致，否则 begin_frame 会在更大 swapchain 上只绘制较小区域。
        self.sync_extent_from_surface();
        // swapchain/缓冲 resize 后内容丢失，下一帧须全帧重绘。
        self.force_full_frame = true;
        Ok(())
    }

    /// 将会话 extent 与后端 surface 对齐（initialize_prepared / resize 后调用）。
    fn sync_extent_from_surface(&mut self) {
        let size = self.backend.surface().size();
        self.width = size.w.round().max(1.0) as i32;
        self.height = size.h.round().max(1.0) as i32;
    }

    /// 运行时切换后端；下一帧将强制 FullRedraw。
    pub fn set_backend(&mut self, kind: BackendKind) -> Result<(), Error> {
        self.require_owner("set_backend")?;
        let resolved = resolve_kind(kind);
        // GPU 会在通用工厂中被拒绝，正式路径必须直接注入已验证 backend。
        let replacement = create_backend(resolved)?;
        // 旧 backend 的检查式关闭成功是唯一 owner 切换的提交点。
        self.backend.try_shutdown()?;
        // 提交后立刻安装候选，后续 resize 失败也保留新 owner 供恢复或重试。
        self.backend = replacement;
        // capability/资源 owner 已改变，任何后续结果都必须保留全帧重绘要求。
        self.force_full_frame = true;
        if self.width > 0 && self.height > 0 {
            self.backend.resize(self.width, self.height)?;
            self.sync_extent_from_surface();
        }
        Ok(())
    }

    /// 准备并开始一帧；后端切换或尺寸变化后会覆盖为完整重绘。
    pub fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome {
        if let Err(error) = self.require_owner("begin_frame") {
            return RenderOutcome::Failed(crate::draw::renderer::GraphicsFailure::from_error(
                error,
            ));
        }
        // 每帧先执行后端语义型准备；GPU 实现只进入 thin RHI 设备维护。
        if let Err(error) = self.backend.prepare_frame() {
            // 准备失败时保留 force_full_frame，交给恢复层按 typed failure 处理。
            return RenderOutcome::Failed(crate::draw::renderer::GraphicsFailure::from_error(
                // 传播原始设备或 owner-thread 错误。
                error,
            ));
        }
        if self.force_full_frame {
            self.force_full_frame = false;
            let caps = self.backend.capabilities();
            return crate::draw::renderer::lifecycle::begin_frame(
                UpdateStrategy::FullRedraw,
                self.backend.surface(),
                self.width,
                self.height,
                caps,
            );
        }
        let caps = self.backend.capabilities();
        crate::draw::renderer::lifecycle::begin_frame(
            strategy,
            self.backend.surface(),
            self.width,
            self.height,
            caps,
        )
    }

    /// 在所有者线程上结束并提交当前帧。
    pub fn end_frame(&mut self) -> RenderOutcome {
        if let Err(error) = self.require_owner("end_frame") {
            return RenderOutcome::Failed(crate::draw::renderer::GraphicsFailure::from_error(
                error,
            ));
        }
        crate::draw::renderer::lifecycle::end_frame(self.backend.surface())
    }

    pub(crate) fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        debug_assert!(self.require_owner("canvas_2d").is_ok());
        self.backend.surface().canvas()
    }

    /// 当当前后端为 CPU 后端时返回其只读引用。
    pub fn cpu_backend(&self) -> Option<&CpuBackend> {
        self.backend.as_any().downcast_ref()
    }

    pub(crate) fn cpu_backend_mut(&mut self) -> Option<&mut CpuBackend> {
        self.backend.as_any_mut().downcast_mut()
    }

    pub(crate) fn gpu_backend(&self) -> Option<&GpuBackend> {
        self.backend.as_any().downcast_ref()
    }

    pub(crate) fn gpu_backend_mut(&mut self) -> Option<&mut GpuBackend> {
        self.backend.as_any_mut().downcast_mut()
    }

    // 把测试设备丢失注入限制在当前图形 owner thread 和 GPU backend。
    #[cfg(feature = "test-harness")]
    pub(crate) fn inject_graphics_device_lost_for_test(&mut self) -> Result<(), Error> {
        // 先验证调用线程，避免测试入口绕过 native context 亲和性。
        self.require_owner("inject_graphics_device_lost_for_test")?;
        // 只有薄 RHI GPU backend 能把注入推进到 adapter 维护边界。
        let Some(backend) = self.gpu_backend_mut() else {
            return Err(Error::new(
                crate::core::Errc::NotImplemented,
                "render session does not own a GPU backend",
            ));
        };
        // 交给 GPU backend 选择具体的 RHI adapter。
        backend.inject_graphics_device_lost_for_test()
    }

    // 把测试 surface-lost 注入限制在当前图形 owner thread 和 GPU backend。
    #[cfg(feature = "test-harness")]
    pub(crate) fn inject_graphics_surface_lost_for_test(&mut self) -> Result<(), Error> {
        // 先验证调用线程，避免测试入口绕过 native context 亲和性。
        self.require_owner("inject_graphics_surface_lost_for_test")?;
        // 只有薄 RHI GPU backend 能把注入推进到 surface acquire 边界。
        let Some(backend) = self.gpu_backend_mut() else {
            return Err(Error::new(
                crate::core::Errc::NotImplemented,
                "render session does not own a GPU backend",
            ));
        };
        // 交给 GPU backend 选择具体的 RHI surface。
        backend.inject_graphics_surface_lost_for_test()
    }

    // 在图形 owner thread 上安排最终 composite 与 present 之间的回读。
    #[cfg(feature = "test-harness")]
    pub(crate) fn request_surface_readback_for_test(&mut self) -> Result<(), Error> {
        // 与其它原生生命周期入口共用线程亲和性门禁。
        self.require_owner("request_surface_readback_for_test")?;
        // 只通过 RenderBackend 能力端口下沉，不查询具体图形 API。
        self.backend.request_surface_readback_for_test()
    }

    // 在最终 present 返回后消费同一事务的规范 surface 回读。
    #[cfg(feature = "test-harness")]
    pub(crate) fn take_surface_readback_for_test(&mut self) -> Result<SurfaceReadback, Error> {
        // 结果消费同样只能发生在图形 owner thread。
        self.require_owner("take_surface_readback_for_test")?;
        // backend 负责清理请求状态并返回完整结果或 typed failure。
        self.backend.take_surface_readback_for_test()
    }

    pub(crate) fn backend_mut(&mut self) -> &mut dyn RenderBackend {
        debug_assert!(self.require_owner("backend_mut").is_ok());
        &mut *self.backend
    }

    pub(crate) fn backend(&self) -> &dyn RenderBackend {
        debug_assert!(self.require_owner("backend").is_ok());
        &*self.backend
    }

    fn require_owner(&self, operation: &str) -> Result<(), Error> {
        if std::thread::current().id() == self.owner_thread {
            Ok(())
        } else {
            Err(Error::new(
                crate::core::Errc::InvalidState,
                format!("RenderSession::{operation} must run on its owning graphics thread"),
            ))
        }
    }
}

impl Drop for RenderSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn resolve_kind(kind: BackendKind) -> BackendKind {
    match kind {
        BackendKind::Auto => BackendKind::Cpu,
        other => other,
    }
}
