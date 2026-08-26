use super::*;
use crate::core::Errc;
use crate::draw::backend::cpu::noop_canvas_2d::NoopCanvas2D;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

struct StubTarget {
    frames: u32,
    resize_calls: Arc<AtomicUsize>,
    fail_next: bool,
    // 单独控制 backdrop 快照失败，验证 begin_frame 外的错误登记。
    fail_backdrop_snapshot: bool,
    shutdown: bool,
    canvas: NoopCanvas2D,
}

impl StubTarget {
    fn new() -> Self {
        Self {
            frames: 0,
            resize_calls: Arc::new(AtomicUsize::new(0)),
            fail_next: false,
            // 普通 target 不注入 backdrop 失败。
            fail_backdrop_snapshot: false,
            shutdown: false,
            canvas: NoopCanvas2D,
        }
    }

    fn new_failing() -> Self {
        Self {
            frames: 0,
            resize_calls: Arc::new(AtomicUsize::new(0)),
            fail_next: true,
            // 帧失败用例不同时注入 backdrop 失败。
            fail_backdrop_snapshot: false,
            shutdown: false,
            canvas: NoopCanvas2D,
        }
    }

    // 创建只在 backdrop 快照边界报告设备丢失的 target。
    fn new_backdrop_failing() -> Self {
        // 初始化独立故障 target。
        Self {
            // 尚未开始任何帧。
            frames: 0,
            // 独立记录该 target 的 resize 调用。
            resize_calls: Arc::new(AtomicUsize::new(0)),
            // begin_frame 自身保持成功。
            fail_next: false,
            // 下一次快照调用返回 typed failure。
            fail_backdrop_snapshot: true,
            // target 尚未 shutdown。
            shutdown: false,
            // 测试不需要真实画布。
            canvas: NoopCanvas2D,
        }
    }

    fn with_resize_counter() -> (Self, Arc<AtomicUsize>) {
        let target = Self::new();
        let counter = Arc::clone(&target.resize_calls);
        (target, counter)
    }
}

impl RenderTarget for StubTarget {
    fn initialize(&mut self, _width: i32, _height: i32) -> Result<(), Error> {
        Ok(())
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        self.shutdown = true;
        Ok(())
    }

    fn resize(&mut self, _width: i32, _height: i32) -> Result<(), Error> {
        self.resize_calls.fetch_add(1, AtomicOrdering::SeqCst);
        Ok(())
    }

    fn begin_frame(&mut self, _strategy: UpdateStrategy) -> RenderOutcome {
        self.frames += 1;
        if self.fail_next {
            self.fail_next = false;
            return RenderOutcome::Failed(GraphicsFailure::DeviceLost(Error::new(
                Errc::GraphicsDeviceLost,
                "stub device lost",
            )));
        }
        RenderOutcome::FrameReady(DamageRegion::full())
    }

    fn end_frame(&mut self, _damage: &DamageRegion) -> RenderOutcome {
        RenderOutcome::Present(DamageRegion::full())
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        &mut self.canvas
    }

    // 在专用用例中从 begin_frame 外注入设备丢失。
    fn snapshot_overlay_backdrop(&mut self) -> Result<bool, Error> {
        // 只让专用构造器触发故障。
        if self.fail_backdrop_snapshot {
            // 返回恢复 FSM 可识别的稳定设备丢失错误。
            return Err(Error::new(
                // 保留真实 GraphicsDeviceLost 分类。
                Errc::GraphicsDeviceLost,
                // 提供可诊断的测试文本。
                "stub overlay backdrop device lost",
            ));
        }
        // 普通测试 target 不支持 GPU backdrop。
        Ok(false)
    }
}

fn counting_rebuilder(rebuilds: &Arc<AtomicUsize>) -> RenderTargetRebuilder {
    let rebuilds = Arc::clone(rebuilds);
    Box::new(
        move |_action: GraphicsRecoveryAction, _width: i32, _height: i32| {
            rebuilds.fetch_add(1, AtomicOrdering::SeqCst);
            Ok(Box::new(StubTarget::new()) as Box<dyn RenderTarget>)
        },
    )
}

