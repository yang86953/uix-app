// 引入被测二维码及其私有编码矩阵。
use super::QRCode;
// 引入公开 View 构建入口。
use crate::ui::view::View;

// 验证 UIX 声明壳保持原二维码单叶节点与编码结果。
#[test]
fn uix_root_preserves_qrcode_kernel_and_encoding() {
    // 构建已完成高纠错等级编码的二维码。
    let expected = QRCode::new("https://example.com").error_level(3);
    let expected_modules = expected.module_count();
    let node = View::build(expected);
    // UIX 声明不得增加包装或展示子节点。
    assert!(node.children.is_empty());
    // 运行时动态类型必须继续是拥有编码矩阵的 QRCode。
    let kernel = node
        .widget
        .as_any()
        .downcast_ref::<QRCode>()
        .expect("UIX 根必须保留 QRCode 内核");
    // 已编码内容、等级与矩阵边长必须无损进入同一内核。
    assert_eq!(kernel.value, "https://example.com");
    assert_eq!(kernel.error_level, 3);
    assert_eq!(kernel.module_count(), expected_modules);
    assert!(kernel.is_valid());
}
