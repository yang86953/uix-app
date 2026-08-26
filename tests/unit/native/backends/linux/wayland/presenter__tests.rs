// 引入 presenter 私有缩放函数。
use super::WaylandPresenter;

// scale=2 必须把每个逻辑像素扩展成 2×2 物理像素块。
#[test]
// 使用 2×2 输入验证行列映射均正确。
fn scale_pixels_expands_logical_pixels_to_drawable_blocks() {
    // 四个不同像素便于观察所有象限。
    let pixels = [1, 2, 3, 4];
    // 输出由 helper 一次分配。
    let mut output = Vec::new();
    // 执行 scale=2 的最近邻放大。
    WaylandPresenter::scale_pixels(&pixels, 2, 2, 2, &mut output)
        // 有效小尺寸不应失败。
        .expect("scale=2 像素映射必须成功");
    // 物理 4×4 行列应各复制两次。
    assert_eq!(output, vec![1, 1, 2, 2, 1, 1, 2, 2, 3, 3, 4, 4, 3, 3, 4, 4]);
}
