    // 引入被测共享值对象和字段偏移。
    use super::*;

    // 仿射 Shadow 必须由共享层唯一派生原点和两条物理边。
    #[test]
    fn affine_params_own_edges_and_layout() {
        // 构造带旋转分量的非轴对齐阴影四角。
        let params = RhiShadowRasterParams::new(
            // 使用确定的物理视口。
            RhiViewport {
                // 保存物理宽度。
                width: 800.0,
                // 保存物理高度。
                height: 600.0,
            },
            // 使用 TL、TR、BR、BL 固定顺序。
            [[10.0, 20.0], [14.0, 23.0], [8.0, 31.0], [4.0, 28.0]],
            // 使用可区分的直通颜色。
            [0.1, 0.2, 0.3, 0.4],
            // 使用可区分的四角半径。
            [1.0, 2.0, 3.0, 4.0],
            // 使用非对称物理模糊量。
            [5.0, 6.0],
            // 使用非正方形本体尺寸。
            [70.0, 80.0],
            // 使用环境阴影曲线。
            true,
        );
        // 取得不可变共享 ABI 视图。
        let values = params.as_f32s();
        // viewport 必须位于固定起始槽。
        assert_eq!(
            // 读取共享 viewport float2。
            &values[SHADOW_VIEWPORT_FLOAT_OFFSET..SHADOW_VIEWPORT_FLOAT_OFFSET + 2],
            // 比较输入物理尺寸。
            &[800.0, 600.0]
        );
        // origin 与 X 边必须位于第二个 float4。
        assert_eq!(
            // 读取共享 origin/edge_x 字段。
            &values[SHADOW_ORIGIN_EDGE_X_FLOAT_OFFSET..SHADOW_ORIGIN_EDGE_X_FLOAT_OFFSET + 4],
            // TL 为原点，TR-TL 为四和三。
            &[10.0, 20.0, 4.0, 3.0]
        );
        // Y 边与两轴模糊量必须共用固定 float4。
        assert_eq!(
            // 读取共享 edge_y/blur 字段。
            &values[SHADOW_EDGE_Y_BLUR_FLOAT_OFFSET..SHADOW_EDGE_Y_BLUR_FLOAT_OFFSET + 4],
            // BL-TL 得到负六和八，并保留两个 blur。
            &[-6.0, 8.0, 5.0, 6.0]
        );
        // 本体尺寸与环境标记必须位于最后一个 float4。
        assert_eq!(
            // 读取共享 body_size/ambient 字段。
            &values
                [SHADOW_BODY_SIZE_AMBIENT_FLOAT_OFFSET..SHADOW_BODY_SIZE_AMBIENT_FLOAT_OFFSET + 4],
            // 环境标记固定编码为一，padding 固定为零。
            &[70.0, 80.0, 1.0, 0.0]
        );
        // 编码后的字节数必须与 PipelineContract 完全一致。
        assert_eq!(params.encode_ne_bytes().len(), SHADOW_UNIFORM_BYTES);
    }

    // 非环境阴影必须把共享标记精确编码为零。
    #[test]
    fn non_ambient_flag_is_zero() {
        // 构造最小轴对齐阴影参数。
        let params = RhiShadowRasterParams::new(
            // 使用有效物理视口。
            RhiViewport {
                // 保存物理宽度。
                width: 100.0,
                // 保存物理高度。
                height: 100.0,
            },
            // 使用十像素方形扩展四角。
            [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]],
            // 颜色不参与当前断言。
            [0.0; 4],
            // 圆角不参与当前断言。
            [0.0; 4],
            // blur 不参与当前断言。
            [0.0; 2],
            // 使用十像素本体。
            [10.0; 2],
            // 关闭环境覆盖曲线。
            false,
        );
        // 读取最后一个 float4 的 ambient 槽。
        let values = params.as_f32s();
        // 普通阴影必须以精确零进入两个 Adapter。
        assert_eq!(values[SHADOW_BODY_SIZE_AMBIENT_FLOAT_OFFSET + 2], 0.0);
    }
