// 引入被测共享 Shape 类型和字段索引。
use super::{
    // 引入共享 Shape 参数值对象。
    RhiShapeRasterParams,
    // 引入 draw rect 字段索引。
    SHAPE_DRAW_RECT_FLOAT_OFFSET,
    // 引入描边字段索引。
    SHAPE_STROKE_FLOAT_OFFSET,
    // 引入固定 ABI 字节数。
    SHAPE_UNIFORM_BYTES,
};
// 引入共享视口值。
use super::super::RhiViewport;

// 构造一个可精确表示的标准一像素描边。
fn stroke_fixture() -> RhiShapeRasterParams {
    // 返回带四像素圆角的一像素描边契约。
    RhiShapeRasterParams::new(
        // 使用固定物理视口。
        RhiViewport {
            // 视口宽度。
            width: 100.0,
            // 视口高度。
            height: 80.0,
        },
        // 使用固定原始矩形。
        [10.0, 20.0, 30.0, 40.0],
        // 使用不影响 coverage 的白色。
        [1.0; 4],
        // 使用统一四像素圆角。
        [4.0; 4],
        // 半像素半宽对应一像素描边。
        0.5,
    )
}

// 验证描边外扩、同心 SDF 原点和 ABI 编码都由共享值对象拥有。
#[test]
fn shared_shape_contract_owns_geometry_and_abi() {
    // 构造标准描边参数。
    let stroke = stroke_fixture();
    // 描边 quad 必须向四周扩张一点五像素。
    assert_eq!(stroke.draw_rect(), [8.5, 18.5, 33.0, 43.0]);
    // 读取固定 float ABI。
    let values = stroke.as_f32s();
    // 描边字段必须携带半宽、fringe、outer 偏移和 inner 偏移。
    assert_eq!(
        &values[SHAPE_STROKE_FLOAT_OFFSET..SHAPE_DRAW_RECT_FLOAT_OFFSET],
        &[0.5, 1.0, 1.0, 2.0]
    );
    // 最后一个 float4 必须携带共享 draw rect。
    assert_eq!(
        &values[SHAPE_DRAW_RECT_FLOAT_OFFSET..],
        &[8.5, 18.5, 33.0, 43.0]
    );
    // 字节编码必须严格匹配固定 ABI 大小。
    assert_eq!(stroke.encode_ne_bytes().len(), SHAPE_UNIFORM_BYTES);
}

// 验证填充不扩张且没有伪造描边局部偏移。
#[test]
fn shared_shape_contract_keeps_fill_bounds() {
    // 构造没有描边的填充参数。
    let fill = RhiShapeRasterParams::new(
        // 使用固定物理视口。
        RhiViewport {
            // 视口宽度。
            width: 100.0,
            // 视口高度。
            height: 80.0,
        },
        // 使用固定原始矩形。
        [10.0, 20.0, 30.0, 40.0],
        // 使用不影响几何的白色。
        [1.0; 4],
        // 使用统一四像素圆角。
        [4.0; 4],
        // 零半宽表示填充。
        0.0,
    );
    // 填充 draw rect 必须等于原始矩形。
    assert_eq!(fill.draw_rect(), [10.0, 20.0, 30.0, 40.0]);
    // 填充描边字段除固定 fringe 声明外都必须为零。
    assert_eq!(
        &fill.as_f32s()[SHAPE_STROKE_FLOAT_OFFSET..SHAPE_DRAW_RECT_FLOAT_OFFSET],
        &[0.0, 1.0, 0.0, 0.0]
    );
}

// 验证 inner SDF 使用独立原点后与 outer SDF 严格同心。
#[test]
fn shared_shape_pixel_semantics_are_concentric() {
    // 构造标准描边参数。
    let stroke = stroke_fixture();
    // 选择垂直中心，排除圆角对水平边界 coverage 的影响。
    let center_y = 21.5;
    // outer 几何边界应得到半覆盖率。
    assert_eq!(stroke.coverage_at([1.0, center_y]), 0.5);
    // 原始矩形外边应得到完整描边覆盖率。
    assert_eq!(stroke.coverage_at([1.5, center_y]), 1.0);
    // inner 几何边界应得到半覆盖率。
    assert_eq!(stroke.coverage_at([2.0, center_y]), 0.5);
    // 矩形中心必须被 inner SDF 完整挖空。
    assert_eq!(stroke.coverage_at([16.5, center_y]), 0.0);
}

// 从 Rust 源文件中提取两个固定标记之间的 shader 常量区域。
fn source_section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    // 查找区域起始标记。
    let start_index = source.find(start).expect("shader section start must exist");
    // 从起始位置之后查找区域结束标记。
    let relative_end = source[start_index..]
        .find(end)
        .expect("shader section end must exist");
    // 返回不包含下一个常量的当前 shader 区域。
    &source[start_index..start_index + relative_end]
}

