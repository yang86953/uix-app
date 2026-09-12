// 各项自带原 cfg 门控（uix_gpu_parity_* 或含 test/生产分支的 any 组合），
// 在源文件模块作用域 include! 展开。

// 显式共享 parity 通过真实 Canvas WidgetRender 入口提交一条 UI 绘制命令。
#[cfg(any(uix_gpu_parity_vulkan, uix_gpu_parity_opengl, uix_gpu_parity_d3d11))]
pub(crate) fn render_shared_production_scene(
    draw_context: &mut crate::draw::painting::PaintContext<'_>,
    frame: Rect,
    rect: Rect,
    color: crate::draw::Color,
) {
    let tree = WidgetTree::new();
    let mut context = PaintContext::new(draw_context, tree.theme_tokens());
    let widget = Canvas {
        size: Size::new(frame.w, frame.h),
        paint: Box::new(move |_frame, context| context.fill_rect(rect, color, None)),
    };
    WidgetRender::render(&widget, frame, &mut context, &tree);
}
