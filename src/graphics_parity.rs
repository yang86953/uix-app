//! 显式 GPU parity 的跨 System 测试组合根。
//!
//! 这里只负责把 UI、Drawing 与具体 Adapter 的测试能力组装起来；场景语义、
//! FramePlan lowering 和 Vulkan 原生执行仍由各自唯一责任方持有。

use std::cell::Cell;
use std::ffi::c_void;
use std::time::{Duration, Instant};

use crate::app::window::window_creation::create_app_window;
use crate::core::Errc;
use crate::diagnostics::PendingFailureQueue;
#[cfg(feature = "vulkan-parity-test")]
use crate::draw::backend::production_chain_parity::execute_ui_production_chain;
use crate::draw::backend::production_chain_parity::execute_ui_production_surface_chain;
use crate::draw::backend::production_chain_parity::execute_ui_production_surface_chain_with_present_hook;
#[cfg(feature = "vulkan-parity-test")]
use crate::draw::backend::rhi_renderer::consistency::validate_production_chain_readback;
use crate::draw::backend::rhi_renderer::consistency::{
    ProductionChainScene, production_chain_scene,
};
#[cfg(all(target_os = "linux", feature = "opengl-parity-test"))]
use crate::native::presentation::graphics::opengl::platform::EglContext;
#[cfg(feature = "vulkan-parity-test")]
use crate::native::presentation::graphics::vulkan::platform::{
    VulkanContext, VulkanSurfaceFaultForParity,
};
use crate::platform::presentation::GraphicsContextLifecycle;
#[cfg(all(target_os = "linux", feature = "opengl-parity-test"))]
use crate::platform::presentation::rhi::GraphicsSurface;
use crate::platform::presentation::rhi::{GraphicsContextRhi, RhiExtent, SurfaceToken};
#[cfg(feature = "vulkan-parity-test")]
use crate::platform::presentation::rhi::{RhiScissor, RhiSurfaceReadback};
use crate::platform::windowing::event::{UiEvent, UiEventPayload};
use crate::platform::{PendingNativeOptions, create_platform_with_pending};

// 在真实 Vulkan device 上验收 UI → Drawing → FramePlan → Adapter 全链。
#[cfg(feature = "vulkan-parity-test")]
pub(crate) fn run_vulkan_ui_production_chain_test() {
    let scene = production_chain_scene();
    let mut context = VulkanContext::new_headless_for_parity_test(scene.extent)
        .expect("a production Vulkan graphics device is required for UI parity");
    let adapter = context.parity_adapter_diagnostic();
    let frame = scene.frame;
    let rect = scene.rect;
    let color = scene.color;
    let target = execute_ui_production_chain(&mut context, scene.extent, |draw_context| {
        crate::ui::widgets::combinators::render_shared_production_scene(
            draw_context,
            frame,
            rect,
            color,
        );
    })
    .expect("UI production scene must execute through the shared Drawing FramePlan");
    let pixels = context.readback_texture_for_parity_test(target);
    let invariant_count = validate_production_chain_readback(&scene, &pixels)
        .expect("Vulkan production-chain readback must satisfy the shared Drawing invariants");
    context
        .try_shutdown()
        .expect("Vulkan production-chain fixture must shut down cleanly");
    eprintln!(
        "Vulkan production chain verified: {adapter}; path=UI Canvas::render -> Drawing PaintContext/Canvas2D -> shared FramePlan -> VulkanContext GraphicsDevice; draw-readback={invariant_count}/{}",
        scene.samples.len(),
    );
}

// 所有 WSI 验收都从唯一共享场景进入同一 UI Surface 桥。
fn present_shared_production_scene(
    context: &mut dyn GraphicsContextRhi,
    scene: &ProductionChainScene,
) -> crate::core::Result<SurfaceToken> {
    execute_ui_production_surface_chain(context, |draw_context| {
        crate::ui::widgets::combinators::render_shared_production_scene(
            draw_context,
            scene.frame,
            scene.rect,
            scene.color,
        );
    })
}

