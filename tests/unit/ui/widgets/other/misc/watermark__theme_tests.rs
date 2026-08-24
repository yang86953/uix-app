// 引入被测组件。
use super::{WATERMARK_VISUAL_REF, Watermark};
// 引入测试颜色值。
use crate::draw::Color;
// 引入可定制主题 token。
use crate::ui::theme::DesignTokens;
// 引入公开 View 构建入口。
use crate::ui::view::View;

// UIX 声明根必须保持原水印单叶节点与平铺配置。
#[test]
fn uix_root_preserves_watermark_kernel() {
    // 构建带作者透明度与旋转角度的水印。
    let node = View::build(Watermark::new("内部资料").opacity(0.2).rotate(-30.0));
    // UIX 声明不得增加包装或展示子节点。
    assert!(node.children.is_empty());
    // 根动态类型必须继续是拥有平铺与绘制机制的 Watermark。
    let kernel = node
        .widget
        .as_any()
        .downcast_ref::<Watermark>()
        .expect("UIX 根必须保留 Watermark 内核");
    // 作者配置必须无损进入同一内核。
    assert_eq!(kernel.text, "内部资料");
    assert_eq!(kernel.opacity, 0.2);
    assert_eq!(kernel.rotate, -30.0);
    // UIX 声明必须进入真实绘制内核，而不是只保留根节点壳。
    assert_eq!(
        kernel.visual_contract_for_test(),
        (0.15, -22.0, 200.0, 160.0, 1.4, 2)
    );
}

// UIX 默认值只填充未显式设置的字段，并由所有实例共享。
#[test]
fn uix_defaults_preserve_authored_values_and_share_visual_configuration() {
    // 构建前默认值与构建后内核必须指向同一份 UIX 静态视觉事实。
    assert!(std::ptr::eq(
        Watermark::new("构建前").visual,
        WATERMARK_VISUAL_REF
    ));
    let authored = View::build(
        Watermark::new("作者水印")
            .opacity(0.2)
            .rotate(-30.0)
            .gap(220.0, 180.0)
            .offset(5.0, 7.0),
    );
    let defaults = View::build(Watermark::new("默认水印"));
    let authored = authored
        .widget
        .as_any()
        .downcast_ref::<Watermark>()
        .expect("作者水印必须保留 Watermark 内核");
    let defaults = defaults
        .widget
        .as_any()
        .downcast_ref::<Watermark>()
        .expect("默认水印必须保留 Watermark 内核");
    assert_eq!(
        (
            authored.opacity,
            authored.rotate,
            authored.gap_x,
            authored.gap_y,
            authored.x_offset,
            authored.y_offset,
        ),
        (0.2, -30.0, 220.0, 180.0, 5.0, 7.0)
    );
    assert_eq!(
        (
            defaults.opacity,
            defaults.rotate,
            defaults.gap_x,
            defaults.gap_y,
            defaults.x_offset,
            defaults.y_offset,
        ),
        (0.15, -22.0, 200.0, 160.0, 0.0, 0.0)
    );
    assert!(authored.shares_visual_with_for_test(defaults));
    assert!(std::ptr::eq(defaults.visual, WATERMARK_VISUAL_REF));
}

// 每个平铺实例必须按完整文本行绘制，避免逐字符命令破坏整形并放大布局次数。
#[test]
fn tiled_render_records_full_lines_under_shared_rotation() {
    use crate::draw::backend::cpu::noop_canvas_2d::NoopCanvas2D;
    use crate::draw::painting::{DisplayList, PaintContext as DrawPaintContext, PaintOp};
    use crate::draw::resources::font::font_service::FontService;
    use crate::ui::Theme;
    use crate::ui::widget_runtime::paint_context::PaintContext as UiPaintContext;
    use crate::ui::widget_runtime::traits::WidgetRender;

    let watermark = Watermark::new("AB\n水🦀");
    let tree = crate::ui::WidgetTree::new();
    let mut canvas = NoopCanvas2D;
    let font_service = FontService::new();
    let image_service = crate::draw::resources::image::ImageService::new();
    let mut draw_context = DrawPaintContext::new_for_test(
        &mut canvas,
        crate::draw::FontHandle::new(0),
        &font_service,
        &image_service,
        96.0,
        1.0,
        crate::draw::geometry::spatial::Orientation::YDown,
        32,
        32,
    );
    let mut list = DisplayList::new();
    let tokens = Theme::antd_light().tokens_arc();
    draw_context.with_recorder(&mut list, |draw_context| {
        let mut ui_context = UiPaintContext::new(draw_context, tokens);
        WidgetRender::render(
            &watermark,
            crate::core::Rect::new(0.0, 0.0, 20.0, 20.0),
            &mut ui_context,
            &tree,
        );
    });

    let lines = list
        .ops()
        .iter()
        .filter_map(|operation| match operation {
            PaintOp::DrawText { text, .. } => Some(text.as_ref()),
            _ => None,
        })
        .collect::<Vec<_>>();
    // 20x20 区域按 1 个可见单元加 2 个 UIX 外扩单元，共 3x3 个平铺实例。
    assert_eq!(lines.len(), 18);
    assert!(lines.iter().all(|line| *line == "AB" || *line == "水🦀"));
    let transforms = list
        .ops()
        .iter()
        .filter_map(|operation| match operation {
            PaintOp::SetTransform { transform } => Some(transform),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(transforms.len(), 9);
    assert!(transforms.iter().all(|transform| {
        transform.m[1].abs() > f32::EPSILON && transform.m[3].abs() > f32::EPSILON
    }));
}

// 默认主题值与显式作者值必须保持正确优先级。
#[test]
fn watermark_defaults_follow_theme_tokens() {
    // 构造可定制的完整主题 token。
    let mut tokens = DesignTokens::antd_light();
    // 覆写正文字号以证明默认值不是固定 14px。
    tokens.font_size = 18.0;
    // 构造默认水印。
    let themed = Watermark::new("主题水印");
    // 默认字号必须来自当前主题。
    assert_eq!(themed.resolved_font_size(&tokens), 18.0);
    // 构造不透明测试正文色。
    let theme_text = Color::from_rgb(1, 2, 3);
    // 默认颜色必须从当前主题正文色派生并应用默认 opacity。
    assert_eq!(
        themed.effective_color_for_test(theme_text),
        theme_text.with_alpha(38)
    );
    // 显式作者值必须覆盖主题默认值。
    let customized = Watermark::new("自定义水印")
        // 设置显式颜色。
        .color(Color::from_rgb(4, 5, 6))
        // 设置显式字号。
        .font_size(20.0);
    // 显式字号不得被主题重写。
    assert_eq!(customized.resolved_font_size(&tokens), 20.0);
    // 显式颜色不得被主题重写。
    assert_eq!(
        customized.effective_color_for_test(theme_text),
        Color::from_rgb(4, 5, 6).with_alpha(38)
    );
}
