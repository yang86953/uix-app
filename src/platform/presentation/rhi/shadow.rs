//! Drawing System 在共享 GPU Raster/RHI 边界拥有的 Shadow 像素契约。

// 引入共享物理视口值，避免 Shadow 契约依赖任何平台 API 类型。
use super::RhiViewport;

// 固定 Shadow uniform 的六个 float4 ABI 总字节数。
pub(crate) const SHADOW_UNIFORM_BYTES: usize = 96;

// 固定 Shadow uniform 的 float 数量。
const SHADOW_UNIFORM_FLOATS: usize =
    // 用总字节数除以单个浮点大小得到紧密字段数量。
    SHADOW_UNIFORM_BYTES / std::mem::size_of::<f32>();

// viewport float2 在 Shadow ABI 中的起始 float 索引。
pub(crate) const SHADOW_VIEWPORT_FLOAT_OFFSET: usize = 0;

// 仿射原点与 X 边组成的 float4 在 Shadow ABI 中的起始索引。
pub(crate) const SHADOW_ORIGIN_EDGE_X_FLOAT_OFFSET: usize = 4;

// 直通颜色 float4 在 Shadow ABI 中的起始索引。
pub(crate) const SHADOW_COLOR_FLOAT_OFFSET: usize = 8;

// 四角半径 float4 在 Shadow ABI 中的起始索引。
pub(crate) const SHADOW_RADIUS_FLOAT_OFFSET: usize = 12;

// 仿射 Y 边与两轴模糊量组成的 float4 在 Shadow ABI 中的起始索引。
pub(crate) const SHADOW_EDGE_Y_BLUR_FLOAT_OFFSET: usize = 16;

// 本体尺寸、环境曲线标记与 padding 组成的 float4 在 Shadow ABI 中的起始索引。
pub(crate) const SHADOW_BODY_SIZE_AMBIENT_FLOAT_OFFSET: usize = 20;

// 保存已经冻结的平台无关 Shadow 仿射几何、颜色和软边参数。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RhiShadowRasterParams {
    // 六个 float4 依次保存 viewport、仿射 X 轴、颜色、圆角、仿射 Y 轴与本体参数。
    values: [f32; SHADOW_UNIFORM_FLOATS],
}

// 为共享 Shadow 像素契约提供唯一派生与编码入口。
impl RhiShadowRasterParams {
    // 从已经完成物理坐标 lowering 的阴影事实构造固定 Shadow ABI。
    pub(crate) fn new(
        // 接收物理绘制目标尺寸。
        viewport: RhiViewport,
        // 接收固定 TL、TR、BR、BL 顺序的阴影扩展四角。
        corners: [[f32; 2]; 4],
        // 接收 straight-alpha 阴影颜色。
        color: [f32; 4],
        // 接收左上、右上、右下、左下顺序的本体圆角。
        radius: [f32; 4],
        // 接收两个物理轴上的模糊量。
        blur: [f32; 2],
        // 接收不含模糊扩展的阴影本体尺寸。
        body_size: [f32; 2],
        // 接收是否使用环境阴影覆盖曲线的唯一语义标记。
        ambient: bool,
    ) -> Self {
        // TL 是单位 quad 仿射映射的唯一物理原点。
        let origin = corners[0];
        // TR 减去 TL 得到单位局部 X 轴的物理边。
        let edge_x = [
            // 保存 X 边的物理横向分量。
            corners[1][0] - origin[0],
            // 保存 X 边的物理纵向分量。
            corners[1][1] - origin[1],
        ];
        // BL 减去 TL 得到单位局部 Y 轴的物理边。
        let edge_y = [
            // 保存 Y 边的物理横向分量。
            corners[3][0] - origin[0],
            // 保存 Y 边的物理纵向分量。
            corners[3][1] - origin[1],
        ];
        // 按固定六个 float4 ABI 构造不可分叉的 Shadow 参数。
        let values = [
            // viewport.width。
            viewport.width,
            // viewport.height。
            viewport.height,
            // viewport padding.x。
            0.0,
            // viewport padding.y。
            0.0,
            // origin.x。
            origin[0],
            // origin.y。
            origin[1],
            // edge_x.x。
            edge_x[0],
            // edge_x.y。
            edge_x[1],
            // color.r。
            color[0],
            // color.g。
            color[1],
            // color.b。
            color[2],
            // color.a。
            color[3],
            // radius.tl。
            radius[0],
            // radius.tr。
            radius[1],
            // radius.br。
            radius[2],
            // radius.bl。
            radius[3],
            // edge_y.x。
            edge_y[0],
            // edge_y.y。
            edge_y[1],
            // blur.x。
            blur[0],
            // blur.y。
            blur[1],
            // body_size.width。
            body_size[0],
            // body_size.height。
            body_size[1],
            // ambient 标记使用精确零或一，避免 Adapter 自行解释布尔布局。
            if ambient { 1.0 } else { 0.0 },
            // 保留最后一个 16-byte 对齐槽。
            0.0,
        ];
        // 返回已经冻结的共享 Shadow 值对象。
        Self { values }
    }

    // 返回 Adapter 可按共享字段索引读取的 float ABI。
    pub(crate) const fn as_f32s(&self) -> &[f32; SHADOW_UNIFORM_FLOATS] {
        // 借出不可变数组，禁止下层重新组织字段。
        &self.values
    }

    // 把共享 Shadow ABI 编码为当前 host 的紧密字节载荷。
    pub(crate) fn encode_ne_bytes(&self) -> Vec<u8> {
        // 为完整 ABI 预留精确容量。
        let mut bytes = Vec::with_capacity(SHADOW_UNIFORM_BYTES);
        // 逐个保持 IEEE float 的原始 native-endian 表示。
        for value in self.values {
            // Adapter 与共享层运行在同一 host，因此直接追加 native-endian 字节。
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        // 返回可直接上传到 uniform buffer 的紧密载荷。
        bytes
    }
}
