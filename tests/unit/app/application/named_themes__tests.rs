// 引入父模块主题表。
use super::*;

// 验证文档同名主题覆盖内建预设。
#[test]
fn document_theme_overrides_builtin_name() {
    // 创建包含内建主题的默认表。
    let mut themes = NamedThemes::default();
    // 用暗色值覆盖 light 名称。
    themes.insert("light".to_string(), Theme::antd_dark());
    // 精确名称应返回覆盖后的暗色值。
    assert!(themes.resolve("light").expect("light 应存在").is_dark());
    // 未登记名称必须保持失败而非静默回退。
    assert!(themes.resolve("missing").is_none());
}
