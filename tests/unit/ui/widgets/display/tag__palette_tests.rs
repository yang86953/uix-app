    // 引入被测颜色解析入口与预设枚举。
    use super::{TagColor, resolve_tag_colors};
    // 引入标准明暗主题。
    use crate::ui::Theme;

    // 品牌 token 与扩展色阶必须分别响应主题变化。
    #[test]
    fn tag_palette_follows_theme_mode() {
        // 构造标准亮色主题。
        let light = Theme::antd_light();
        // 构造标准暗色主题。
        let dark = Theme::antd_dark();
        // 蓝色预设必须直接使用亮色主题品牌色对。
        let light_blue = resolve_tag_colors(TagColor::Blue, light.tokens());
        // 核对品牌背景 token。
        assert_eq!(light_blue.0, light.tokens().color_primary_bg());
        // 核对品牌前景 token。
        assert_eq!(light_blue.1, light.tokens().color_primary());
        // 分别解析扩展青色的明暗色对。
        let light_cyan = resolve_tag_colors(TagColor::Cyan, light.tokens());
        // 解析暗色主题下的同一扩展色。
        let dark_cyan = resolve_tag_colors(TagColor::Cyan, dark.tokens());
        // 扩展色背景必须随明暗模式变化。
        assert_ne!(light_cyan.0, dark_cyan.0);
        // 亮色背景必须保持低强调的浅色表面。
        assert!(light_cyan.0.is_light());
        // 暗色背景必须切换为深色表面。
        assert!(!dark_cyan.0.is_light());
    }
