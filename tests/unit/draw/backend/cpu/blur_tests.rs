//! CPU Picture blur 与后续 Additive 合成回归。

// 引入被测 CPU 后端。
use super::CpuBackend;
// 引入矩形与错误码，核验可见区域和 typed failure。
use crate::core::{Errc, Rect};
// 引入后端契约以调用 checked 离屏入口。
use crate::draw::backend::contract::RenderBackend;
// 引入 Picture 合成使用的实际混合模式。
use crate::draw::geometry::types::{BlendMode, ImageHandle};
// 引入画布状态接口以读取保存后的 offset。
use crate::draw::Canvas2D;

// 对一个预乘像素应用与生产路径相同的有限 opacity。
fn apply_opacity(pixel: u32, opacity: f32) -> u32 {
    // 复用共享软件执行器的逐通道量化规则。
    crate::draw::raster::rasterizer::apply_opacity(pixel, opacity)
}

// 计算 premultiplied Additive 的逐通道饱和参考值。
fn additive_reference(destination: u32, source: u32) -> u32 {
    // 对指定通道执行与 SoftwareRasterizer 相同的饱和加法。
    let add = |shift: u32| {
        // 先提取两端通道，再把结果限制到八位范围。
        (((destination >> shift) & 0xff) + ((source >> shift) & 0xff)).min(0xff)
    };
    // 按 AARRGGBB 顺序重新组装参考像素。
    (add(24) << 24) | (add(16) << 16) | (add(8) << 8) | add(0)
}

// 验证真实 blur 结果会按目标当前 Additive、opacity 与 clip 合成到主表面。
#[test]
fn blurred_picture_blit_preserves_main_additive_state() {
    // 创建独立 CPU 后端。
    let mut backend = CpuBackend::new();
    // 建立五像素主表面供区域边界和扩散像素共同验证。
    RenderBackend::resize(&mut backend, 5, 1).expect("main surface should resize");
    // 创建同尺寸 Picture 离屏目标。
    let handle = RenderBackend::try_create_offscreen(&mut backend, 5, 1)
        // 检查式 CPU 分配不应返回 typed failure。
        .expect("checked offscreen allocation should succeed")
        // 测试尺寸必须可分配。
        .expect("offscreen should allocate");
    // 构造中心脉冲，使 blur 后相邻像素得到非零贡献。
    let impulse = [0, 0, 0x8040_0000, 0, 0];
    // 取得离屏像素所有者。
    let offscreen = backend
        // 使用内部池直接准备确定性的预乘输入。
        .offscreens
        // 目标刚创建，必须仍然存在。
        .get_mut(&handle)
        // 缺失目标表示池生命周期已破坏。
        .expect("offscreen should exist");
    // 写入单点红色预乘脉冲。
    offscreen
        // 取得内部像素表面。
        .surface_mut()
        // 借用紧密排列的像素缓冲。
        .pixels_mut()
        // 用完整输入替换透明初值。
        .copy_from_slice(&impulse);
    // 对完整 Picture 区域执行生产 CPU 高斯模糊。
    RenderBackend::try_blur_offscreen(
        // 修改同一离屏资源。
        &mut backend,
        // 指定刚写入的 Picture。
        &handle,
        // 模糊整个五像素区域。
        Rect::new(0.0, 0.0, 5.0, 1.0),
        // 使用可观察到相邻扩散的有限半径。
        2.0,
    )
    // 合法 blur 不应产生 typed failure。
    .expect("blur should succeed");
    // 保存实际 blur 输出作为后续目标相关参考源。
    let blurred = RenderBackend::copy_offscreen_pixels(&backend, &handle)
        // Picture 像素必须仍可读取。
        .expect("blurred pixels should remain available")
        // 测试只需要紧密像素数组。
        .0;
    // blur 必须把中心能量扩散到左右相邻像素。
    assert!(blurred[1] != 0 && blurred[3] != 0);

    // 使用非零透明背景证明结果依赖目标而非覆盖写入。
    let background = 0x6030_2010;
    // 把整个主表面初始化为同一预乘背景。
    backend
        // 取得主表面画布。
        .main
        // 借用状态完整的软件画布。
        .canvas_mut()
        // 取得内部像素表面。
        .surface_mut()
        // 借用目标像素。
        .pixels_mut()
        // 写入统一背景。
        .fill(background);
    // 取得主画布设置实际合成状态。
    let main = backend.main.canvas_mut();
    // 只允许中间三个像素接收 Picture 贡献。
    main.push_clip(Rect::new(1.0, 0.0, 3.0, 1.0));
    // 使用半透明度验证 opacity 仅应用一次。
    main.set_opacity(0.5);
    // 选择目标相关的饱和加法模式。
    main.set_blend_mode(BlendMode::Additive);
    // 设置非零 offset，证明 Picture bounds 已在 surface 空间且不会重复平移。
    main.set_offset(7.0, 0.0);
    // 把已模糊 Picture 合成到主表面。
    RenderBackend::try_blit_offscreen_src(
        // 修改主表面。
        &mut backend,
        // 使用已模糊的 Picture 资源。
        &handle,
        // 采样完整离屏范围。
        Rect::new(0.0, 0.0, 5.0, 1.0),
        // 目标同样覆盖完整 surface 空间。
        Rect::new(0.0, 0.0, 5.0, 1.0),
    )
    // checked blit 必须成功。
    .expect("blurred additive blit should succeed");

    // 读取最终主表面像素。
    let actual = backend.pixels();
    // clip 外的左端必须保持原背景。
    assert_eq!(actual[0], background);
    // clip 外的右端也必须保持原背景。
    assert_eq!(actual[4], background);
    // 中间每个像素都应等于半透明 blur 源与背景的饱和加法。
    for index in 1..4 {
        // 按生产量化顺序先应用 opacity，再执行 Additive。
        let expected = additive_reference(background, apply_opacity(blurred[index], 0.5));
        // 核验目标相关的逐像素结果。
        assert_eq!(actual[index], expected, "pixel {index}");
    }
    // canonical Picture 合成结束后必须恢复调用方 offset。
    assert_eq!(backend.main.canvas_mut().offset(), (7.0, 0.0));
    // canonical Picture 合成结束后也必须恢复调用方 opacity。
    assert_eq!(backend.main.canvas_mut().opacity(), 0.5);
}

// 验证有效 blur 对缺失 Picture 返回稳定 typed failure。
#[test]
fn blur_missing_picture_is_a_typed_error() {
    // 创建不含任何离屏资源的 CPU 后端。
    let mut backend = CpuBackend::new();
    // 对未知句柄执行有效半径 blur。
    let error = RenderBackend::try_blur_offscreen(
        // 修改被测后端。
        &mut backend,
        // 使用从未分配的句柄。
        &ImageHandle(77),
        // 提供非空有限区域。
        Rect::new(0.0, 0.0, 1.0, 1.0),
        // 提供会实际进入 blur 的半径。
        1.0,
    )
    // 缺失资源不得伪装成功。
    .expect_err("missing Picture blur should fail");
    // 错误类别必须保持资源状态错误。
    assert_eq!(error.code(), Errc::InvalidState);
}
