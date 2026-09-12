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
        paint: Box::new(move |_frame, context| {
            use crate::draw::{Color, GradientStop, LinearGradient, Radius};
            use crate::ui::theme::style::{Style, BoxShadowDef};
            context.fill_rect(rect, color, None);
            let red = Color::from_rgb(255, 0, 0);
            let green = Color::from_rgb(0, 255, 0);
            let blue = Color::from_rgb(0, 0, 255);
            let gradient = LinearGradient::new(90.0, &[
                GradientStop::new(0.0, red), GradientStop::new(0.5, green), GradientStop::new(1.0, blue),
            ]).unwrap();
            context.fill_linear_gradient_stops(Rect::new(32.0, 4.0, 16.0, 8.0), gradient, None);
            let hard = LinearGradient::new(90.0, &[
                GradientStop::new(0.0, red), GradientStop::new(0.5, red),
                GradientStop::new(0.5, blue), GradientStop::new(1.0, blue),
            ]).unwrap();
            context.fill_linear_gradient_stops(Rect::new(32.0, 16.0, 16.0, 8.0), hard, None);
            let background = Color::from_rgb(16, 32, 48);
            context.fill_rect(Rect::new(54.0, 4.0, 20.0, 20.0), background, None);
            let shadow_style = Style::default().with_shadows(vec![
                BoxShadowDef::new(Color::from_rgba(255, 0, 0, 128), 0.0, 0.0, 0.0),
                BoxShadowDef::new(Color::from_rgba(0, 0, 255, 128), 0.0, 0.0, 0.0),
            ]);
            crate::ui::style_paint::apply_style(context, Rect::new(58.0, 8.0, 10.0, 10.0), &shadow_style);
            context.fill_rect(Rect::new(32.0, 28.0, 16.0, 12.0), background, None);
            let translucent = LinearGradient::new(45.0, &[
                GradientStop::new(0.0, Color::from_rgba(255, 0, 0, 128)),
                GradientStop::new(0.5, Color::from_rgba(0, 255, 0, 128)),
                GradientStop::new(1.0, Color::from_rgba(0, 0, 255, 128)),
            ]).unwrap();
            context.save();
            context.set_opacity(0.5);
            context.push_clip(Rect::new(32.0, 28.0, 14.0, 12.0));
            context.fill_linear_gradient_stops(Rect::new(32.0, 28.0, 16.0, 12.0), translucent, Some(Radius::uniform(4.0)));
            context.pop_clip();
            context.restore();
            let mut sixteen = [GradientStop::new(0.0, Color::BLACK); 16];
            for (i, stop) in sixteen.iter_mut().enumerate() { stop.offset = i as f32 / 15.0; }
            sixteen[15].color = Color::WHITE;
            context.fill_linear_gradient_stops(Rect::new(54.0, 28.0, 16.0, 12.0), LinearGradient::new(90.0, &sixteen).unwrap(), None);
        }),
    };
    WidgetRender::render(&widget, frame, &mut context, &tree);
}
