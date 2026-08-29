//! 显式 GPU parity 的跨 System 测试组合根。
//!
//! 这里只负责把 UI、Drawing 与具体 Adapter 的测试能力组装起来；场景语义、
//! FramePlan lowering 和原生执行仍由各自唯一责任方持有。

use std::cell::Cell;
use std::time::{Duration, Instant};

use crate::app::window::window_creation::create_app_window;
use crate::core::Errc;
use crate::diagnostics::PendingFailureQueue;
#[cfg(feature = "vulkan-parity-test")]
use crate::draw::backend::production_chain_parity::execute_ui_production_chain;
use crate::draw::backend::production_chain_parity::execute_ui_production_surface_chain_with_present_hook;
use crate::draw::backend::rhi_renderer::consistency::validate_production_chain_readback;
use crate::draw::backend::rhi_renderer::consistency::{
    ProductionChainScene, production_chain_scene,
};
#[cfg(all(target_os = "linux", feature = "opengl-parity-test"))]
use crate::native::presentation::graphics::opengl::platform::OpenGlWsiParityAdapter;
#[cfg(feature = "vulkan-parity-test")]
use crate::native::presentation::graphics::vulkan::platform::{
    VulkanHeadlessUiParityAdapter, VulkanWsiParityAdapter,
};
use crate::platform::presentation::GraphicsContextLifecycle;
use crate::platform::presentation::rhi::{
    GraphicsContextRhi, GraphicsSurface, HeadlessUiParityAdapter, RhiExtent, RhiScissor,
    RhiSurfaceReadback, SurfaceToken, WsiParityAdapter, WsiParityFramePresenter,
};
use crate::platform::windowing::event::{UiEvent, UiEventPayload};
use crate::platform::{PendingNativeOptions, create_platform_with_pending};

// 在真实 headless device 上验收 UI → Drawing → FramePlan → Adapter 全链。
#[cfg(feature = "vulkan-parity-test")]
fn run_headless_ui_production_chain_test<A: HeadlessUiParityAdapter>() {
    let scene = production_chain_scene();
    let mut context = A::create_context(scene.extent)
        .expect("a production graphics device is required for UI parity");
    let adapter = A::diagnostic(&context);
    let frame = scene.frame;
    let rect = scene.rect;
    let color = scene.color;
    let target = execute_ui_production_chain(context.device(), scene.extent, |draw_context| {
        crate::ui::widgets::combinators::render_shared_production_scene(
            draw_context,
            frame,
            rect,
            color,
        );
    })
    .expect("UI production scene must execute through the shared Drawing FramePlan");
    let pixels = A::readback(&mut context, target);
    let invariant_count = validate_production_chain_readback(&scene, &pixels)
        .expect("production-chain readback must satisfy the shared Drawing invariants");
    context
        .try_shutdown()
        .expect("production-chain fixture must shut down cleanly");
    eprintln!(
        "GPU production chain verified: {adapter}; path=UI Canvas::render -> Drawing PaintContext/Canvas2D -> shared FramePlan -> GraphicsDevice adapter; draw-readback={invariant_count}/{}",
        scene.samples.len(),
    );
}

// 显式 API 入口只选择 Adapter 实现，具体 fixture 与诊断留在 native 边界。
#[cfg(feature = "vulkan-parity-test")]
pub(crate) fn run_vulkan_ui_production_chain_test() {
    run_headless_ui_production_chain_test::<VulkanHeadlessUiParityAdapter>();
}

// 共享 presenter 唯一拥有 UI 场景进入 Drawing Surface 桥与中立 readback 判定。
struct SharedUiSurfacePresenter<'a> {
    scene: &'a ProductionChainScene,
    // 性能采样帧关闭 parity 回读，保持与普通生产提交相同的 Surface 工作量。
    verify_readback: bool,
}

