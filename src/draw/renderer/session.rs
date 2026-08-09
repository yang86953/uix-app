//! 绘图层会话 — Pipeline + Backend 组合入口（Phase 1 骨架）。

use crate::core::Error;
use crate::draw::backend::GpuBackend;
use crate::draw::backend::{
    create_backend, BackendCapabilities, BackendKind, CpuBackend, RenderBackend,
};
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

    pub fn backend_kind(&self) -> BackendKind {
        self.backend.kind()
    }

    pub fn capabilities(&self) -> BackendCapabilities {
        self.backend.capabilities()
    }

    pub fn graphics_capabilities(&self) -> GraphicsCapabilities {
        self.capabilities().into()
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.require_owner("initialize")?;
        self.backend.resize(width, height)?;
        self.sync_extent_from_surface();
        Ok(())
    }

    /// Starts a session on a native target the factory has already prepared.
    /// The backend reports the drawable extent it actually adopted so the
    /// session never assumes the requested logical size matches a corrected
    /// swapchain/client extent.
    pub(crate) fn initialize_prepared(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.require_owner("initialize_prepared")?;
        let _ = self.backend.initialize_prepared(width, height)?;
        self.sync_extent_from_surface();
        Ok(())
    }

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
        let _ = self.backend.try_shutdown();
        self.backend = replacement;
        if self.width > 0 && self.height > 0 {
            self.backend.resize(self.width, self.height)?;
            self.sync_extent_from_surface();
        }
        self.force_full_frame = true;
        Ok(())
    }

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

// 验证帧准备错误会在触碰 surface 前保持 typed failure 分类。
#[cfg(test)]
mod prepare_frame_tests {
    // 复用当前模块的会话与后端类型。
    use super::*;
    // 引入 downcast 契约需要的 Any。
    use std::any::Any;

    // 锁定 GPU 只能通过已验证原生 recipe factory 进入会话。
    #[test]
    fn gpu_selection_requires_validated_native_recipe_factory() {
        // 通用会话构造器不得接收缺少 recipe 校验的 GPU 选择。
        let creation_error = RenderSession::new(BackendKind::Gpu)
            // 提取稳定错误而不要求 RenderSession 实现 Debug。
            .err()
            // GPU 通用构造必须失败。
            .expect("generic GPU session creation must fail");
        // 错误分类必须保持为调用方可修正的非法参数。
        assert_eq!(creation_error.code(), crate::core::Errc::InvalidArgument);
        // 创建可安全切换的 CPU 会话基线。
        let mut session = RenderSession::new(BackendKind::Cpu).expect("CPU session must construct");
        // 运行时通用切换同样不得绕过 native recipe factory。
        let switch_error = session
            // 请求未经 recipe 校验的 GPU 切换。
            .set_backend(BackendKind::Gpu)
            // GPU 通用切换必须失败。
            .expect_err("generic GPU session switch must fail");
        // 运行时切换使用与构造一致的稳定错误分类。
        assert_eq!(switch_error.code(), crate::core::Errc::InvalidArgument);
        // 失败切换不得替换当前可用后端。
        assert_eq!(session.backend_kind(), BackendKind::Cpu);
    }

    // 用 CPU surface 承载测试，但在语义型帧准备入口注入设备丢失。
    struct FailingPrepareBackend {
        // 复用已验证的 CPU DrawSurface，避免测试伪造绘制协议。
        cpu: CpuBackend,
    }

    // 提供失败后端的最小构造入口。
    impl FailingPrepareBackend {
        // 创建尚未初始化尺寸的 CPU backend。
        fn new() -> Self {
            // 返回只覆写 prepare_frame 的测试 owner。
            Self {
                // 其余后端行为全部委托给真实 CPU 实现。
                cpu: CpuBackend::new(),
            }
        }
    }

    // 为测试 owner 实现 RenderBackend 生命周期。
    impl RenderBackend for FailingPrepareBackend {
        // 保持 CPU backend 身份，证明准备 hook 不依赖 presentation 分支。
        fn kind(&self) -> BackendKind {
            // 返回与承载 surface 一致的后端类型。
            BackendKind::Cpu
        }

        // 复用真实 CPU capability 快照。
        fn capabilities(&self) -> BackendCapabilities {
            // 不为测试制造额外图形能力。
            self.cpu.capabilities()
        }

        // 把尺寸更新委托给真实 CPU backend。
        fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
            // 保持会话 initialize 的正常前置状态。
            self.cpu.resize(width, height)
        }

        // 把检查式销毁委托给真实 CPU backend。
        fn try_shutdown(&mut self) -> Result<(), Error> {
            // 测试结束时仍遵守统一 teardown 契约。
            self.cpu.try_shutdown()
        }

        // 返回真实 CPU 绘制表面。
        fn surface(&mut self) -> &mut dyn crate::draw::backend::DrawSurface {
            // 只有 prepare_frame 意图失败，surface 本身保持有效。
            self.cpu.surface()
        }

        // 在每帧准备边界注入可分类的设备丢失。
        fn prepare_frame(&mut self) -> Result<(), Error> {
            // 返回恢复层必须识别的 typed device-lost。
            Err(Error::new(
                // 使用生产恢复 FSM 的真实错误码。
                crate::core::Errc::GraphicsDeviceLost,
                // 保留稳定测试诊断。
                "test frame preparation device lost",
            ))
        }

        // 支持 RenderSession 的只读 backend downcast。
        fn as_any(&self) -> &dyn Any {
            // 返回测试 backend 自身。
            self
        }

        // 支持 RenderSession 的可变 backend downcast。
        fn as_any_mut(&mut self) -> &mut dyn Any {
            // 返回测试 backend 自身。
            self
        }
    }

    // 锁定 prepare_frame 的错误传播顺序与分类。
    #[test]
    fn begin_frame_propagates_prepare_failure_before_surface_work() {
        // 把失败 owner 注入真实 RenderSession。
        let mut session = RenderSession::with_backend(Box::new(FailingPrepareBackend::new()));
        // 先完成正常尺寸初始化，确保失败只来自帧准备。
        let initialized = session.initialize(8, 8);
        // 测试 backend 的真实 CPU surface 必须初始化成功。
        assert!(initialized.is_ok());
        // 开始一帧并捕获语义型准备结果。
        let outcome = session.begin_frame(UpdateStrategy::FullRedraw);
        // device-lost 必须保持 typed 分类，不能进入 surface 或伪装成普通失败。
        assert!(matches!(
            // 检查本次帧结果。
            outcome,
            // 只接受恢复层识别的设备丢失分类。
            RenderOutcome::Failed(crate::draw::renderer::GraphicsFailure::DeviceLost(error))
                // 原始错误码必须完整保留。
                if error.code() == crate::core::Errc::GraphicsDeviceLost
        ));
    }
}
