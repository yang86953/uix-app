// 引入被测共享值对象和字段偏移。
use super::*;

// 线性仿射 Gradient 必须冻结物理边和对应长度。
#[test]
fn linear_affine_params_own_edges_lengths_and_layout() {
    // 构造带旋转分量的非轴对齐仿射四角。
    let params = RhiGradientRasterParams::new(
        // 使用确定的物理视口。
        RhiViewport {
            // 保存物理宽度。
            width: 800.0,
            // 保存物理高度。
            height: 600.0,
        },
        // 使用 TL、TR、BR、BL 固定顺序。
        [[10.0, 20.0], [13.0, 24.0], [7.0, 27.0], [4.0, 23.0]],
        // 使用可区分的起始颜色。
        [0.1, 0.2, 0.3, 0.4],
        // 使用可区分的结束颜色。
        [0.5, 0.6, 0.7, 0.8],
        // 线性模式必须覆盖调用方传入的旧长度槽。
        [0.0, 2.0, 99.0, 88.0],
    );
    // 取得不可变共享 ABI 视图。
    let values = params.as_f32s();
    // viewport 必须位于固定起始槽。
    assert_eq!(
        // 读取共享 viewport float2。
        &values[GRADIENT_VIEWPORT_FLOAT_OFFSET..GRADIENT_VIEWPORT_FLOAT_OFFSET + 2],
        // 比较输入物理尺寸。
        &[800.0, 600.0]
    );
    // origin 与 X 边必须位于第二个 float4。
    assert_eq!(
        // 读取共享 origin/edge_x 字段。
        &values[GRADIENT_ORIGIN_EDGE_X_FLOAT_OFFSET..GRADIENT_ORIGIN_EDGE_X_FLOAT_OFFSET + 4],
        // TL 为原点，TR-TL 为三四五边。
        &[10.0, 20.0, 3.0, 4.0]
    );
    // Y 边必须来自 BL-TL 而不是平台 shader 自行推导。
    assert_eq!(
        // 读取共享 edge_y float2。
        &values[GRADIENT_EDGE_Y_FLOAT_OFFSET..GRADIENT_EDGE_Y_FLOAT_OFFSET + 2],
        // BL-TL 得到负六和三。
        &[-6.0, 3.0]
    );
    // 线性长度必须由共享层唯一计算。
    assert_eq!(
        // 读取共享 params float4。
        &values[GRADIENT_PARAMS_FLOAT_OFFSET..GRADIENT_PARAMS_FLOAT_OFFSET + 4],
        // X 边长度为五，Y 边长度为 sqrt(45)。
        &[0.0, 2.0, 5.0, 45.0f32.sqrt()]
    );
    // 编码后的字节数必须与 PipelineContract 完全一致。
    assert_eq!(params.encode_ne_bytes().len(), GRADIENT_UNIFORM_BYTES);
}

// 径向 Gradient 必须保留调用方已经归一化的半径参数。
#[test]
fn radial_params_preserve_normalized_radii() {
    // 构造轴对齐径向 Gradient。
    let params = RhiGradientRasterParams::new(
        // 使用方形物理视口。
        RhiViewport {
            // 保存物理宽度。
            width: 100.0,
            // 保存物理高度。
            height: 100.0,
        },
        // 使用完整 20x20 仿射区域。
        [[0.0, 0.0], [20.0, 0.0], [20.0, 20.0], [0.0, 20.0]],
        // 颜色值不参与本断言。
        [0.0; 4],
        // 颜色值不参与本断言。
        [1.0; 4],
        // 径向模式携带内外归一化半径。
        [1.0, 0.1, 0.5, 0.0],
    );
    // 读取共享 params 槽。
    let values = params.as_f32s();
    // 径向参数不得被线性边长派生覆盖。
    assert_eq!(
        // 截取完整 params float4。
        &values[GRADIENT_PARAMS_FLOAT_OFFSET..GRADIENT_PARAMS_FLOAT_OFFSET + 4],
        // 保留调用方输入的内外半径。
        &[1.0, 0.1, 0.5, 0.0]
    );
}

// Gradient 值域必须只接受合法线性模式和正径向外半径。
#[test]
fn gradient_value_domain_accepts_linear_and_positive_radial() {
    // 使用统一的最小构造器保持每个值域断言只改变参数槽。
    let make = |params| {
        // 构造固定物理四角和有限颜色。
        RhiGradientRasterParams::new(
            // 使用有效方形 viewport。
            RhiViewport {
                // 保存 viewport 宽度。
                width: 100.0,
                // 保存 viewport 高度。
                height: 100.0,
            },
            // 使用单位轴对齐四角。
            [[0.0, 0.0], [20.0, 0.0], [20.0, 20.0], [0.0, 20.0]],
            // 使用有限起始颜色。
            [0.0; 4],
            // 使用有限结束颜色。
            [1.0; 4],
            // 注入当前待验证的模式与半径参数。
            params,
        )
    };
    // 线性模式保持既有有效语义。
    assert!(make([0.0, 2.0, -1.0, -1.0]).is_valid());
    // 正径向外半径必须通过共享门禁。
    assert!(make([1.0, 0.1, 0.5, 0.0]).is_valid());
    // 未知模式不得交给任一 Adapter 猜测。
    assert!(!make([0.5, 0.1, 0.5, 0.0]).is_valid());
    // 零径向外半径必须被拒绝。
    assert!(!make([1.0, 0.1, 0.0, 0.0]).is_valid());
    // 负径向外半径必须被拒绝。
    assert!(!make([1.0, 0.1, -0.5, 0.0]).is_valid());
    // 非有限径向外半径必须被通用有限值门禁拒绝。
    assert!(!make([1.0, 0.1, f32::NAN, 0.0]).is_valid());
}
