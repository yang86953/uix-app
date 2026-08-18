// 引入被测范围、结果和尺寸值。
use super::{RhiExtent, RhiScissor, RhiSurfaceReadback};

// 验证合法区域保留原点、尺寸和规范像素。
#[test]
fn accepts_exact_top_left_argb_payload() {
    // 构造完整落在四乘三 surface 内的二乘二区域。
    let region = RhiScissor {
        // 从第二列开始。
        x: 1,
        // 从第二行开始。
        y: 1,
        // 读取两列。
        width: 2,
        // 读取两行。
        height: 2,
    };
    // 使用可辨识的 0xAARRGGBB 像素构造结果。
    let readback = RhiSurfaceReadback::try_new(
        // 保留请求区域。
        region,
        // 声明当前 surface extent。
        RhiExtent::new(4, 3),
        // 按顶部到底部的紧密顺序提供四个像素。
        vec![0xFF11_2233, 0xFF44_5566, 0xFF77_8899, 0xFFAA_BBCC],
    )
    // 合法结果必须构造成功。
    .expect("valid surface readback must be accepted");
    // 回读区域不得被 Adapter 改写。
    assert_eq!(readback.region, region);
    // 规范像素必须保持原值和行序。
    assert_eq!(
        readback.into_pixels(),
        vec![0xFF11_2233, 0xFF44_5566, 0xFF77_8899, 0xFFAA_BBCC]
    );
}

// 验证越界和载荷长度不匹配不能被 Adapter 静默接受。
#[test]
fn rejects_clipped_or_partial_payloads() {
    // 构造越过右边界的区域。
    let outside = RhiScissor {
        // 从最后一列开始。
        x: 3,
        // 从首行开始。
        y: 0,
        // 请求两列会越过四列 surface。
        width: 2,
        // 请求一行。
        height: 1,
    };
    // 越界请求必须在原生 API 前失败。
    assert!(RhiSurfaceReadback::validate_region(outside, RhiExtent::new(4, 3)).is_err());
    // 构造合法二乘二区域。
    let region = RhiScissor {
        // 从原点开始。
        x: 0,
        // 从顶部开始。
        y: 0,
        // 请求两列。
        width: 2,
        // 请求两行。
        height: 2,
    };
    // 少一个像素的载荷必须被视为 Adapter 契约破坏。
    assert!(RhiSurfaceReadback::try_new(region, RhiExtent::new(4, 3), vec![0; 3]).is_err());
}