impl WsiParityFramePresenter for SharedUiSurfacePresenter<'_> {
    fn present(
        &mut self,
        context: &mut dyn GraphicsContextRhi,
        before_present: Option<&mut dyn FnMut(&mut dyn GraphicsSurface)>,
    ) -> crate::core::Result<SurfaceToken> {
        let mut adapter_hook = before_present;
        let mut readback = None;
        let presented = execute_ui_production_surface_chain_with_present_hook(
            context,
            |draw_context| {
                crate::ui::widgets::combinators::render_shared_production_scene(
                    draw_context,
                    self.scene.frame,
                    self.scene.rect,
                    self.scene.color,
                );
            },
            &mut |surface| {
                if let Some(hook) = adapter_hook.as_mut() {
                    (**hook)(surface);
                }
                if self.verify_readback && surface.surface_capabilities().readback {
                    readback = Some(surface.read_surface_pixels(RhiScissor {
                        x: 0,
                        y: 0,
                        width: self.scene.extent.width as i32,
                        height: self.scene.extent.height as i32,
                    }));
                }
            },
        )?;
        if let Some(readback) = readback {
            let readback = readback?;
            let invariant_count = validate_wsi_readback(self.scene, readback);
            eprintln!(
                "WSI Surface readback verified: generation={}; region={}x{}; invariants={}/{}; subsequent-present=ok",
                presented.generation,
                self.scene.extent.width,
                self.scene.extent.height,
                invariant_count,
                self.scene.samples.len(),
            );
        }
        Ok(presented)
    }
}

// 把统一 0xAARRGGBB 结果转换为共享场景断言消费的 RGBA8 字节。
fn validate_wsi_readback(scene: &ProductionChainScene, readback: RhiSurfaceReadback) -> usize {
    assert_eq!(readback.region.x, 0);
    assert_eq!(readback.region.y, 0);
    assert_eq!(readback.region.width, scene.extent.width as i32);
    assert_eq!(readback.region.height, scene.extent.height as i32);
    let pixels = readback.into_pixels();
    let mut rgba = Vec::with_capacity(pixels.len().saturating_mul(4));
    for pixel in pixels {
        rgba.extend_from_slice(&[
            ((pixel >> 16) & 0xff) as u8,
            ((pixel >> 8) & 0xff) as u8,
            (pixel & 0xff) as u8,
            ((pixel >> 24) & 0xff) as u8,
        ]);
    }
    validate_production_chain_readback(scene, &rgba)
        .expect("WSI final Surface pixels must satisfy shared Drawing invariants")
}

// 仅由 parity 组合根选择的事件泵与连续生产帧观测计划。
#[derive(Clone, Copy)]
struct WsiPacingObservationPlan {
    production_presents: usize,
    total_timeout: Duration,
}

// 所有真实 WSI 验收共用同一有界 owner-thread 八帧观测计划。
const WSI_PACING_OBSERVATION_PLAN: WsiPacingObservationPlan = WsiPacingObservationPlan {
    production_presents: 8,
    total_timeout: Duration::from_secs(15),
};

// 事件只由现有 Platform event loop 交给 parity 根观察，不建立第二套原生队列。
#[derive(Default)]
struct WsiEventObservation {
    dispatches: Cell<usize>,
    events: Cell<usize>,
    resize_events: Cell<usize>,
    latest_resize: Cell<Option<(i32, i32)>>,
}

impl WsiEventObservation {
    fn observe(&self, event: &UiEvent) -> bool {
        self.events.set(self.events.get().saturating_add(1));
        if let UiEventPayload::Resize(resize) = &event.payload {
            self.resize_events
                .set(self.resize_events.get().saturating_add(1));
            self.latest_resize.set(Some((resize.width, resize.height)));
        }
        true
    }
}

