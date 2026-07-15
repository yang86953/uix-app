use crate::draw::engine::bootstrap::assemble_graphics_engine;
use crate::draw::engine::{GraphicsFailure, RecoveringGraphicsEngine};
use crate::draw::traits::UpdateStrategy;
use crate::native::graphics::platform::windows as win_surface;
use crate::native::graphics::vulkan::platform::context::VulkanContext;
use crate::tests::common::*;
use crate::tests::native::gfx_r5::expected_gfx_r5_vendor;

const INITIAL_LOGICAL_EXTENT: (i32, i32) = (151, 113);
const RESIZED_LOGICAL_EXTENT: (i32, i32) = (229, 163);

#[test]
#[ignore = "requires a Vulkan-capable Windows driver and UIX_GFX_R5_EXPECT_VENDOR=nvidia|amd|intel"]
fn native_vulkan_surface_fault_reaches_engine_recovery_boundary() {
    let expected = expected_gfx_r5_vendor().unwrap_or_else(|error| panic!("{error}"));
    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window(
            "UIX GFX-R5 engine native fault",
            INITIAL_LOGICAL_EXTENT.0,
            INITIAL_LOGICAL_EXTENT.1,
        )
        .expect("window");
    window.show().expect("show window");
    let _ = platform.event_loop().poll_event(&|_| true);

    let surface = window.native_surface_ptr();
    assert!(!surface.is_null(), "Windows HWND must be available");
    let context = VulkanContext::new(surface, INITIAL_LOGICAL_EXTENT.0, INITIAL_LOGICAL_EXTENT.1)
        .expect("initial Vulkan context");
    expected.assert_runtime(
        &context.adapter_info,
        context.swapchain_maintenance1_enabled_for_test(),
    );
    let adapter = context.adapter_info.diagnostic_summary();
    let initial = assemble_graphics_engine(
        Box::new(context),
        INITIAL_LOGICAL_EXTENT.0,
        INITIAL_LOGICAL_EXTENT.1,
    )
    .unwrap_or_else(|failure| panic!("initial engine: {}", failure.into_error().what()));

    let actions = Rc::new(RefCell::new(Vec::new()));
    let recorded_actions = Rc::clone(&actions);
    let mut engine = RecoveringGraphicsEngine::new(
        initial,
        Box::new(move |action, width, height| {
            recorded_actions.borrow_mut().push(action);
            let context = VulkanContext::new(surface, width, height)?;
            expected.assert_runtime(
                &context.adapter_info,
                context.swapchain_maintenance1_enabled_for_test(),
            );
            assemble_graphics_engine(Box::new(context), width, height)
                .map_err(|failure| failure.into_error())
        }),
    )
    .with_extent(INITIAL_LOGICAL_EXTENT.0, INITIAL_LOGICAL_EXTENT.1);

    assert!(matches!(
        engine.begin_frame(UpdateStrategy::FullRedraw),
        RenderOutcome::FrameReady(_)
    ));
    assert!(matches!(
        engine.end_frame(&DamageRegion::full()),
        RenderOutcome::Present(_)
    ));

    window
        .properties_mut()
        .set_size(RESIZED_LOGICAL_EXTENT.0, RESIZED_LOGICAL_EXTENT.1)
        .expect("resize native window before engine resize");
    let drawable =
        win_surface::drawable_size(surface, RESIZED_LOGICAL_EXTENT.0, RESIZED_LOGICAL_EXTENT.1);
    assert!(matches!(
        engine.begin_frame(UpdateStrategy::FullRedraw),
        RenderOutcome::FrameReady(_)
    ));
    let fault = match engine.end_frame(&DamageRegion::full()) {
        RenderOutcome::Failed(GraphicsFailure::SurfaceLost(error)) => error,
        outcome => panic!("expected native surface fault, got {outcome:?}"),
    };
    assert!(
        fault.message().contains("vkAcquireNextImageKHR")
            || fault.message().contains("vkQueuePresentKHR"),
        "fault must originate in native WSI: {}",
        fault.what()
    );

    engine
        .resize(drawable.logical_width, drawable.logical_height)
        .expect("deliver native resize before recovery boundary");
    assert!(matches!(
        engine.begin_frame(UpdateStrategy::FullRedraw),
        RenderOutcome::FrameReady(_)
    ));
    assert_eq!(
        actions.borrow().as_slice(),
        [RecoveryAction::RebuildSurface]
    );
    assert_eq!(
        engine.logical_extent(),
        (drawable.logical_width, drawable.logical_height)
    );
    assert!(matches!(
        engine.end_frame(&DamageRegion::full()),
        RenderOutcome::Present(_)
    ));

    println!(
        "GFX-R5 engine recovery evidence: expected={}; action={:?}; fault={}; {}",
        expected.label(),
        RecoveryAction::RebuildSurface,
        fault.message(),
        adapter
    );
    engine.try_shutdown().expect("shutdown recovery engine");
    window.close().expect("close recovery window");
}
