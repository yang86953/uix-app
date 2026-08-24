// 引入被测二维码及其私有编码矩阵。
use super::QRCode;
// 引入公开 View 构建入口。
use crate::ui::view::View;

// 验证 UIX 视觉声明保持二维码单叶节点、编码结果与作者尺寸优先级。
#[test]
fn uix_root_preserves_qrcode_kernel_and_encoding() {
    // 构建已完成高纠错等级编码的二维码。
    let expected = QRCode::new("https://example.com")
        .size(192.0)
        .error_level(3);
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
    // 显式尺寸必须覆盖 UIX 声明的 Rust 直构默认尺寸。
    assert_eq!(kernel.size, 192.0);
    assert!(kernel.size_authored);
    // UIX 声明必须真实注入模块色、错误文案与字体角色。
    assert!(std::ptr::eq(kernel.visual, super::QRCODE_VISUAL_REF));
    assert!(
        std::mem::size_of::<super::QRCodeVisual>()
            > std::mem::size_of::<&'static super::QRCodeVisual>()
    );
    assert_eq!(kernel.visual.error_label, "QR !");
}

// 验证未显式设置尺寸时采用 UIX 声明的默认边长。
#[test]
fn uix_root_injects_qrcode_default_size() {
    let node = View::build(QRCode::new("default-size"));
    let kernel = node
        .widget
        .as_any()
        .downcast_ref::<QRCode>()
        .expect("UIX 根必须保留 QRCode 内核");
    assert_eq!(kernel.size, kernel.visual.default_size);
    assert_eq!(kernel.size, 160.0);
    assert!(!kernel.size_authored);
}

// 验证连续区间合并逐位保持二维码矩阵，并确实减少绘制提交数量。
#[test]
fn dark_run_stream_reconstructs_exact_module_matrix() {
    // 使用包含定位图案与数据区的有效二维码矩阵。
    let code = QRCode::new("https://example.com/uix").error_level(2);
    let side = code.module_count();
    let mut reconstructed = vec![false; side * side];
    let mut run_count = 0;
    // 按生产路径相同的区间流重建每个深色模块。
    code.for_each_dark_run(|y, start_x, run_length| {
        assert!(run_length > 0);
        assert!(start_x + run_length <= side);
        run_count += 1;
        for x in start_x..start_x + run_length {
            reconstructed[y * side + x] = true;
        }
    });
    // 合并后的矩形并集必须逐位等于编码器产生的权威矩阵。
    assert_eq!(reconstructed, code.modules);
    // 典型二维码存在连续深色模块，提交数必须少于逐模块绘制。
    let dark_modules = code.modules.iter().filter(|module| **module).count();
    assert!(
        run_count < dark_modules,
        "runs={run_count}, dark={dark_modules}"
    );
}