// 复用生产 Platform owner 的 timeout dispatch，等待条件成立或总 deadline 到期。
fn dispatch_wsi_events_until(
    platform: &mut dyn crate::platform::platform::PlatformSystem,
    observation: &WsiEventObservation,
    deadline: Instant,
    context: &str,
    satisfied: impl Fn(&WsiEventObservation) -> bool,
) {
    while !satisfied(observation) {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .unwrap_or_default();
        assert!(
            !remaining.is_zero(),
            "real platform event dispatch timed out during {context}"
        );
        // 该时长只是平台 poll 的有界等待片，不用于推断刷新率或模拟垂直同步。
        let dispatch_timeout = remaining.min(Duration::from_millis(100));
        let running = platform
            .event_loop()
            .wait_timeout(dispatch_timeout, &|event| observation.observe(event));
        observation
            .dispatches
            .set(observation.dispatches.get().saturating_add(1));
        assert!(running, "real platform event loop exited during {context}");
        if let Some(error) = platform.take_pending_failure() {
            panic!("real platform event dispatch failed during {context}: {error}");
        }
    }
}

// 从同代 logical resize 事实和初始整数 buffer scale 计算原生 drawable extent。
fn resize_event_drawable_extent(
    initial: SurfaceToken,
    initial_logical: (i32, i32),
    resized_logical: (i32, i32),
) -> RhiExtent {
    assert!(initial_logical.0 > 0 && initial_logical.1 > 0);
    assert!(resized_logical.0 > 0 && resized_logical.1 > 0);
    let initial_logical_width = initial_logical.0 as u32;
    let initial_logical_height = initial_logical.1 as u32;
    assert_eq!(initial.extent.width % initial_logical_width, 0);
    assert_eq!(initial.extent.height % initial_logical_height, 0);
    let scale_x = initial.extent.width / initial_logical_width;
    let scale_y = initial.extent.height / initial_logical_height;
    assert_eq!(scale_x, scale_y, "platform buffer scale must be uniform");
    assert!(scale_x > 0, "platform buffer scale must be positive");
    RhiExtent::new(
        (resized_logical.0 as u32)
            .checked_mul(scale_x)
            .expect("resized drawable width must fit u32"),
        (resized_logical.1 as u32)
            .checked_mul(scale_y)
            .expect("resized drawable height must fit u32"),
    )
}

