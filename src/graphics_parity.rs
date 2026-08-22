//! 显式 GPU parity 的跨 System 测试组合根。
//!
//! 这里只负责把 UI、Drawing 与具体 Adapter 的测试能力组装起来；场景语义、
//! FramePlan lowering 和 Vulkan 原生执行仍由各自唯一责任方持有。

use std::ffi::c_void;

use crate::app::window::window_creation::create_app_window;
use crate::core::Errc;
use crate::diagnostics::PendingFailureQueue;
#[cfg(feature = "vulkan-parity-test")]
use crate::draw::backend::production_chain_parity::execute_ui_production_chain;
use crate::draw::backend::production_chain_parity::execute_ui_production_surface_chain;
use crate::draw::backend::rhi_renderer::consistency::production_chain_scene;
#[cfg(feature = "vulkan-parity-test")]
use crate::draw::backend::rhi_renderer::consistency::validate_production_chain_readback;
#[cfg(all(target_os = "linux", feature = "opengl-parity-test"))]
use crate::native::presentation::graphics::opengl::platform::EglContext;
#[cfg(feature = "vulkan-parity-test")]
use crate::native::presentation::graphics::vulkan::platform::VulkanContext;
use crate::platform::presentation::GraphicsContextLifecycle;
use crate::platform::presentation::rhi::{GraphicsContextRhi, RhiExtent};
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

// 在真实平台窗口上复用同一 UI、Drawing、Surface 与 resize 验收事务。
fn run_wsi_production_chain_test<C>(
    backend: &'static str,
    window_title: &'static str,
    create_context: impl FnOnce(*mut c_void, i32, i32) -> crate::core::Result<C>,
    adapter_diagnostic: impl FnOnce(&C) -> String,
) where
    C: GraphicsContextRhi + GraphicsContextLifecycle,
{
    let scene = production_chain_scene();
    let logical_width = 96;
    let logical_height = 72;
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

    let frame = scene.frame;
    let rect = scene.rect;
    let color = scene.color;
    let first_present = execute_ui_production_surface_chain(&mut context, |draw_context| {
        crate::ui::widgets::combinators::render_shared_production_scene(
            draw_context,
            frame,
            rect,
            color,
        );
    })
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

    // 当前会话 WSI 可可靠触发显式 swapchain resize，成功后必须推进共享代际。
    let requested = RhiExtent::new(
        first_present.extent.width + 16,
        first_present.extent.height + 12,
    );
    let resized = context
        .surface()
        .resize(requested)
        .expect("real WSI Surface resize must succeed");
    eprintln!("{backend} WSI stage: resized");
    assert_eq!(resized.extent, requested);
    assert_eq!(resized.generation, first_present.generation + 1);

    let second_present = execute_ui_production_surface_chain(&mut context, |draw_context| {
        crate::ui::widgets::combinators::render_shared_production_scene(
            draw_context,
            frame,
            rect,
            color,
        );
    })
    .expect("the resized WSI generation must acquire, render and present");
    eprintln!("{backend} WSI stage: second-present");
    assert_eq!(second_present, resized);
    context
        .try_shutdown()
        .expect("the production WSI context must shut down before its native window");
    window
        .close()
        .expect("the native window must close after graphics shutdown");

    eprintln!(
        "{backend} WSI production chain verified: {adapter}; path=UI Canvas::render -> Drawing PaintContext/Canvas2D -> shared FramePlan/RHI -> native Surface acquire/render/present; first={}x{}@{}; rejected=0x{}:{:?},token-unchanged; resized={}x{}@{}; presents=2; shutdown=ok",
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

// 在真实平台窗口上验收 Vulkan WSI acquire → render → present 与 resize 生命周期。
#[cfg(feature = "vulkan-parity-test")]
pub(crate) fn run_vulkan_wsi_production_chain_test() {
    run_wsi_production_chain_test(
        "Vulkan",
        "UIX Vulkan WSI parity",
        |native_surface, width, height| VulkanContext::new(native_surface, width, height),
        |context| context.parity_adapter_diagnostic(),
    );
}

// 在真实 Wayland 窗口上验收 EGL/GLES 对同一 Surface 生产链的机械复用。
#[cfg(all(target_os = "linux", feature = "opengl-parity-test"))]
pub(crate) fn run_opengl_wsi_production_chain_test() {
    run_wsi_production_chain_test(
        "OpenGL ES",
        "UIX OpenGL ES WSI parity",
        |native_surface, width, height| {
            let context = EglContext::new(native_surface, width, height)?;
            context.disable_swap_interval_for_parity_test()?;
            Ok(context)
        },
        |_| {
            let display = std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "<unset>".to_owned());
            format!("Wayland display={display}; EGL window surface; GLES context")
        },
    );
}