// platform 已完成的 Surface 重建只重试脏帧，不得再次触发整后端恢复。
#[test]
fn surface_changed_does_not_schedule_backend_rebuild() {
    let rebuilds = Arc::new(AtomicUsize::new(0));
    let mut driver =
        RecoveryDriver::new(Box::new(StubTarget::new()), counting_rebuilder(&rebuilds))
            .with_extent(100, 100);
    driver.record_failure(GraphicsFailure::from_error(Error::new(
        Errc::GraphicsSurfaceChanged,
        "surface generation advanced",
    )));

    assert!(driver.pending_failure.is_none());
    assert!(matches!(
        driver.begin_frame(UpdateStrategy::FullRedraw),
        RenderOutcome::FrameReady(_)
    ));
    assert_eq!(rebuilds.load(AtomicOrdering::SeqCst), 0);
}

// 零尺寸由窗口调度器保持暂停，恢复层不得向底层提交 1x1 resize。
#[test]
fn zero_extent_resize_does_not_touch_native_target() {
    let rebuilds = Arc::new(AtomicUsize::new(0));
    let (target, resize_calls) = StubTarget::with_resize_counter();
    let mut driver =
        RecoveryDriver::new(Box::new(target), counting_rebuilder(&rebuilds)).with_extent(100, 100);

    driver.resize(0, 100).expect("zero extent enters pause");

    assert_eq!(resize_calls.load(AtomicOrdering::SeqCst), 0);
    assert_eq!(rebuilds.load(AtomicOrdering::SeqCst), 0);
}

// 验证持续的 probe/Present 矛盾会升级为 surface-lost，而不是永久遮挡循环。
#[test]
fn repeated_present_probe_conflicts_escalate_surface_loss() {
    // 创建仅用于满足恢复包装器构造的计数器。
    let rebuilds = Arc::new(AtomicUsize::new(0));
    // 构造不主动失败的底层 target，直接测试恢复器的协议观测状态。
    let mut driver = RecoveryDriver::new(
        // 底层 target 不主动返回图形失败。
        Box::new(StubTarget::new()),
        // rebuilder 仅满足恢复包装器的构造契约。
        counting_rebuilder(&rebuilds),
    );
    // 连续模拟 probe 可用后真实 Present 仍返回遮挡。
    for conflict in 1..=MAX_PRESENT_PROBE_CONFLICTS {
        // 构造无数据 probe 的结构化可呈现结果。
        let probe_result = Ok(PresentTestResult::Presentable);
        // 先登记 probe 事实，保持与生产调用顺序一致。
        driver.observe_present_test(&probe_result);
        // 再登记紧随其后的真实 Present 遮挡失败。
        driver.record_failure(GraphicsFailure::Occluded(Error::new(
            // 使用正式遮挡错误码，不依赖 D3D11 私有类型。
            Errc::GraphicsOccluded,
            // 提供稳定的测试诊断文本。
            "stub present remained occluded",
        )));
        // 未达到门槛时仍按健康遮挡处理。
        if conflict < MAX_PRESENT_PROBE_CONFLICTS {
            // 瞬时竞态不得提前进入恢复 FSM。
            assert!(driver.pending_failure.is_none());
        }
    }
    // 达到门槛后必须登记 surface-lost，供下一帧执行有界恢复。
    assert!(matches!(
        // 读取恢复器保存的首个未处理失败。
        driver.pending_failure,
        // 矛盾门槛必须转换为正式 surface-lost 分类。
        Some(GraphicsFailure::SurfaceLost(_))
    ));
    // 协议矛盾应把下一次恢复动作直接推进到 Software。
    assert_eq!(
        // 使用刚登记的 typed failure 驱动同一恢复状态机。
        driver
            .recovery
            .on_failure(driver.pending_failure.as_ref().unwrap()),
        // 不应再尝试另一条可能同样不可见的 GPU swapchain。
        GraphicsRecoveryAction::UseSoftware
    );
}

