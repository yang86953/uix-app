//! Drawing System 在共享 GPU Raster/RHI 边界拥有的 Gradient 像素契约。

// 引入共享物理视口值，避免 Gradient 契约依赖任何平台 API 类型。
use super::RhiViewport;

// 固定 Gradient uniform 的六个 float4 ABI 总字节数。
pub(crate) const GRADIENT_UNIFORM_BYTES: usize = 96;

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

// 保存已经冻结的平台无关 Gradient 仿射几何、颜色与采样参数。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RhiGradientRasterParams {
    // 六个 float4 依次保存 viewport、origin/edge_x、edge_y、两色与参数。
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
        // 接收 mode、方向或径向半径参数。
        mut params: [f32; 4],
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
        // 线性模式需要两条仿射边的物理长度保持既有方向插值语义。
        if params[0] < 0.5 {
            // 共享层唯一计算 X 边长度，禁止 Adapter 使用不同公式。
            params[2] = edge_x[0].hypot(edge_x[1]);
            // 共享层唯一计算 Y 边长度，禁止 Adapter 使用不同公式。
            params[3] = edge_y[0].hypot(edge_y[1]);
        }
        // 按固定六个 float4 ABI 构造不可分叉的 Gradient 参数。
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
            // params.outer_radius_or_edge_x_length。
            params[2],
            // params.edge_y_length。
            params[3],
        ];
        // 返回已经冻结的共享 Gradient 值对象。
        Self { values }
    }

    // 返回 Adapter 可按共享字段索引读取的 float ABI。
    pub(crate) const fn as_f32s(&self) -> &[f32; GRADIENT_UNIFORM_FLOATS] {
        // 借出不可变数组，禁止下层重新组织字段。
        &self.values
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

// 验证共享 Gradient 派生公式和固定 ABI，禁止 Adapter 漂移后缺少参考证据。
#[cfg(test)]
mod tests {
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
}
