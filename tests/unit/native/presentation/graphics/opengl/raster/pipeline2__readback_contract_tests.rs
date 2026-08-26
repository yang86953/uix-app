// 引入同一私有 adapter 的待测辅助函数。
use super::{
    gl_readback_y_from_top, normalize_rgba_readback_pixels, reverse_readback_rows,
    validate_readback_region,
};
// 引入稳定错误分类。
use crate::core::Errc;

// 锁定所有 OpenGL window surface 使用同一左上到左下坐标换算。
#[test]
fn maps_top_left_region_to_opengl_surface_origin() {
    // 八行 surface 中从顶部第二行开始的三行区域应映射到 GL 第三行。
    assert_eq!(gl_readback_y_from_top(8, 2, 3), 3);
}

// 锁定垂直翻转只改变行顺序，不改变单行像素顺序。
#[test]
fn reverses_readback_rows_without_reversing_columns() {
    // 构造三行、每行两个像素的 bottom-up 载荷。
    let mut pixels = [1u32, 2, 3, 4, 5, 6];
    // 把 bottom-up 行序原地规范化为 top-left。
    reverse_readback_rows(&mut pixels, 2);
    // 最底行应移到末尾，同时每行左右像素保持原顺序。
    assert_eq!(pixels, [5, 6, 3, 4, 1, 2]);
}

// 锁定 OpenGL RGBA 字节与 D3D11 BGRA backbuffer 产生同一 packed 像素。
#[test]
fn normalizes_rgba_bytes_to_argb_pixel_values() {
    // 模拟驱动按内存顺序写入 R=11、G=22、B=33、A=44。
    let mut pixels = [u32::from_ne_bytes([0x11, 0x22, 0x33, 0x44])];
    // 执行 Adapter 私有的通道规范化。
    normalize_rgba_readback_pixels(&mut pixels);
    // 公共 readback 必须得到数值 0xAARRGGBB。
    assert_eq!(pixels, [0x4411_2233]);
}

// 锁定越界区域在调用 OpenGL 前返回 typed 参数错误。
#[test]
fn rejects_readback_region_outside_drawable() {
    // 构造纵向越过八行 drawable 的区域。
    let error = validate_readback_region(0, 7, 4, 2, 8, 8)
        // 越界输入必须失败。
        .expect_err("out-of-range readback must fail before the driver call");
    // 失败必须保持为调用方可修正的无效参数分类。
    assert_eq!(error.code(), Errc::InvalidArgument);
}
