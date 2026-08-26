// 引入被测共享值对象和字段偏移。
use super::*;

// MSDF 必须冻结 viewport、atlas 尺寸、距离范围和三个零 padding。
#[test]
fn params_own_complete_layout_and_texture_extent() {
    // 构造每个有效字段都可区分的共享 MSDF 参数。
    let params = RhiMsdfRasterParams::new(
        // 使用非方形 viewport 验证字段顺序。
        RhiViewport {
            // 保存可区分的 viewport 宽度。
            width: 800.0,
            // 保存可区分的 viewport 高度。
            height: 600.0,
        },
        // 使用非方形 atlas extent 验证纹理尺寸语义。
        RhiExtent::new(1024, 512),
        // 使用可区分的距离范围。
        7.5,
    );
    // 取得不可变共享 ABI 视图。
    let values = params.as_f32s();
    // viewport 必须占用首两个 float。
    assert_eq!(
        // 读取共享 viewport 字段。
        &values[MSDF_VIEWPORT_FLOAT_OFFSET..MSDF_VIEWPORT_FLOAT_OFFSET + 2],
        // 比较目标宽高。
        &[800.0, 600.0]
    );
    // sampled texture extent 必须紧随 viewport。
    assert_eq!(
        // 读取共享 texture size 字段。
        &values[MSDF_TEXTURE_SIZE_FLOAT_OFFSET..MSDF_TEXTURE_SIZE_FLOAT_OFFSET + 2],
        // 比较 atlas 宽高。
        &[1024.0, 512.0]
    );
    // 距离范围必须位于第二个 float4 的首槽。
    assert_eq!(values[MSDF_RANGE_FLOAT_OFFSET], 7.5);
    // 第二个 float4 的其余槽必须保持确定性零值。
    assert_eq!(&values[MSDF_RANGE_FLOAT_OFFSET + 1..], &[0.0, 0.0, 0.0]);
    // 编码后的字节数必须与 PipelineContract 完全一致。
    assert_eq!(params.encode_ne_bytes().len(), MSDF_UNIFORM_BYTES);
}
