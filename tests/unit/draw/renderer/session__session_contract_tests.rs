// 复用当前模块的会话与后端类型。
use super::*;
// 引入 downcast 契约需要的 Any。
use std::any::Any;

// 构造只在第一次关闭时失败的 backend，验证切换不会覆盖旧 owner。
struct FailingShutdownBackend {
    // 复用真实 CPU surface，避免伪造绘制和尺寸协议。
    cpu: CpuBackend,
    // 记录检查式关闭边界的调用次数。
    shutdown_attempts: usize,
}

// 提供可确定重试的关闭失败 owner。
impl FailingShutdownBackend {
    // 创建尚未初始化且第一次关闭将失败的 backend。
    fn new() -> Self {
        // 返回持有唯一 CPU 资源 owner 的测试实例。
        Self {
            // 真实 CPU backend 承载其余生命周期。
            cpu: CpuBackend::new(),
            // 第一次 set_backend 前尚未调用关闭。
            shutdown_attempts: 0,
        }
    }
}

// 为关闭失败测试实现最小 RenderBackend 契约。
impl RenderBackend for FailingShutdownBackend {
    // 保持 CPU backend 身份，测试只改变 teardown 结果。
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
        // 保持会话初始化和失败后继续使用的真实 surface。
        self.cpu.resize(width, height)
    }

    // 第一次关闭返回 typed failure，随后允许 Drop 重试完成。
    fn try_shutdown(&mut self) -> Result<(), Error> {
        // 记录每次检查式 teardown 尝试。
        self.shutdown_attempts = self.shutdown_attempts.saturating_add(1);
        // 只让 set_backend 触发的第一次尝试失败。
        if self.shutdown_attempts == 1 {
            // 返回恢复层可分类的设备丢失。
            return Err(Error::new(
                // 使用真实 graphics recovery 错误码。
                crate::core::Errc::GraphicsDeviceLost,
                // 保留稳定的切换失败诊断。
                "test backend shutdown failed before owner switch",
            ));
        }
        // 后续重试检查式释放真实 CPU 资源。
        self.cpu.try_shutdown()
    }

    // 返回真实 CPU 绘制表面。
    fn surface(&mut self) -> &mut dyn crate::draw::backend::DrawSurface {
        // 测试 backend 继续拥有同一 surface。
        self.cpu.surface()
    }

    // 支持测试读取保留下来的具体 owner。
    fn as_any(&self) -> &dyn Any {
        // 返回测试 backend 自身。
        self
    }

    // 支持会话内部可变 downcast 契约。
    fn as_any_mut(&mut self) -> &mut dyn Any {
        // 返回测试 backend 自身。
        self
    }
}

// 锁定 backend 切换在旧 owner 关闭失败前不会提交。
#[test]
// 验证 typed teardown failure、owner 和 extent 同时保留。
fn backend_switch_retains_current_owner_when_shutdown_fails() {
    // 把可重试的关闭失败 owner 注入真实 RenderSession。
    let mut session = RenderSession::with_backend(Box::new(FailingShutdownBackend::new()));
    // 建立可观察的会话 extent 和真实 CPU surface。
    session
        // 初始化旧 backend 的资源。
        .initialize(8, 6)
        // 测试前置必须成功。
        .expect("test backend should initialize");

    // 请求切换到通用工厂可创建的新 CPU backend。
    let error = session
        // 进入唯一 owner 切换事务。
        .set_backend(BackendKind::Cpu)
        // 第一次旧 owner teardown 必须阻止提交。
        .expect_err("shutdown failure must reject backend switch");
    // 原始设备丢失分类必须直接返回给恢复层。
    assert_eq!(error.code(), crate::core::Errc::GraphicsDeviceLost);

    // 失败后仍应能取得原来的具体 backend owner。
    let retained = session
        // 通过只读 RenderBackend 契约观察当前实例。
        .backend()
        // 取得测试 owner 的类型化只读视图。
        .as_any()
        // 若候选覆盖了旧 owner，downcast 会失败。
        .downcast_ref::<FailingShutdownBackend>()
        // 明确报告 owner 被错误替换。
        .expect("failed switch must retain the current backend owner");
    // set_backend 只应执行一次旧 owner 关闭尝试。
    assert_eq!(retained.shutdown_attempts, 1);
    // 失败事务不得改写会话逻辑宽度。
    assert_eq!(session.width(), 8);
    // 失败事务不得改写会话逻辑高度。
    assert_eq!(session.height(), 6);
    // 读取保留 owner 的真实 surface 尺寸，排除只保留陈旧会话字段的假阳性。
    let retained_size = session
        // 重新借用当前 backend 的唯一可变 owner。
        .backend_mut()
        // 取得仍由旧 backend 持有的真实 CPU surface。
        .surface()
        // 复制稳定尺寸值，立即结束 surface 借用。
        .size();
    // 旧 surface 的实际宽度必须仍然可用。
    assert_eq!(retained_size.w, 8.0);
    // 旧 surface 的实际高度必须仍然可用。
    assert_eq!(retained_size.h, 6.0);
}

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
