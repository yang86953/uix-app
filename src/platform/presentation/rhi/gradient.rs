//! Drawing System 在共享 GPU Raster/RHI 边界拥有的 Gradient 像素契约。

// 引入共享物理视口值，避免 Gradient 契约依赖任何平台 API 类型。
use super::RhiViewport;

// 固定 Gradient uniform 的二十九个 float4 ABI 总字节数（基础九组 + 十六色 + 四组位置）。
pub(crate) const GRADIENT_UNIFORM_BYTES: usize = 464;

// 固定 Gradient uniform 的 float 数量。
const GRADIENT_UNIFORM_FLOATS: usize =
    // 用字节数除以单个浮点大小得到紧密字段数量。
    GRADIENT_UNIFORM_BYTES / std::mem::size_of::<f32>();

// viewport float2 在 Gradient ABI 中的起始 float 索引。
pub(crate) const GRADIENT_VIEWPORT_FLOAT_OFFSET: usize = 0;

// origin 与 edge_x 组成的 float4 在 Gradient ABI 中的起始索引。
pub(crate) const GRADIENT_ORIGIN_EDGE_X_FLOAT_OFFSET: usize = 4;

// edge_y 与 padding 组成的 float4 在 Gradient ABI 中的起始索引。
pub(crate) const GRADIENT_EDGE_Y_FLOAT_OFFSET: usize = 8;

// 起始颜色 float4 在 Gradient ABI 中的起始索引。
pub(crate) const GRADIENT_COLOR_A_FLOAT_OFFSET: usize = 12;

// 结束颜色 float4 在 Gradient ABI 中的起始索引。
pub(crate) const GRADIENT_COLOR_B_FLOAT_OFFSET: usize = 16;

// mode、方向或半径参数 float4 在 Gradient ABI 中的起始索引。
pub(crate) const GRADIENT_PARAMS_FLOAT_OFFSET: usize = 20;
// 四角圆角掩码半径（tl/tr/br/bl，逻辑像素）float4 的起始索引。
pub(crate) const GRADIENT_MASK_RADIUS_FLOAT_OFFSET: usize = 24;

// quad 逻辑宽高与掩码单位矩形起点 float4 的起始索引。
pub(crate) const GRADIENT_QUAD_SIZE_FLOAT_OFFSET: usize = 28;

// 掩码单位矩形宽高 float4 的起始索引。
pub(crate) const GRADIENT_MASK_SIZE_FLOAT_OFFSET: usize = 32;

// 保存已经冻结的平台无关 Gradient 仿射几何、颜色与采样参数。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RhiGradientRasterParams {
    // 固定 float4 保存几何、颜色、掩码及可选色标。
    values: [f32; GRADIENT_UNIFORM_FLOATS],
}

// 为共享 Gradient 像素契约提供唯一派生与编码入口。
impl RhiGradientRasterParams {
    // 从已经完成物理坐标 lowering 的四角和颜色构造固定 Gradient ABI。
    pub(crate) fn new(
        // 接收物理绘制目标尺寸。
        viewport: RhiViewport,
        // 接收固定 TL、TR、BR、BL 顺序的仿射四角。
        corners: [[f32; 2]; 4],
        // 接收起始 straight-alpha 颜色。
        color_a: [f32; 4],
        // 接收结束 straight-alpha 颜色。
        color_b: [f32; 4],
        // 接收 mode、方向或径向半径参数；线性模式的 params[2]/[3] 由 graphics
        // lowering 按渐变局部矩形宽高填入，与 CPU `linear_gradient_t` 同源。
        params: [f32; 4],
        // 接收已归一化的四角掩码半径（tl/tr/br/bl，逻辑像素）；全零关闭掩码。
        mask_radius: [f32; 4],
        // 接收 quad 的逻辑宽高，供掩码把单位局部坐标换算为像素空间。
        quad_size: [f32; 2],
        // 接收掩码参考矩形在 quad 单位坐标空间中的 x/y/w/h。
        mask_rect: [f32; 4],
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
        // 按固定九个基础 float4 构造 Gradient 参数，色标区稍后零初始化。
        let base = [
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
            // edge_y.x。
            edge_y[0],
            // edge_y.y。
            edge_y[1],
            // edge_y padding.x。
            0.0,
            // edge_y padding.y。
            0.0,
            // color_a.r。
            color_a[0],
            // color_a.g。
            color_a[1],
            // color_a.b。
            color_a[2],
            // color_a.a。
            color_a[3],
            // color_b.r。
            color_b[0],
            // color_b.g。
            color_b[1],
            // color_b.b。
            color_b[2],
            // color_b.a。
            color_b[3],
            // params.mode。
            params[0],
            // params.direction_or_inner_radius。
            params[1],
            // params.linear_local_width_or_radial_outer_radius。
            params[2],
            // params.linear_local_height。
            params[3],
            // mask_radius.tl。
            mask_radius[0],
            // mask_radius.tr。
            mask_radius[1],
            // mask_radius.br。
            mask_radius[2],
            // mask_radius.bl。
            mask_radius[3],
            // quad 逻辑宽度。
            quad_size[0],
            // quad 逻辑高度。
            quad_size[1],
            // mask_rect 单位 x。
            mask_rect[0],
            // mask_rect 单位 y。
            mask_rect[1],
            // mask_rect 单位宽度。
            mask_rect[2],
            // mask_rect 单位高度。
            mask_rect[3],
            // 掩码尺寸 float4 保留填充。
            0.0,
            0.0,
        ];
        // 返回已经冻结的共享 Gradient 值对象。
        let mut values = [0.0; GRADIENT_UNIFORM_FLOATS];
        values[..36].copy_from_slice(&base);
        Self { values }
    }

