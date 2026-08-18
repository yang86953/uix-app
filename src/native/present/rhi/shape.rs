//! Drawing System 在共享 GPU Raster/RHI 边界拥有的 Shape 像素契约。

// 引入共享视口值，避免 Shape 契约依赖任何平台 API 类型。
use super::RhiViewport;

// 固定 Shape uniform 的六个 float4 ABI 总字节数。
pub(crate) const SHAPE_UNIFORM_BYTES: usize = 96;

// 固定 Shape uniform 的 float 数量。
const SHAPE_UNIFORM_FLOATS: usize = SHAPE_UNIFORM_BYTES / std::mem::size_of::<f32>();

// Shape 的分析抗锯齿在几何边界外保留一个物理像素。
const SHAPE_AA_FRINGE: f32 = 1.0;

// viewport float2 在 Shape ABI 中的起始 float 索引。
pub(crate) const SHAPE_VIEWPORT_FLOAT_OFFSET: usize = 0;

// 原始矩形 float4 在 Shape ABI 中的起始 float 索引。
pub(crate) const SHAPE_RECT_FLOAT_OFFSET: usize = 4;

// 直通颜色 float4 在 Shape ABI 中的起始 float 索引。
pub(crate) const SHAPE_COLOR_FLOAT_OFFSET: usize = 8;

// 四角半径 float4 在 Shape ABI 中的起始 float 索引。
pub(crate) const SHAPE_RADIUS_FLOAT_OFFSET: usize = 12;

// 描边语义 float4 在 Shape ABI 中的起始 float 索引。
pub(crate) const SHAPE_STROKE_FLOAT_OFFSET: usize = 16;

// 实际绘制边界 float4 在 Shape ABI 中的起始 float 索引。
pub(crate) const SHAPE_DRAW_RECT_FLOAT_OFFSET: usize = 20;

// 保存已经冻结的平台无关 Shape 像素参数与 ABI 布局。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RhiShapeRasterParams {
    // 六个 float4 依次保存 viewport、rect、颜色、圆角、描边和 draw rect。
    values: [f32; SHAPE_UNIFORM_FLOATS],
}

// 为共享 Shape 像素契约提供唯一构造与编码入口。
impl RhiShapeRasterParams {
    // 从已经完成物理坐标 lowering 的 Shape 值构造共享参数。
    pub(crate) fn new(
        // 接收物理绘制目标尺寸。
        viewport: RhiViewport,
        // 接收原始 Shape 左上角和尺寸。
        rect: [f32; 4],
        // 接收八位颜色语义规整前的直通浮点颜色。
        color: [f32; 4],
        // 接收左上、右上、右下、左下圆角半径。
        radius: [f32; 4],
        // 接收描边半宽，零表示填充。
        half_stroke: f32,
    ) -> Self {
        // 填充不扩张，描边同时容纳半线宽与分析抗锯齿边界。
        let expansion = if half_stroke > 0.0 {
            // 描边需要覆盖外半宽和一像素 coverage 过渡。
            half_stroke + SHAPE_AA_FRINGE
        } else {
            // 填充的 quad 与原始矩形完全一致。
            0.0
        };
        // 共享层冻结 adapter 必须直接消费的实际绘制边界。
        let draw_rect = [
            // 向左容纳完整描边与 coverage。
            rect[0] - expansion,
            // 向上容纳完整描边与 coverage。
            rect[1] - expansion,
            // 同时扩张左右两侧。
            rect[2] + expansion * 2.0,
            // 同时扩张上下两侧。
            rect[3] + expansion * 2.0,
        ];
        // outer SDF 原点相对 draw rect 固定偏移一个 coverage fringe。
        let outer_origin_offset = if half_stroke > 0.0 {
            // 描边 outer rect 从 draw rect 内缩一个 fringe。
            SHAPE_AA_FRINGE
        } else {
            // 填充直接使用 draw rect 局部坐标。
            0.0
        };
        // inner SDF 必须再内缩一个完整线宽，保持两个 SDF 严格同心。
        let inner_origin_offset = if half_stroke > 0.0 {
            // 两倍半描边宽度就是 outer 与 inner 左上角之间的距离。
            SHAPE_AA_FRINGE + half_stroke * 2.0
        } else {
            // 填充没有 inner SDF。
            0.0
        };
        // 按固定六个 float4 ABI 构造不可分叉的 Shape 参数。
        let values = [
            // viewport.x。
            viewport.width,
            // viewport.y。
            viewport.height,
            // viewport padding.x。
            0.0,
            // viewport padding.y。
            0.0,
            // rect.x。
            rect[0],
            // rect.y。
            rect[1],
            // rect.width。
            rect[2],
            // rect.height。
            rect[3],
            // color.r。
            color[0],
            // color.g。
            color[1],
            // color.b。
            color[2],
            // color.a。
            color[3],
            // radius.top_left。
            radius[0],
            // radius.top_right。
            radius[1],
            // radius.bottom_right。
            radius[2],
            // radius.bottom_left。
            radius[3],
            // stroke.x 保存半描边宽度。
            half_stroke,
            // stroke.y 保存分析抗锯齿 fringe。
            SHAPE_AA_FRINGE,
            // stroke.z 保存 outer SDF 局部原点偏移。
            outer_origin_offset,
            // stroke.w 保存 inner SDF 局部原点偏移。
            inner_origin_offset,
            // draw_rect.x。
            draw_rect[0],
            // draw_rect.y。
            draw_rect[1],
            // draw_rect.width。
            draw_rect[2],
            // draw_rect.height。
            draw_rect[3],
        ];
        // 返回已经冻结的共享 Shape 值对象。
        Self { values }
    }

