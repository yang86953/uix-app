    use super::icon_char;

    /// 全量表中已知名称应解析为具体字形（而非 fallback 的 search）。
    #[test]
    fn icon_char_resolves_known_names() {
        // 抽查常见与历史保留名称；search 本身字形即兜底字符，单独断言。
        assert_eq!(icon_char("search"), "\u{E151}");
        for name in ["home", "bell", "layout-dashboard", "terminal"] {
            let glyph = icon_char(name);
            assert_ne!(glyph, "\u{E151}", "{name} 不应落入 search 兜底");
        }
    }

    /// 未知名称应落入 search 兜底字形。
    #[test]
    fn icon_char_falls_back_for_unknown_names() {
        assert_eq!(icon_char("definitely-not-an-icon"), "\u{E151}");
    }