// 在真实平台窗口上复用同一 UI、Drawing、Surface 与 resize 验收事务。
fn run_wsi_production_chain_test<A: WsiParityAdapter>(
    mut before_profiled_frame: Option<&mut dyn FnMut()>,
    mut after_profiled_frame: Option<&mut dyn FnMut(Duration)>,
) {
    let scene = production_chain_scene();
    let mut presenter = SharedUiSurfacePresenter {
        scene: &scene,
        verify_readback: true,
    };
    let profile = A::profile();
    let backend = profile.backend_label();
    let pacing = profile
        .observe_platform_pacing()
        .then_some(WSI_PACING_OBSERVATION_PLAN);
    let logical_width = 96;
    let logical_height = 72;
    let deadline = pacing.map(|plan| Instant::now() + plan.total_timeout);
    let event_observation = WsiEventObservation::default();
    eprintln!("{backend} WSI stage: create-platform");
    let mut platform =
        create_platform_with_pending(PendingNativeOptions::new(PendingFailureQueue::new()))
            .expect("the current session must provide a production window platform");
    let mut window = create_app_window(
        platform.window_manager(),
        profile.window_title(),
        logical_width,
        logical_height,
        &crate::platform::windowing::WindowSurfaceRole::Toplevel,
    )
    .expect("the current session must create a real platform window");
    eprintln!("{backend} WSI stage: window-created");
    if let Some(deadline) = deadline {
        let dispatches_before = event_observation.dispatches.get();
        dispatch_wsi_events_until(
            platform.as_mut(),
            &event_observation,
            deadline,
            "initial platform configure",
            |observation| observation.dispatches.get() > dispatches_before,
        );
    }
    let native_surface = window.native_surface_ptr();
    assert!(
        !native_surface.is_null(),
        "the production window must expose a native WSI surface"
    );
    let mut context = A::create_context(native_surface, logical_width, logical_height)
        .unwrap_or_else(|error| {
            panic!("the production {backend} WSI context must initialize: {error}")
        });
    eprintln!("{backend} WSI stage: context-created");
    let adapter = A::diagnostic(&context);
    let initial = context.surface_ref().token();
    assert!(initial.extent.is_valid());
    let initial_logical = window.client_logical_extent();

    let first_present = presenter
        .present(&mut context, None)
        .expect("the first real WSI frame must acquire, render and present");
    eprintln!("{backend} WSI stage: first-present");
    assert_eq!(first_present, initial);
    window
        .show()
        .expect("the platform window must become visible after the first present");

    // 非法 extent 必须在任何原生 recreate 与共享状态提交前失败。
    let rejected = RhiExtent::new(0, first_present.extent.height);
    let rejected_error = context
        .surface()
        .resize(rejected)
        .expect_err("zero-width WSI resize must be rejected");
    assert_eq!(rejected_error.code(), Errc::InvalidArgument);
    assert_eq!(
        context.surface_ref().token(),
        first_present,
        "rejected resize must not commit generation or extent",
    );

    // 有节拍的 WSI 验收由真实 compositor configure 驱动 resize。
    let resized_logical = if let Some(deadline) = deadline {
        let resize_events_before = event_observation.resize_events.get();
        window
            .properties_mut()
            .maximize()
            .expect("the production platform window must request maximize");
        dispatch_wsi_events_until(
            platform.as_mut(),
            &event_observation,
            deadline,
            "maximized platform resize",
            |observation| {
                observation.resize_events.get() > resize_events_before
                    && observation
                        .latest_resize
                        .get()
                        .is_some_and(|extent| extent != initial_logical)
            },
        );
        let resized_logical = event_observation
            .latest_resize
            .get()
            .expect("real platform resize dispatch must carry a logical extent");
        // 与应用 WindowDriver 相同，消费已分发的 resize 事实并同步窗口平台状态。
        window
            .resize_notify(resized_logical.0, resized_logical.1)
            .expect("the dispatched resize must update the production platform window");
        Some(resized_logical)
    } else {
        None
    };
    // 原生 Surface resize 成功后必须推进共享代际。
    let requested = resized_logical.map_or_else(
        || {
            RhiExtent::new(
                first_present.extent.width + 16,
                first_present.extent.height + 12,
            )
        },
        |logical| resize_event_drawable_extent(first_present, initial_logical, logical),
    );
    assert_ne!(
        requested, first_present.extent,
        "real resize must change extent"
    );
    let resized = context
        .surface()
        .resize(requested)
        .expect("real WSI Surface resize must succeed");
    eprintln!("{backend} WSI stage: resized");
    assert_eq!(resized.extent, requested);
    assert_eq!(resized.generation, first_present.generation + 1);

    let second_present = presenter
        .present(&mut context, None)
        .expect("the resized WSI generation must acquire, render and present");
    eprintln!("{backend} WSI stage: second-present");
    assert_eq!(second_present, resized);

    let mut production_presents = 2usize;
    if let (Some(plan), Some(deadline)) = (pacing, deadline) {
        // 只有显式性能探针关闭逐帧 parity 回读，避免把诊断分配算入生产提交。
        presenter.verify_readback = before_profiled_frame.is_none();
        while production_presents < plan.production_presents {
            let dispatches_before = event_observation.dispatches.get();
            dispatch_wsi_events_until(
                platform.as_mut(),
                &event_observation,
                deadline,
                "paced production frame",
                |observation| observation.dispatches.get() > dispatches_before,
            );
            let present_result = if let (Some(before), Some(after)) = (
                before_profiled_frame.as_deref_mut(),
                after_profiled_frame.as_deref_mut(),
            ) {
                // 外部测试二进制在真实 submit/present 前开启全局分配计数。
                before();
                let frame_started = Instant::now();
                let result = presenter.present(&mut context, None);
                // 无论成功或失败都及时关闭计数，避免错误诊断污染样本。
                after(frame_started.elapsed());
                result
            } else {
                presenter.present(&mut context, None)
            };
            let presented =
                present_result.expect("the paced real WSI frame must acquire, render and present");
            assert_eq!(presented, resized);
            production_presents = production_presents.saturating_add(1);
        }
        assert!(
            Instant::now() < deadline,
            "paced production frames exceeded the WSI total deadline"
        );
        assert!(
            event_observation.dispatches.get() >= plan.production_presents,
            "paced production frames must each be preceded by real platform event dispatch",
        );
        // 恢复路径继续执行完整 readback 验收，不让性能探针削弱像素证据。
        presenter.verify_readback = true;
    }
    A::verify_surface_recovery(&mut context, second_present, &mut presenter);
    context
        .try_shutdown()
        .expect("the production WSI context must shut down before its native window");
    window
        .close()
        .expect("the native window must close after graphics shutdown");

    if let Some(resized_logical) = resized_logical {
        eprintln!(
            "{backend} WSI production chain verified: {adapter}; path=UI Canvas::render -> Drawing PaintContext/Canvas2D -> shared FramePlan/RHI -> native Surface acquire/render/present; first={}x{}@{}; rejected=0x{}:{:?},token-unchanged; resize-event={}x{}; resize-token={}x{}@{}; baseline-presents=2; paced-presents={}; production-presents={production_presents}; recovery-presents=2; platform-timeout-dispatches={}; dispatched-events={}; resize-events={}; shutdown=ok; checked-window-close=ok; timing-claim=bounded-platform-dispatch-only",
            first_present.extent.width,
            first_present.extent.height,
            first_present.generation,
            first_present.extent.height,
            rejected_error.code(),
            resized_logical.0,
            resized_logical.1,
            resized.extent.width,
            resized.extent.height,
            resized.generation,
            production_presents.saturating_sub(2),
            event_observation.dispatches.get(),
            event_observation.events.get(),
            event_observation.resize_events.get(),
        );
    } else {
        eprintln!(
            "{backend} WSI production chain verified: {adapter}; path=UI Canvas::render -> Drawing PaintContext/Canvas2D -> shared FramePlan/RHI -> native Surface acquire/render/present; first={}x{}@{}; rejected=0x{}:{:?},token-unchanged; resized={}x{}@{}; baseline-presents=2; shutdown=ok",
            first_present.extent.width,
            first_present.extent.height,
            first_present.generation,
            first_present.extent.height,
            rejected_error.code(),
            resized.extent.width,
            resized.extent.height,
            resized.generation,
        );
    }
}

// 具体 API 入口只选择 Adapter，实现差异不回流到共享组合根。
#[cfg(feature = "vulkan-parity-test")]
pub(crate) fn run_vulkan_wsi_production_chain_test() {
    run_wsi_production_chain_test::<VulkanWsiParityAdapter>(None, None);
}

#[cfg(all(target_os = "linux", feature = "opengl-parity-test"))]
pub(crate) fn run_opengl_wsi_production_chain_test() {
    run_wsi_production_chain_test::<OpenGlWsiParityAdapter>(None, None);
}

// 仅为真实 paced frame 性能测试开放测量边界，不暴露任何原生 Adapter。
#[cfg(all(target_os = "linux", feature = "opengl-parity-test"))]
pub(crate) fn run_opengl_wsi_production_chain_profile(
    before_profiled_frame: &mut dyn FnMut(),
    after_profiled_frame: &mut dyn FnMut(Duration),
) {
    run_wsi_production_chain_test::<OpenGlWsiParityAdapter>(
        Some(before_profiled_frame),
        Some(after_profiled_frame),
    );
}