    // 追加多色标常量，传统模式保留其原有前九组 float4。
    pub(crate) fn with_linear_stops(mut self, stops: Option<RhiLinearGradientStops>) -> Self {
        if let Some(stops) = stops {
            self.values[20..24].copy_from_slice(&[2.0, stops.count as f32, stops.axis[0], stops.axis[1]]);
            for (i, color) in stops.colors.iter().enumerate() {
                self.values[36 + i * 4..40 + i * 4].copy_from_slice(color);
            }
            self.values[100..116].copy_from_slice(&stops.offsets);
        }
        self
    }

    // 返回 Adapter 可按共享字段索引读取的 float ABI。
    pub(crate) const fn as_f32s(&self) -> &[f32; GRADIENT_UNIFORM_FLOATS] {
        // 借出不可变数组，禁止下层重新组织字段。
        &self.values
    }

    // 验证 Gradient 的有限值、模式闭集与径向外半径值域。
    pub(crate) fn is_valid(&self) -> bool {
        // 先拒绝所有会让两个 shader 产生未定义结果的非有限字段。
        if !self.values.iter().all(|value| value.is_finite()) {
            // 非有限值不得进入任一 Adapter。
            return false;
        }
        // 读取共享 ABI 中唯一的模式字段。
        let mode = self.values[GRADIENT_PARAMS_FLOAT_OFFSET];
        // 模式只允许传统线性零、径向一或多色标二，禁止 Adapter 解释未知模式。
        if mode != 0.0 && mode != 1.0 && mode != 2.0 {
            // 未知模式属于共享参数错误。
            return false;
        }
        if mode == 2.0 {
            let count = self.values[21];
            return (2.0..=16.0).contains(&count) && count.fract() == 0.0
                && (0.99999..=1.00001).contains(&(self.values[22].abs() + self.values[23].abs()))
                && self.values[100..100 + count as usize].iter().all(|v| (0.0..=1.0).contains(v))
                && self.values[100..100 + count as usize].windows(2).all(|w| w[0] <= w[1]);
        }
        // 线性模式保持既有有限值语义，不额外改变其长度规则。
        if mode == 0.0 {
            // 线性字段已经通过通用有限值门禁。
            return true;
        }
        // 读取共享 ABI 中唯一的径向外半径字段。
        let outer_radius = self.values[GRADIENT_PARAMS_FLOAT_OFFSET + 2];
        // 径向外半径必须为严格正值，避免两端 shader 分叉。
        outer_radius > 0.0
    }

    // 把共享 Gradient ABI 编码为当前 host 的紧密字节载荷。
    pub(crate) fn encode_ne_bytes(&self) -> Vec<u8> {
        // 为完整 ABI 预留精确容量。
        let mut bytes = Vec::with_capacity(GRADIENT_UNIFORM_BYTES);
        // 逐个保持 IEEE float 的原始 native-endian 表示。
        for value in self.values {
            // Adapter 与共享层运行在同一 host，因此直接追加 native-endian 字节。
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        // 返回可直接上传到 uniform buffer 的紧密载荷。
        bytes
    }
}

// RHI 多色标的有界 straight-alpha 载荷，颜色在 lowering 前已解析并应用 opacity。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RhiLinearGradientStops {
    pub(crate) count: u8,
    pub(crate) axis: [f32; 2],
    pub(crate) colors: [[f32; 4]; 16],
    pub(crate) offsets: [f32; 16],
}

// OpenGL 将同一紧密 ABI 映射到 uniform 数组。
pub(crate) const GRADIENT_STOP_COLORS_FLOAT_OFFSET: usize = 36;
pub(crate) const GRADIENT_STOP_OFFSETS_FLOAT_OFFSET: usize = 100;
