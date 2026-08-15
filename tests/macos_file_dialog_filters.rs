// 直接复用 macOS 文件对话框的纯过滤器解析组件，不加载 AppKit。
#[path = "../src/native/backends/macos/platform/file_dialog_filters.rs"]
mod file_dialog_filters;

// 引入被测解析入口。
use file_dialog_filters::parse_allowed_file_types;

// 验证常见分号与描述格式会提取扩展名并稳定去重。
#[test]
fn extracts_extensions_and_preserves_first_seen_order() {
    // 混合描述、空白、分号和重复大小写模式。
    let filters = "Images (*.png *.jpg);*.PNG|Documents (*.pdf)";
    // 解析为 AppKit 文件类型列表。
    let actual = parse_allowed_file_types(filters);
    // 大写描述不会误入，重复 png 只保留首次出现。
    assert_eq!(actual, vec!["png", "jpg", "pdf"]);
}

// 验证全通配符和空过滤器不会错误收窄 AppKit 面板。
#[test]
fn treats_empty_and_all_file_patterns_as_unrestricted() {
    // 空字符串不产生限制。
    assert!(parse_allowed_file_types("").is_empty());
    // 常见全文件模式也不产生限制。
    assert!(parse_allowed_file_types("All Files (*.*);*").is_empty());
}

// 验证 Win32 双 NUL 风格过滤器也能提取真实扩展名。
#[test]
fn accepts_win32_nul_separated_filter_payloads() {
    // 模拟 UIX Windows 后端现有的描述与模式交替格式。
    let filters = "Images\0*.png;*.jpeg\0Text\0*.txt\0\0";
    // 解析结果忽略描述和终止 NUL。
    assert_eq!(
        parse_allowed_file_types(filters),
        vec!["png", "jpeg", "txt"]
    );
}

// 验证裸小写类型与复合扩展名保持可用。
#[test]
fn accepts_bare_and_compound_file_types() {
    // 裸类型兼容 Linux 调用方，复合扩展名保留内部点号。
    assert_eq!(
        parse_allowed_file_types("png;tar.gz;public.jpeg"),
        vec!["png", "tar.gz", "public.jpeg"]
    );
}

// 验证路径模式与说明文字不会被误识别成扩展名。
#[test]
fn rejects_paths_and_descriptive_words() {
    // 路径和大写描述都不是 allowedFileTypes 输入。
    assert!(parse_allowed_file_types("Images;C:\\temp\\*.png;/tmp/*.jpg").is_empty());
}