    // 返回 adapter 可按固定字段索引读取的 float ABI。
    pub(crate) const fn as_f32s(&self) -> &[f32; SHAPE_UNIFORM_FLOATS] {
        // 借出不可变数组，禁止下层重新组织字段。
        &self.values
    }

    // 把共享 Shape ABI 编码为当前 host 的紧密字节载荷。
    pub(crate) fn encode_ne_bytes(&self) -> Vec<u8> {
        // 为完整 ABI 预留精确容量。
        let mut bytes = Vec::with_capacity(SHAPE_UNIFORM_BYTES);
        // 逐个保持 IEEE float 的原始 native-endian 表示。
        for value in self.values {
            // adapter 与共享层运行在同一 host，因此直接追加 native-endian 字节。
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        // 返回可直接上传到 uniform buffer 的紧密载荷。
        bytes
    }

    // 返回共享层冻结的实际绘制边界。
    pub(crate) const fn draw_rect(&self) -> [f32; 4] {
        // 从固定最后一个 float4 读取边界。
        [
            // draw_rect.x。
            self.values[SHAPE_DRAW_RECT_FLOAT_OFFSET],
            // draw_rect.y。
            self.values[SHAPE_DRAW_RECT_FLOAT_OFFSET + 1],
            // draw_rect.width。
            self.values[SHAPE_DRAW_RECT_FLOAT_OFFSET + 2],
            // draw_rect.height。
            self.values[SHAPE_DRAW_RECT_FLOAT_OFFSET + 3],
        ]
    }

    // 以共享公式计算 draw rect 局部采样点的 Shape coverage。
    pub(crate) fn coverage_at(&self, draw_local: [f32; 2]) -> f32 {
        // 读取原始矩形尺寸。
        let rect_size = [
            // rect.width。
            self.values[SHAPE_RECT_FLOAT_OFFSET + 2],
            // rect.height。
            self.values[SHAPE_RECT_FLOAT_OFFSET + 3],
        ];
        // 读取四角半径。
        let radius = [
            // radius.top_left。
            self.values[SHAPE_RADIUS_FLOAT_OFFSET],
            // radius.top_right。
            self.values[SHAPE_RADIUS_FLOAT_OFFSET + 1],
            // radius.bottom_right。
            self.values[SHAPE_RADIUS_FLOAT_OFFSET + 2],
            // radius.bottom_left。
            self.values[SHAPE_RADIUS_FLOAT_OFFSET + 3],
        ];
        // 读取描边半宽。
        let half_stroke = self.values[SHAPE_STROKE_FLOAT_OFFSET];
        // 描边使用同心 outer 与 inner SDF 的 coverage 乘积。
        if half_stroke > 0.0 {
            // 读取共享 outer 原点偏移。
            let outer_offset = self.values[SHAPE_STROKE_FLOAT_OFFSET + 2];
            // 读取共享 inner 原点偏移。
            let inner_offset = self.values[SHAPE_STROKE_FLOAT_OFFSET + 3];
            // 把 draw rect 局部坐标转换到 outer SDF 局部坐标。
            let outer_local = [draw_local[0] - outer_offset, draw_local[1] - outer_offset];
            // 把 draw rect 局部坐标转换到 inner SDF 局部坐标。
            let inner_local = [draw_local[0] - inner_offset, draw_local[1] - inner_offset];
            // outer 尺寸向四边各扩张半描边宽度。
            let outer_size = [
                // outer.width。
                rect_size[0] + half_stroke * 2.0,
                // outer.height。
                rect_size[1] + half_stroke * 2.0,
            ];
            // inner 尺寸向四边各内缩半描边宽度。
            let inner_size = [
                // inner.width。
                (rect_size[0] - half_stroke * 2.0).max(0.0),
                // inner.height。
                (rect_size[1] - half_stroke * 2.0).max(0.0),
            ];
            // outer 圆角随外边界扩张半描边宽度。
            let outer_radius = radius.map(|value| value + half_stroke);
            // inner 圆角随内边界收缩半描边宽度且不低于零。
            let inner_radius = radius.map(|value| (value - half_stroke).max(0.0));
            // outer coverage 使用固定一像素线性分析抗锯齿。
            let outer_coverage = sdf_to_coverage(rounded_rect_sdf(
                // 传入 outer 局部采样点。
                outer_local,
                // 传入 outer 尺寸。
                outer_size,
                // 传入 outer 四角半径。
                outer_radius,
            ));
            // 宽描边盖满矩形时不再构造退化 inner SDF。
            if inner_size[0] <= 0.0 || inner_size[1] <= 0.0 {
                // 直接返回 outer coverage。
                return outer_coverage;
            }
            // inner coverage 取反后挖空描边内部。
            let inner_coverage = sdf_to_coverage(-rounded_rect_sdf(
                // 传入已经内缩原点的 inner 局部采样点。
                inner_local,
                // 传入 inner 尺寸。
                inner_size,
                // 传入 inner 四角半径。
                inner_radius,
            ));
            // 返回同心双 SDF 的覆盖率乘积。
            return outer_coverage * inner_coverage;
        }
        // 无圆角填充在 draw quad 内始终完全覆盖。
        if radius.iter().all(|value| *value <= 0.0) {
            // 返回完整 coverage。
            return 1.0;
        }
        // 圆角填充使用同一 SDF 与一像素线性 coverage。
        sdf_to_coverage(rounded_rect_sdf(draw_local, rect_size, radius))
    }
}

// 计算左上、右上、右下、左下四角顺序的圆角矩形有符号距离。
fn rounded_rect_sdf(local: [f32; 2], size: [f32; 2], radius: [f32; 4]) -> f32 {
    // 计算矩形半尺寸。
    let half_size = [size[0] * 0.5, size[1] * 0.5];
    // 把局部采样点转换到矩形中心坐标。
    let q = [local[0] - half_size[0], local[1] - half_size[1]];
    // 按采样点象限选择对应圆角半径。
    let corner_radius = if q[0] < 0.0 {
        // 左侧按纵向选择左上或左下圆角。
        if q[1] < 0.0 { radius[0] } else { radius[3] }
    } else {
        // 右侧按纵向选择右上或右下圆角。
        if q[1] < 0.0 { radius[1] } else { radius[2] }
    };
    // 计算减去半尺寸并加回圆角半径的距离向量。
    let d = [
        // 横向圆角距离。
        q[0].abs() - half_size[0] + corner_radius,
        // 纵向圆角距离。
        q[1].abs() - half_size[1] + corner_radius,
    ];
    // 计算圆角外部的欧氏距离。
    let outside = d[0].max(0.0).hypot(d[1].max(0.0));
    // 计算矩形内部的负距离。
    let inside = d[0].max(d[1]).min(0.0);
    // 合并内外距离并移除圆角半径偏置。
    outside + inside - corner_radius
}

// 把有符号距离转换为固定一像素线性 coverage。
fn sdf_to_coverage(distance: f32) -> f32 {
    // 以像素中心为零并把半像素两侧夹紧到零和一。
    (0.5 - distance).clamp(0.0, 1.0)
}

// 锁定共享 Shape 参数、像素语义以及两个 native shader 的编码约束。
#[cfg(test)]
mod tests {
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
        let d3d_source = include_str!("../../presentation/graphics/d3d11/platform/pipeline/mod.rs");
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
        let gl_source = include_str!("../../presentation/graphics/opengl/raster/rhi_shaders.rs");
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
        let gl_pipeline_source =
            include_str!("../../presentation/graphics/opengl/raster/rhi_device_pipeline.rs");
        // Shape 语义必须选择 Shape 专属顶点阶段。
        assert!(
            gl_pipeline_source.contains("(rhi_shaders::SHAPE_VERTEX, rhi_shaders::SHAPE_FRAGMENT)")
        );
        // Sector 语义必须选择 Sector 专属顶点阶段。
        assert!(gl_pipeline_source.contains(
            "PipelineKind::Sector => (rhi_shaders::SECTOR_VERTEX, rhi_shaders::SECTOR_FRAGMENT)"
        ));
    }
}
