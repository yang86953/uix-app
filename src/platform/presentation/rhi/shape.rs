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