// 让 API 中立的 parity 组合根复用唯一 UI 场景并观察 submit/present 边界。
#[cfg(all(target_os = "linux", feature = "opengl-parity-test"))]
fn present_shared_production_scene_with_hook(
    context: &mut dyn GraphicsContextRhi,
    scene: &ProductionChainScene,
    before_present: &mut dyn FnMut(&mut dyn GraphicsSurface),
) -> crate::core::Result<SurfaceToken> {
    execute_ui_production_surface_chain_with_present_hook(
        context,
        |draw_context| {
            crate::ui::widgets::combinators::render_shared_production_scene(
                draw_context,
                scene.frame,
                scene.rect,
                scene.color,
            );
        },
        before_present,
    )
}

// Vulkan WSI parity 只通过既有 before-present 钩子读取最终 swapchain image。
#[cfg(feature = "vulkan-parity-test")]
fn present_shared_production_scene_with_readback(
    context: &mut dyn GraphicsContextRhi,
    scene: &ProductionChainScene,
) -> crate::core::Result<(SurfaceToken, RhiSurfaceReadback)> {
    let mut readback = None;
    let presented = execute_ui_production_surface_chain_with_present_hook(
        context,
        |draw_context| {
            crate::ui::widgets::combinators::render_shared_production_scene(
                draw_context,
                scene.frame,
                scene.rect,
                scene.color,
            );
        },
        &mut |surface| {
            let capability = surface.surface_capabilities();
            readback = Some(if capability.readback {
                surface.read_surface_pixels(RhiScissor {
                    x: 0,
                    y: 0,
                    width: scene.extent.width as i32,
                    height: scene.extent.height as i32,
                })
            } else {
                Err(crate::core::Error::new(
                    Errc::NotImplemented,
                    "Vulkan WSI Surface did not report production readback support",
                ))
            });
        },
    )?;
    let readback = readback.ok_or_else(|| {
        crate::core::Error::new(
            Errc::InvalidState,
            "Vulkan WSI before-present hook did not produce a readback result",
        )
    })??;
    Ok((presented, readback))
}

// 只选择 parity 故障安排位置，不拥有恢复状态或原生生命周期。
#[cfg(all(target_os = "linux", feature = "opengl-parity-test"))]
#[derive(Clone, Copy, Debug)]
enum OpenGlSurfaceFaultForParity {
    Acquire,
    PresentAfterSubmit,
}

// 在同一真实 EGL window surface 上证明一次拒绝只触发一次原生 replacement。
#[cfg(all(target_os = "linux", feature = "opengl-parity-test"))]
fn verify_opengl_surface_fault(
    context: &mut EglContext,
    scene: &ProductionChainScene,
    fault: OpenGlSurfaceFaultForParity,
) -> SurfaceToken {
    let old = context.surface_ref().token();
    let replacements_before = context.window_surface_replacements_for_test();
    let injected = match fault {
        OpenGlSurfaceFaultForParity::Acquire => {
            context
                .surface()
                .inject_surface_lost_for_test()
                .expect("acquire SurfaceLost injection must arm at an idle boundary");
            assert_eq!(
                context.surface_ref().token(),
                old,
                "arming acquire SurfaceLost must not commit authoritative state",
            );
            present_shared_production_scene(context, scene)
        }
        OpenGlSurfaceFaultForParity::PresentAfterSubmit => {
            let mut armed_after_submit = false;
            let result =
                present_shared_production_scene_with_hook(context, scene, &mut |surface| {
                    surface
                        .inject_surface_lost_for_test()
                        .expect("present SurfaceLost injection must arm after submit");
                    assert_eq!(
                        surface.token(),
                        old,
                        "arming present SurfaceLost must not commit authoritative state",
                    );
                    armed_after_submit = true;
                });
            assert!(
                armed_after_submit,
                "present SurfaceLost must be armed after the shared RHI submit",
            );
            result
        }
    };

    let retry = injected.expect_err("SurfaceLost must reject the old OpenGL WSI frame");
    assert_eq!(retry.code(), Errc::GraphicsSurfaceChanged);
    let rebuilt = context.surface_ref().token();
    assert_eq!(rebuilt.extent, old.extent, "recovery must keep WSI extent");
    assert_eq!(
        rebuilt.generation,
        old.generation + 1,
        "recovery must commit exactly one shared generation",
    );
    assert_eq!(
        context.window_surface_replacements_for_test(),
        replacements_before + 1,
        "one SurfaceLost must replace the real EGLSurface exactly once",
    );

    let recovered = present_shared_production_scene(context, scene)
        .expect("the next real UI production frame must submit and eglSwapBuffers");
    assert_eq!(recovered, rebuilt);
    assert_eq!(
        context.window_surface_replacements_for_test(),
        replacements_before + 1,
        "the recovered frame must reuse the single replacement EGLSurface",
    );
    eprintln!(
        "OpenGL ES WSI recovery path: fault={fault:?}; old={}x{}@{}; new={}x{}@{}; injection-token=unchanged; retry=RetryFrame(GraphicsSurfaceChanged); egl-surface-replacements=1; recovered-submit=ok; recovered-eglSwapBuffers=ok@{}",
        old.extent.width,
        old.extent.height,
        old.generation,
        rebuilt.extent.width,
        rebuilt.extent.height,
        rebuilt.generation,
        recovered.generation,
    );
    rebuilt
}

