// 引入被测共享值对象和字段偏移。
use super::*;

// 构造可区分的测试 viewport。
fn viewport() -> RhiViewport {
    // 返回非方形 viewport，避免宽高交换无法被发现。
    RhiViewport {
        // 保存测试宽度。
        width: 800.0,
        // 保存测试高度。
        height: 600.0,
    }
}

// Mesh 必须冻结 viewport、padding 与完整颜色。
#[test]
fn mesh_params_own_viewport_padding_and_color() {
    // 构造完整共享 Mesh 参数。
    let params = RhiMeshRasterParams::new(viewport(), [0.1, 0.2, 0.3, 0.4]);
    // 取得不可变 ABI 视图。
    let values = params.as_f32s();
    // viewport 与 padding 必须占用第一个 float4。
    assert_eq!(
        &values[MESH_VIEWPORT_FLOAT_OFFSET..4],
        &[800.0, 600.0, 0.0, 0.0]
    );
    // straight-alpha 颜色必须占用第二个 float4。
    assert_eq!(&values[MESH_COLOR_FLOAT_OFFSET..], &[0.1, 0.2, 0.3, 0.4]);
    // 编码字节数必须与 pipeline 契约一致。
    assert_eq!(params.encode_ne_bytes().len(), MESH_UNIFORM_BYTES);
}

// Sampled 与 coverage 必须共享同一个 viewport/padding ABI。
#[test]
fn sampled_params_own_shared_viewport_layout() {
    // 构造 sampled/coverage 共用参数。
    let params = RhiSampledRasterParams::new(viewport());
    // 两个 float4 必须包含 viewport、默认零圆角与确定性 padding。
    assert_eq!(
        params.as_f32s(),
        &[800.0, 600.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
    );
    // 编码字节数必须与 pipeline 契约一致。
    assert_eq!(params.encode_ne_bytes().len(), SAMPLED_UNIFORM_BYTES);
}

// 最终 surface 合成必须只在共享 ABI 的固定槽位写入平台圆角事实。
#[test]
fn sampled_surface_radius_uses_the_shared_slot() {
    // 构造带物理圆角半径的最终合成参数。
    let params = RhiSampledRasterParams::with_surface_corner_radius(viewport(), 12.0);
    // viewport 保持不变，圆角半径只占用第二个 float4 的首槽。
    assert_eq!(
        params.as_f32s(),
        &[800.0, 600.0, 0.0, 0.0, 12.0, 0.0, 0.0, 0.0]
    );
}

// Sector 必须冻结 viewport、矩形、颜色、角度和两处 padding。
#[test]
fn sector_params_own_complete_layout() {
    // 构造每个语义段都可区分的 Sector 参数。
    let params = RhiSectorRasterParams::new(
        // 使用共享测试 viewport。
        viewport(),
        // 使用可区分的物理外接矩形。
        [10.0, 20.0, 30.0, 40.0],
        // 使用可区分的 straight-alpha 颜色。
        [0.1, 0.2, 0.3, 0.4],
        // 使用可区分的起始与扫过角。
        [0.5, 1.5],
    );
    // 取得不可变 ABI 视图。
    let values = params.as_f32s();
    // viewport 与 padding 必须占用第一个 float4。
    assert_eq!(
        &values[SECTOR_VIEWPORT_FLOAT_OFFSET..4],
        &[800.0, 600.0, 0.0, 0.0]
    );
    // 矩形必须占用第二个 float4。
    assert_eq!(
        &values[SECTOR_RECT_FLOAT_OFFSET..8],
        &[10.0, 20.0, 30.0, 40.0]
    );
    // 颜色必须占用第三个 float4。
    assert_eq!(
        &values[SECTOR_COLOR_FLOAT_OFFSET..12],
        &[0.1, 0.2, 0.3, 0.4]
    );
    // 角度和 padding 必须占用最后一个 float4。
    assert_eq!(&values[SECTOR_ANGLES_FLOAT_OFFSET..], &[0.5, 1.5, 0.0, 0.0]);
    // 编码字节数必须与 pipeline 契约一致。
    assert_eq!(params.encode_ne_bytes().len(), SECTOR_UNIFORM_BYTES);
}
