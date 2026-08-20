    // 引入被测组件。
    use super::Watermark;
    // 引入测试颜色值。
    use crate::draw::Color;
    // 引入可定制主题 token。
    use crate::ui::theme::DesignTokens;

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