// 两个拒绝边界共享同一个 EGL owner、故障注入、场景和恢复状态机。
#[cfg(all(target_os = "linux", feature = "opengl-parity-test"))]
fn verify_opengl_surface_recovery(
    context: &mut EglContext,
    scene: &ProductionChainScene,
    initial: SurfaceToken,
) {
    let replacements_before = context.window_surface_replacements_for_test();
    let acquired =
        verify_opengl_surface_fault(context, scene, OpenGlSurfaceFaultForParity::Acquire);
    let presented = verify_opengl_surface_fault(
        context,
        scene,
        OpenGlSurfaceFaultForParity::PresentAfterSubmit,
    );
    assert_eq!(acquired.generation, initial.generation + 1);
    assert_eq!(presented.generation, initial.generation + 2);
    assert_eq!(
        context.window_surface_replacements_for_test(),
        replacements_before + 2,
    );
    eprintln!(
        "OpenGL ES WSI Surface recovery verified: paths=2; generation={}->{}; real-egl-surface-replacements=2; recovered-ui-submits=2; recovered-eglSwapBuffers=2",
        initial.generation, presented.generation,
    );
}

// 把统一 0xAARRGGBB 结果转换为已有共享场景断言消费的 RGBA8 字节。
#[cfg(feature = "vulkan-parity-test")]
fn validate_vulkan_wsi_readback(
    scene: &ProductionChainScene,
    readback: RhiSurfaceReadback,
) -> usize {
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
        .expect("Vulkan WSI final Surface pixels must satisfy shared Drawing invariants")
}

// 仅由 parity 组合根选择的事件泵与连续生产帧观测计划。
#[derive(Clone, Copy)]
struct WsiPacingObservationPlan {
    production_presents: usize,
    total_timeout: Duration,
}

