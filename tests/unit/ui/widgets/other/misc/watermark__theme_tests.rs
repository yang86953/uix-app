// 引入被测组件。
use super::Watermark;
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
}

// 栈字符编码必须完整保留 ASCII、中文与四字节 Unicode 标量。
#[test]
fn stack_glyph_encoding_preserves_unicode() {
    // 逐类覆盖 UTF-8 的一、三与四字节编码宽度。
    for character in ['A', '水', '🦀'] {
        let mut buffer = [0_u8; 4];
        let glyph = Watermark::encode_glyph(character, &mut buffer);
        // 生产绘制收到的文本必须与原字符完全一致。
        assert_eq!(glyph, character.to_string());
    }
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