// 验证 ABI 字段或语义标记在原生 shader 中按共享顺序出现。
fn assert_source_order(source: &str, markers: &[&str]) {
    // 从当前源片段起点开始查找。
    let mut cursor = 0;
    // 逐个消费共享契约声明的有序标记。
    for marker in markers {
        // 在尚未消费的源片段内查找当前标记。
        let relative = source[cursor..]
            // 查找精确标记。
            .find(marker)
            // 缺失字段必须让契约测试失败。
            .unwrap_or_else(|| panic!("shader contract marker is missing: {marker}"));
        // 把游标推进到当前标记之后，保证后续字段不能倒序。
        cursor += relative + marker.len();
    }
}

// 验证 OpenGL 与 D3D11 shader 都只是同一共享 Shape 契约的原生编码。
#[test]
fn native_shape_shaders_encode_the_shared_contract() {
    // 在非 Windows 测试目标也读取 D3D11 shader 源以形成跨平台门禁。
    let d3d_source = include_str!(
        "../../../../../src/native/presentation/graphics/d3d11/adapter/pipeline/mod.rs"
    );
    // 提取 D3D11 Shape shader 常量。
    let d3d_shape = source_section(d3d_source, "const RECT_HLSL", "const GLYPH_HLSL");
    // D3D11 cbuffer 必须严格保持共享六个 float4 的 ABI 顺序。
    assert_source_order(
        // 检查当前 D3D11 Shape shader。
        d3d_shape,
        // 列出共享 ABI 的有序字段。
        &[
            // viewport.x/y。
            "float2 u_viewport;",
            // viewport padding。
            "float2 _pad0;",
            // 原始矩形。
            "float4 u_rect;",
            // 直通颜色。
            "float4 u_color;",
            // 四角半径。
            "float4 u_radius;",
            // 描边语义。
            "float4 u_stroke;",
            // 实际绘制边界。
            "float4 u_draw_rect;",
        ],
    );
    // D3D11 顶点阶段必须直接消费共享 draw rect。
    assert!(d3d_shape.contains("u_draw_rect.xy + input.pos * u_draw_rect.zw"));
    // D3D11 outer SDF 必须消费共享 outer 原点偏移。
    assert!(d3d_shape.contains("input.local - float2(u_stroke.z, u_stroke.z)"));
    // D3D11 inner SDF 必须消费共享 inner 原点偏移。
    assert!(d3d_shape.contains("input.local - float2(u_stroke.w, u_stroke.w)"));
    // D3D11 Adapter 不得重新推导 draw quad 外扩。
    assert!(!d3d_shape.contains("float expand ="));
    // 在共享测试目标读取 OpenGL 固定 shader 源。
    let gl_source = include_str!(
        "../../../../../src/native/presentation/graphics/opengl/raster/rhi_shaders.rs"
    );
    // 提取 OpenGL Shape 顶点阶段。
    let gl_shape_vertex = source_section(
        gl_source,
        "pub(super) const SHAPE_VERTEX",
        "pub(super) const SECTOR_VERTEX",
    );
    // 提取 OpenGL Sector 顶点阶段。
    let gl_sector_vertex = source_section(
        gl_source,
        "pub(super) const SECTOR_VERTEX",
        "pub(super) const GRADIENT_VERTEX",
    );
    // 提取 OpenGL Shape 片元阶段。
    let gl_shape_fragment = source_section(
        gl_source,
        "pub(super) const SHAPE_FRAGMENT",
        "pub(super) const SECTOR_FRAGMENT",
    );
    // OpenGL Shape 顶点阶段必须直接消费共享 draw rect。
    assert!(gl_shape_vertex.contains("u_draw_rect.xy + a_pos * u_draw_rect.zw"));
    // OpenGL outer SDF 必须消费共享 outer 原点偏移。
    assert!(gl_shape_fragment.contains("v_local - vec2(u_stroke.z)"));
    // OpenGL inner SDF 必须消费共享 inner 原点偏移。
    assert!(gl_shape_fragment.contains("v_local - vec2(u_stroke.w)"));
    // Sector 不得取得 Shape 的 draw rect 契约。
    assert!(!gl_sector_vertex.contains("u_draw_rect"));
    // Sector 不得取得 Shape 的描边契约。
    assert!(!gl_sector_vertex.contains("u_stroke"));
    // 读取 OpenGL pipeline 语义到固定 shader 的原生编码表。
    let gl_pipeline_source = include_str!(
        "../../../../../src/native/presentation/graphics/opengl/raster/rhi_device_pipeline.rs"
    );
    // Shape 语义必须选择 Shape 专属顶点阶段。
    assert!(
        gl_pipeline_source.contains("(rhi_shaders::SHAPE_VERTEX, rhi_shaders::SHAPE_FRAGMENT)")
    );
    // Sector 语义必须选择 Sector 专属顶点阶段。
    assert!(gl_pipeline_source.contains(
        "PipelineKind::Sector => (rhi_shaders::SECTOR_VERTEX, rhi_shaders::SECTOR_FRAGMENT)"
    ));
}
