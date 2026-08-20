    // 引入被测共享值对象和字段偏移。
    use super::*;

    // Blur 必须冻结不同目标与源尺寸、区域、方向及全部六十四个权重。
    #[test]
    fn params_own_complete_layout_and_weight_range() {
        // 构造每个槽都可区分的完整权重数组。
        let mut weights = [0.0f32; BLUR_WEIGHT_COUNT];
        // 按索引写入稳定递增值。
        for (index, weight) in weights.iter_mut().enumerate() {
            // 加一避免第一个权重与未使用零槽混淆。
            *weight = (index + 1) as f32 / 100.0;
        }
        // 构造目标与源尺寸不同的共享 Blur 参数。
        let params = RhiBlurRasterParams::new(
            // 使用独立目标尺寸。
            RhiExtent::new(800, 600),
            // 使用独立 source texture 尺寸。
            RhiExtent::new(400, 300),
            // 使用可区分的正值物理区域。
            RhiScissor {
                // 保存区域左边界。
                x: 10,
                // 保存区域上边界。
                y: 20,
                // 保存区域宽度。
                width: 30,
                // 保存区域高度。
                height: 40,
            },
            // 使用垂直方向验证顺序。
            [0.0, 1.0],
            // 使用可区分的 tap 半径。
            7,
            // 传入完整权重数组。
            &weights,
        );
        // 取得不可变共享 ABI 视图。
        let values = params.as_f32s();
        // 目标与 source extent 必须位于第一个 float4。
        assert_eq!(
            // 读取共享 sizes 字段。
            &values[BLUR_SIZES_FLOAT_OFFSET..BLUR_SIZES_FLOAT_OFFSET + 4],
            // 比较目标宽高和 source 宽高。
            &[800.0, 600.0, 400.0, 300.0]
        );
        // 区域必须位于第二个 float4。
        assert_eq!(
            // 读取共享 region 字段。
            &values[BLUR_REGION_FLOAT_OFFSET..BLUR_REGION_FLOAT_OFFSET + 4],
            // 比较左、上、宽、高。
            &[10.0, 20.0, 30.0, 40.0]
        );
        // 方向、半径和 padding 必须位于第三个 float4。
        assert_eq!(
            // 读取共享 direction/taps 字段。
            &values[BLUR_DIRECTION_TAPS_FLOAT_OFFSET..BLUR_DIRECTION_TAPS_FLOAT_OFFSET + 4],
            // 比较垂直方向、半径与固定零 padding。
            &[0.0, 1.0, 7.0, 0.0]
        );
        // 权重区间必须完整保留首尾槽，避免 OpenGL 与 D3D11 少读一个 float4。
        assert_eq!(
            // 读取完整共享权重区间。
            &values[BLUR_WEIGHTS_FLOAT_OFFSET..BLUR_WEIGHTS_FLOAT_OFFSET + BLUR_WEIGHT_COUNT],
            // 比较调用方的完整六十四项输入。
            &weights
        );
        // 编码后的字节数必须与 PipelineContract 完全一致。
        assert_eq!(params.encode_ne_bytes().len(), BLUR_UNIFORM_BYTES);
    }
