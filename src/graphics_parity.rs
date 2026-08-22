//! 显式 GPU parity 的跨 System 测试组合根。
//!
//! 这里只负责把 UI、Drawing 与具体 Adapter 的测试能力组装起来；场景语义、
//! FramePlan lowering 和 Vulkan 原生执行仍由各自唯一责任方持有。

use crate::draw::backend::production_chain_parity::execute_ui_production_chain;
use crate::draw::backend::rhi_renderer::consistency::{
    production_chain_scene, validate_production_chain_readback,
};
use crate::native::presentation::graphics::vulkan::platform::VulkanContext;
use crate::platform::presentation::GraphicsContextLifecycle;

// 在真实 Vulkan device 上验收 UI → Drawing → FramePlan → Adapter 全链。
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
