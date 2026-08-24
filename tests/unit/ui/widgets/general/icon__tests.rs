use super::{ICON_VISUAL, Icon, IconFallbackLabel, icon_char};
use crate::ui::view::View;

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

/// UIX 声明必须融合进原 Icon 叶节点，并保留作者显式尺寸。
#[test]
fn uix_shell_applies_defaults_without_overwriting_authored_size() {
    // 默认实例通过 UIX 取得声明尺寸和共享视觉值。
    let default_node = Icon::new("home").build();
    assert!(default_node.children.is_empty());
    let default_icon = default_node
        .widget
        .as_any()
        .downcast_ref::<Icon>()
        .expect("UIX Icon 根必须保持 Icon 内核");
    assert_eq!(default_icon.size, 24.0);
    assert_eq!(default_icon.glyph, '\u{E0F5}');
    assert_eq!(ICON_VISUAL.glyph_scale, 0.85);
    assert_eq!(ICON_VISUAL.fallback_scale, 0.55);

    // 作者尺寸属于 Rust 输入，声明默认不得覆盖。
    let authored_node = Icon::new("bell").size(31.0).build();
    let authored_icon = authored_node
        .widget
        .as_any()
        .downcast_ref::<Icon>()
        .expect("显式尺寸 Icon 必须保持原内核");
    assert_eq!(authored_icon.size, 31.0);
    assert!(authored_icon.size_authored);
}

/// 字体缺失后备文本必须在栈缓冲内保持原 Unicode 大写语义。
#[test]
fn fallback_label_uses_bounded_utf8_buffer() {
    assert_eq!(IconFallbackLabel::new("").as_str(), "?");
    assert_eq!(IconFallbackLabel::new("search").as_str(), "S");
    assert_eq!(IconFallbackLabel::new("ßeta").as_str(), "SS");
}

/// 精确容量名称存储必须抵消缓存字形与作者标记，避免扩大主流 64 位实例。
#[test]
#[cfg(target_pointer_width = "64")]
fn cached_glyph_keeps_icon_instance_compact() {
    assert_eq!(std::mem::size_of::<Icon>(), 32);
}
