// 引入字体族与统一样式契约。
use super::{super::Style, FontFamily};

// 字体族列表必须保留顺序、空格名称并拒绝非法项。
#[test]
fn font_family_preserves_order_and_rejects_invalid_names() {
    // 构造含空格名称和 generic family 的有效列表。
    let family = FontFamily::from_names([" Segoe UI ", "Arial", "sans-serif"])
        // 三项都合法时必须成功。
        .expect("有效字体族列表必须构造成功");
    // 外围空白应去除且源码顺序保持不变。
    assert_eq!(
        family.iter().collect::<Vec<_>>(),
        ["Segoe UI", "Arial", "sans-serif"]
    );
    // 空列表不能形成显式字体族声明。
    assert!(FontFamily::from_names::<[&str; 0], &str>([]).is_none());
    // 空名称不能被静默删除。
    assert!(FontFamily::from_names(["Arial", " "]).is_none());
    // 控制字符不能进入字体注册表查询。
    assert!(FontFamily::from_names(["Arial\nBold"]).is_none());
}

// 显式列表必须完整覆盖继承列表而不是拼接。
#[test]
fn font_family_explicit_list_overrides_inherited_list() {
    // 构造基础字体族列表。
    let base = Style::default().with_font_family(
        // 基础列表优先 Arial。
        FontFamily::from_names(["Arial", "sans-serif"]).expect("基础列表有效"),
    );
    // 构造显式覆盖列表。
    let overlay = Style::default().with_font_family(
        // 覆盖列表只选择 Segoe UI。
        FontFamily::from_names(["Segoe UI"]).expect("覆盖列表有效"),
    );
    // 合并必须使用覆盖列表。
    let merged = base.apply(overlay);
    // 不得把基础回退项拼接到覆盖列表后。
    assert_eq!(
        merged
            .font_family
            .expect("显式列表存在")
            .iter()
            .collect::<Vec<_>>(),
        ["Segoe UI"]
    );
}
