// 引入待验证的排版组件与主题颜色值。
use super::*;
// 引入预设主题和语义角色。
use crate::ui::{NeutralRole, PaletteColor, Theme};

// 验证排版语义颜色由当前主题令牌解析。
#[test]
fn semantic_color_resolves_against_current_theme_tokens() {
    // 创建亮色主题令牌快照。
    let light = Theme::antd_light().tokens_arc();
    // 创建暗色主题令牌快照。
    let dark = Theme::antd_dark().tokens_arc();
    // 配置次要文字语义色。
    let secondary = Typography::text("secondary")
        // 使用中性次要文字角色。
        .semantic_color(ColorValue::neutral(NeutralRole::TextSecondary));
    // 亮色主题必须解析为自身的次要文字令牌。
    assert_eq!(
        // 调用组件内部绘制颜色解析边界。
        secondary.resolved_text_color(&secondary.visual.resolve(light.as_ref()), light.as_ref()),
        // 读取亮色主题期望值。
        light.color_text_secondary()
    );
    // 暗色主题必须重新解析而不是复用亮色 RGB。
    assert_eq!(
        // 使用同一声明式组件解析暗色主题。
        secondary.resolved_text_color(&secondary.visual.resolve(dark.as_ref()), dark.as_ref()),
        // 读取暗色主题期望值。
        dark.color_text_secondary()
    );
    // 配置危险语义色对应的错误色板角色。
    let danger = Typography::text("danger")
        // 使用主题错误色而非固定颜色常量。
        .semantic_color(ColorValue::palette(PaletteColor::Error));
    // 危险文字必须解析为当前主题错误色。
    assert_eq!(
        // 解析亮色主题危险色。
        danger.resolved_text_color(&danger.visual.resolve(light.as_ref()), light.as_ref()),
        // 读取亮色主题错误令牌。
        light.color_error()
    );
}

// 验证直接构造与 View 构建共享同目录 UIX 的唯一静态视觉地址。
#[test]
fn view_build_uses_uix_visual_without_a_rust_default_copy() {
    let typography = Typography::text("UIX Typography");
    assert!(std::ptr::eq(typography.visual, TYPOGRAPHY_VISUAL_REF));
    let node = crate::ui::view::View::build(typography);
    let typography = node
        .widget
        .as_any()
        .downcast_ref::<Typography>()
        .expect("UIX 根必须保留 Typography Rust 内核");
    assert!(std::ptr::eq(typography.visual, TYPOGRAPHY_VISUAL_REF));
}

// 验证 Style 行高优先于 Typography 专有段落 spacing。
#[test]
fn line_height_style_overrides_typography_spacing() {
    // 创建带两倍段落 spacing 的排版组件。
    let mut typography = Typography::paragraph("line height")
        // 既有专有入口先声明两倍行高。
        .spacing(2.0);
    // 未应用 Style 时保留专有 spacing。
    assert_eq!(typography.resolved_line_height(10.0), 20.0);
    // 构造固定十八像素的统一样式。
    let style = Style::default().with_line_height(
        // 正像素值必须构造成功。
        LineHeight::pixels(18.0).expect("正像素行高必须有效"),
    );
    // 通过 View 私有适配入口应用统一样式。
    typography.apply_view_style(&style);
    // 显式 Style 行高必须覆盖较早的 spacing。
    assert_eq!(typography.resolved_line_height(10.0), 18.0);
}

// 验证显式 Style 文本装饰覆盖既有局部构建器标记。
#[test]
fn text_decoration_style_explicit_none_overrides_typography_builders() {
    // 创建同时启用下划线和删除线的兼容组件。
    let mut typography = Typography::text("decoration")
        // 启用既有下划线入口。
        .underline()
        // 启用既有删除线入口。
        .delete();
    // 未应用 Style 时两个局部标记保持有效。
    assert!(typography.underline && typography.delete);
    // 构造显式关闭文本装饰的统一样式。
    let style = Style::default().with_text_decoration(TextDecoration::None);
    // 通过 View 适配边界应用统一样式。
    typography.apply_view_style(&style);
    // 显式 none 必须被保存，不能退化为未声明。
    assert_eq!(typography.text_decoration, Some(TextDecoration::None));
}

// 验证 View 适配边界把显式对齐交给 Typography。
#[test]
fn text_align_style_reaches_typography_layout() {
    // 创建保持默认左对齐的排版组件。
    let mut typography = Typography::paragraph("alignment");
    // 构造显式两端对齐的统一样式。
    let style = Style::default().with_text_align(TextAlign::Justify);
    // 通过 View 私有适配入口应用统一样式。
    typography.apply_view_style(&style);
    // 组件必须保存闭合对齐值供布局阶段消费。
    assert_eq!(typography.text_align, Some(TextAlign::Justify));
}

// 验证显式 normal 到达 Typography 并可覆盖 strong 兼容入口。
#[test]
fn font_weight_style_explicit_normal_reaches_strong_typography() {
    // 创建启用既有粗体入口的排版组件。
    let mut typography = Typography::text("weight").strong();
    // 构造显式常规字重样式。
    let style = Style::default().with_font_weight(FontWeight::NORMAL);
    // 通过 View 私有适配入口应用统一样式。
    typography.apply_view_style(&style);
    // 绘制阶段优先读取该显式值，因此不会退回 strong 粗体面。
    assert_eq!(typography.font_weight, Some(FontWeight::NORMAL));
}

// 验证有序字体族列表到达 Typography 布局边界。
#[test]
fn font_family_style_reaches_typography_layout() {
    // 创建使用当前系统字体的排版组件。
    let mut typography = Typography::text("family");
    // 构造包含首选与通用后备族的显式样式。
    let family = FontFamily::from_names(["Segoe UI", "sans-serif"])
        // 两项名称都合法时必须成功。
        .expect("字体族列表有效");
    // 通过统一样式保存列表。
    let style = Style::default().with_font_family(family.clone());
    // 通过 View 私有适配入口应用统一样式。
    typography.apply_view_style(&style);
    // 组件必须保存完整顺序供绘制布局选择。
    assert_eq!(typography.font_family, Some(family));
}