// 真实 OpenGL WSI 验收必须在同一有界 owner-thread 事务内完成八帧。
const OPENGL_WSI_PACING_PLAN: WsiPacingObservationPlan = WsiPacingObservationPlan {
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
    platform: &mut dyn crate::platform::platform::Platform,
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
        // 该时长只是 Wayland poll 的有界等待片，不用于推断刷新率或模拟垂直同步。
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
    assert_eq!(scale_x, scale_y, "Wayland buffer scale must be uniform");
    assert!(scale_x > 0, "Wayland buffer scale must be positive");
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
fn run_wsi_production_chain_test<C>(
    backend: &'static str,
    window_title: &'static str,
    pacing: Option<WsiPacingObservationPlan>,
    create_context: impl FnOnce(*mut c_void, i32, i32) -> crate::core::Result<C>,
    adapter_diagnostic: impl FnOnce(&C) -> String,
    mut present_surface_frame: impl FnMut(
        &mut C,
        &ProductionChainScene,
    ) -> crate::core::Result<SurfaceToken>,
    verify_surface_recovery: impl FnOnce(&mut C, &ProductionChainScene, SurfaceToken),
) where
    C: GraphicsContextRhi + GraphicsContextLifecycle,
{
    let scene = production_chain_scene();
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
        window_title,
        logical_width,
        logical_height,
    )
    .expect("the current session must create a real platform window");
    eprintln!("{backend} WSI stage: window-created");
    if let Some(deadline) = deadline {
        let dispatches_before = event_observation.dispatches.get();
        dispatch_wsi_events_until(
            platform.as_mut(),
            &event_observation,
            deadline,
            "initial Wayland configure",
            |observation| observation.dispatches.get() > dispatches_before,
        );
    }
    let native_surface = window.native_surface_ptr();
    assert!(
        !native_surface.is_null(),
        "the production window must expose a native WSI surface"
    );
    let mut context =
        create_context(native_surface, logical_width, logical_height).unwrap_or_else(|error| {
            panic!("the production {backend} WSI context must initialize: {error}")
        });
    eprintln!("{backend} WSI stage: context-created");
    let adapter = adapter_diagnostic(&context);
    let initial = context.surface_ref().token();
    assert!(initial.extent.is_valid());
    let initial_logical = window.client_logical_extent();

    let first_present = present_surface_frame(&mut context, &scene)
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

    // OpenGL 节拍验收由真实 compositor configure 驱动 resize；其它 WSI 保持原行为。
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
            "maximized Wayland resize",
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
            .expect("real Wayland resize dispatch must carry a logical extent");
        // 与应用 WindowDriver 相同，消费已分发的 resize 事实并同步窗口平台状态。
        window
            .resize_notify(resized_logical.0, resized_logical.1)
            .expect("the dispatched resize must update the production platform window");
        Some(resized_logical)
    } else {
        None
    };
    // 原生 EGL/Vulkan Surface resize 成功后必须推进共享代际。
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

    let second_present = present_surface_frame(&mut context, &scene)
        .expect("the resized WSI generation must acquire, render and present");
    eprintln!("{backend} WSI stage: second-present");
    assert_eq!(second_present, resized);

    let mut production_presents = 2usize;
    if let (Some(plan), Some(deadline)) = (pacing, deadline) {
        while production_presents < plan.production_presents {
            let dispatches_before = event_observation.dispatches.get();
            dispatch_wsi_events_until(
                platform.as_mut(),
                &event_observation,
                deadline,
                "paced production frame",
                |observation| observation.dispatches.get() > dispatches_before,
            );
            let presented = present_surface_frame(&mut context, &scene)
                .expect("the paced real WSI frame must acquire, render and present");
            assert_eq!(presented, resized);
            production_presents = production_presents.saturating_add(1);
        }
        assert!(
            Instant::now() < deadline,
            "paced production frames exceeded the WSI total deadline"
        );
    }
    verify_surface_recovery(&mut context, &scene, second_present);
    context
        .try_shutdown()
        .expect("the production WSI context must shut down before its native window");
    window
        .close()
        .expect("the native window must close after graphics shutdown");

    if let Some(resized_logical) = resized_logical {
        eprintln!(
            "{backend} WSI production chain verified: {adapter}; path=UI Canvas::render -> Drawing PaintContext/Canvas2D -> shared FramePlan/RHI -> native Surface acquire/render/present; first={}x{}@{}; rejected=0x{}:{:?},token-unchanged; resize-event={}x{}; resize-token={}x{}@{}; baseline-presents=2; paced-presents={}; production-presents={production_presents}; recovery-presents=2; platform-timeout-dispatches={}; dispatched-events={}; resize-events={}; shutdown=ok; checked-window-close=ok; timing-claim=nonzero-swap-policy-only",
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

// 在同一真实 Vulkan WSI owner 上逐项证明四个原生返回位置的恢复语义。
#[cfg(feature = "vulkan-parity-test")]
fn verify_vulkan_surface_fault(
    context: &mut VulkanContext,
    scene: &ProductionChainScene,
    fault: VulkanSurfaceFaultForParity,
) -> SurfaceToken {
    let old = context.surface_ref().token();
    context
        .inject_surface_fault_for_parity_test(fault)
        .unwrap_or_else(|error| {
            panic!("{fault:?} injection must arm at an idle boundary: {error}")
        });
    assert_eq!(
        context.surface_ref().token(),
        old,
        "arming {fault:?} must not commit authoritative Surface state",
    );

    let injected = present_shared_production_scene(context, scene);
    let rebuilt = context.surface_ref().token();
    assert_eq!(
        rebuilt.extent, old.extent,
        "{fault:?} must keep the real WSI extent"
    );
    assert_eq!(
        rebuilt.generation,
        old.generation + 1,
        "{fault:?} must commit exactly one recreated generation",
    );

    let semantics = match fault {
        VulkanSurfaceFaultForParity::AcquireOutOfDate
        | VulkanSurfaceFaultForParity::PresentOutOfDate => {
            let error = injected.expect_err("OUT_OF_DATE must reject the old frame");
            assert_eq!(error.code(), Errc::GraphicsSurfaceChanged);
            "RetryFrame(GraphicsSurfaceChanged)"
        }
        VulkanSurfaceFaultForParity::AcquireSuboptimal
        | VulkanSurfaceFaultForParity::PresentSuboptimal => {
            let presented = injected.expect("SUBOPTIMAL must keep the presented frame successful");
            assert_eq!(presented, rebuilt);
            assert_ne!(presented, old, "SUBOPTIMAL must not return the stale token");
            "Presented(Ok)"
        }
    };

    let recovered = present_shared_production_scene(context, scene)
        .unwrap_or_else(|error| panic!("{fault:?} next real WSI frame must present: {error}"));
    assert_eq!(recovered, rebuilt);
    eprintln!(
        "Vulkan WSI recovery path: fault={fault:?}; old={}x{}@{}; new={}x{}@{}; injected={semantics}; recovered-present=ok@{}",
        old.extent.width,
        old.extent.height,
        old.generation,
        rebuilt.extent.width,
        rebuilt.extent.height,
        rebuilt.generation,
        recovered.generation,
    );
    rebuilt
}

// 四条路径共享同一个真实窗口、GPU、场景、FramePlan 和生命周期 owner。
#[cfg(feature = "vulkan-parity-test")]
fn verify_vulkan_surface_recovery(
    context: &mut VulkanContext,
    scene: &ProductionChainScene,
    initial: SurfaceToken,
) {
    let mut current = initial;
    for fault in [
        VulkanSurfaceFaultForParity::AcquireOutOfDate,
        VulkanSurfaceFaultForParity::PresentOutOfDate,
        VulkanSurfaceFaultForParity::AcquireSuboptimal,
        VulkanSurfaceFaultForParity::PresentSuboptimal,
    ] {
        current = verify_vulkan_surface_fault(context, scene, fault);
    }
    assert_eq!(current.generation, initial.generation + 4);
    eprintln!(
        "Vulkan WSI Surface recovery verified: paths=4; generation={}->{}; recovery-ui-presents=4; injected-suboptimal-presents=2",
        initial.generation, current.generation,
    );
}

// 在真实平台窗口上验收 Vulkan WSI acquire → render → present 与 resize 生命周期。
#[cfg(feature = "vulkan-parity-test")]
pub(crate) fn run_vulkan_wsi_production_chain_test() {
    run_wsi_production_chain_test(
        "Vulkan",
        "UIX Vulkan WSI parity",
        None,
        |native_surface, width, height| VulkanContext::new(native_surface, width, height),
        |context| {
            let display = std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "<unset>".to_owned());
            let session =
                std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "<unset>".to_owned());
            format!(
                "Wayland display={display}; session={session}; {}; {}",
                context.parity_adapter_diagnostic(),
                context.parity_surface_diagnostic(),
            )
        },
        |context, scene| {
            let (presented, readback) =
                present_shared_production_scene_with_readback(context, scene)?;
            let invariant_count = validate_vulkan_wsi_readback(scene, readback);
            eprintln!(
                "Vulkan WSI Surface readback verified: generation={}; region={}x{}; invariants={}/{}; subsequent-present=ok",
                presented.generation,
                scene.extent.width,
                scene.extent.height,
                invariant_count,
                scene.samples.len(),
            );
            Ok(presented)
        },
        verify_vulkan_surface_recovery,
    );
}

// 在真实 Wayland 窗口上验收 EGL/GLES 对同一 Surface 生产链的机械复用。
#[cfg(all(target_os = "linux", feature = "opengl-parity-test"))]
pub(crate) fn run_opengl_wsi_production_chain_test() {
    run_wsi_production_chain_test::<EglContext>(
        "OpenGL ES",
        "UIX OpenGL ES WSI parity",
        Some(OPENGL_WSI_PACING_PLAN),
        |native_surface, width, height| EglContext::new(native_surface, width, height),
        |context| {
            let display = std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "<unset>".to_owned());
            let session =
                std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "<unset>".to_owned());
            let adapter = context
                .parity_adapter_diagnostic()
                .expect("the real EGL window context must report its GPU identity");
            format!("Wayland display={display}; session={session}; EGL window surface; {adapter}")
        },
        |context, scene| present_shared_production_scene(context, scene),
        verify_opengl_surface_recovery,
    );
}