#[test]
fn rebuild_request_runs_bounded_recovery_at_next_frame_boundary() {
    let request = RebuildRequest::default();
    let rebuilds = Arc::new(AtomicUsize::new(0));
    let mut driver =
        RecoveryDriver::new(Box::new(StubTarget::new()), counting_rebuilder(&rebuilds))
            .with_extent(100, 100)
            .with_rebuild_request(request.clone());

    // 无请求时正常推进，不触发重建。
    assert!(matches!(
        driver.begin_frame(UpdateStrategy::FullRedraw),
        RenderOutcome::FrameReady(_)
    ));
    assert_eq!(rebuilds.load(AtomicOrdering::SeqCst), 0);

    // 请求后下一帧执行有界恢复序列（RebuildSurface → rebuilder 一次）。
    request.request_rebuild();
    let outcome = driver.begin_frame(UpdateStrategy::FullRedraw);
    assert!(
        matches!(outcome, RenderOutcome::FrameReady(_)),
        "recovery should rebuild and continue, got {outcome:?}"
    );
    assert_eq!(rebuilds.load(AtomicOrdering::SeqCst), 1);

    // 请求是一次性的：后续帧不再重复恢复。
    let outcome = driver.begin_frame(UpdateStrategy::FullRedraw);
    assert!(matches!(outcome, RenderOutcome::FrameReady(_)));
    assert_eq!(rebuilds.load(AtomicOrdering::SeqCst), 1);
}

#[test]
fn rebuild_request_joins_an_already_pending_failure_without_double_recovery() {
    let request = RebuildRequest::default();
    let rebuilds = Arc::new(AtomicUsize::new(0));
    let mut driver =
        RecoveryDriver::new(Box::new(StubTarget::new()), counting_rebuilder(&rebuilds))
            .with_extent(100, 100)
            .with_rebuild_request(request.clone());

    // 先注入一次真实帧失败：本帧直接失败并保留 pending failure。
    let failing = Box::new(StubTarget::new_failing());
    driver.engine = failing;
    let outcome = driver.begin_frame(UpdateStrategy::FullRedraw);
    assert!(matches!(outcome, RenderOutcome::Failed(_)));
    assert_eq!(rebuilds.load(AtomicOrdering::SeqCst), 0);

    // 恢复请求被消费，但不覆盖首个未处理失败，也不产生第二次重建：
    // 下一帧只执行一次有界恢复（针对原始 device-lost 失败）。
    request.request_rebuild();
    let outcome = driver.begin_frame(UpdateStrategy::FullRedraw);
    assert!(
        matches!(outcome, RenderOutcome::FrameReady(_)),
        "recovery should rebuild once and continue, got {outcome:?}"
    );
    assert_eq!(rebuilds.load(AtomicOrdering::SeqCst), 1);

    // 请求是一次性的：后续帧不再重复恢复。
    let outcome = driver.begin_frame(UpdateStrategy::FullRedraw);
    assert!(matches!(outcome, RenderOutcome::FrameReady(_)));
    assert_eq!(rebuilds.load(AtomicOrdering::SeqCst), 1);
}

// 验证 backdrop typed failure 会登记并在下一帧执行有界恢复。
#[test]
fn backdrop_failure_runs_bounded_recovery_at_next_frame_boundary() {
    // 记录 rebuilder 的实际调用次数。
    let rebuilds = Arc::new(AtomicUsize::new(0));
    // 用只在 backdrop 快照失败的底层 target 构造恢复包装器。
    let mut driver = RecoveryDriver::new(
        // 注入 begin_frame 外的设备丢失。
        Box::new(StubTarget::new_backdrop_failing()),
        // 使用可计数的成功 rebuilder。
        counting_rebuilder(&rebuilds),
    )
    // 提供重建所需的稳定 surface extent。
    .with_extent(100, 100);
    // 先触发 backdrop 快照失败并保留原始错误码。
    let error = driver
        // 调用新增 typed lifecycle 边界。
        .snapshot_overlay_backdrop()
        // 测试 target 必须返回失败。
        .expect_err("backdrop snapshot should report device lost");
    // 确认错误没有退化为不支持。
    assert_eq!(error.code(), Errc::GraphicsDeviceLost);
    // 失败发生当帧不应立即重建。
    assert_eq!(rebuilds.load(AtomicOrdering::SeqCst), 0);
    // 下一帧边界消费 pending failure 并重建。
    let outcome = driver.begin_frame(UpdateStrategy::FullRedraw);
    // 重建后应继续返回可绘制帧。
    assert!(matches!(outcome, RenderOutcome::FrameReady(_)));
    // 有界恢复只调用一次 rebuilder。
    assert_eq!(rebuilds.load(AtomicOrdering::SeqCst), 1);
}
