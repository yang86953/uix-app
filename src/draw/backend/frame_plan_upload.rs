//! FramePlan 拥有的类型化顶点、索引与 Uniform 上传载荷。

// 使用共享引用计数切片保存不可变顶点和索引事实，避免帧计划复制大网格。
use std::sync::Arc;

// 引入已经由共享 RHI 冻结字段语义的各类常量值对象。
use crate::native::present::rhi::{
    IndexFormat, PipelineUniformLayout, PipelineVertexLayout, RhiBlurRasterParams,
    RhiGradientRasterParams, RhiMeshRasterParams, RhiMsdfRasterParams, RhiSampledRasterParams,
    RhiSectorRasterParams, RhiShadowRasterParams, RhiShapeRasterParams,
};

// 保存 FramePlan 允许上传的封闭顶点布局与浮点值。
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FrameVertexPayload {
    // 保存只含物理位置 float2 的顶点流。
    PositionF32x2(Arc<[f32]>),
    // 保存物理位置、UV 与颜色组成的固定 float8 顶点流。
    PositionUvColorF32(Arc<[f32]>),
}

// 为类型化顶点载荷提供构造、验证与唯一字节编码入口。
impl FrameVertexPayload {
    // 构造 position-float2 顶点载荷。
    pub(crate) fn position_f32x2(values: impl Into<Arc<[f32]>>) -> Self {
        // 保留调用方已经冻结的浮点顺序。
        Self::PositionF32x2(values.into())
    }

    // 构造 position/uv/color-float8 顶点载荷。
    pub(crate) fn position_uv_color_f32(values: impl Into<Arc<[f32]>>) -> Self {
        // 保留调用方已经冻结的浮点顺序。
        Self::PositionUvColorF32(values.into())
    }

    // 返回本载荷声明的唯一共享顶点布局。
    pub(crate) const fn layout(&self) -> PipelineVertexLayout {
        // 通过封闭枚举映射，禁止调用方另传可能冲突的 stride。
        match self {
            // position-float2 对应两个浮点的共享布局。
            Self::PositionF32x2(_) => PipelineVertexLayout::PositionF32x2,
            // sampled 顶点对应八个浮点的共享布局。
            Self::PositionUvColorF32(_) => PipelineVertexLayout::PositionUvColorF32,
        }
    }

    // 借出不可变浮点序列供验证和编码共同消费。
    pub(crate) fn values(&self) -> &[f32] {
        // 两个变体都只暴露相同的只读浮点视图。
        match self {
            // 借出 position-float2 数据。
            Self::PositionF32x2(values) => values,
            // 借出 sampled float8 数据。
            Self::PositionUvColorF32(values) => values,
        }
    }

    // 从唯一布局和浮点序列派生完整顶点数量。
    pub(crate) fn vertex_count(&self) -> Option<u32> {
        // 读取本布局每个顶点包含的浮点数量。
        let floats_per_vertex = self.layout().stride_bytes() as usize / std::mem::size_of::<f32>();
        // 读取唯一浮点序列。
        let values = self.values();
        // 不为残缺载荷伪造完整顶点数量。
        if values.len() % floats_per_vertex != 0 {
            // 残缺顶点没有可供 DrawRange 比较的数量。
            return None;
        }
        // 将完整顶点数量收敛到 DrawRange 的 u32 值域。
        u32::try_from(values.len() / floats_per_vertex).ok()
    }

    // 验证载荷非空、有限并包含完整顶点。
    pub(crate) fn is_valid(&self) -> bool {
        // 读取本布局每个顶点包含的浮点数量。
        let floats_per_vertex = self.layout().stride_bytes() as usize / std::mem::size_of::<f32>();
        // 读取唯一浮点序列。
        let values = self.values();
        // 空载荷、残缺顶点或非有限值都不能进入 Adapter。
        if values.is_empty()
            // 每个顶点必须严格匹配共享 stride。
            || values.len() % floats_per_vertex != 0
            // 非有限几何会让不同 rasterizer 产生未定义差异。
            || !values.iter().all(|value| value.is_finite())
        {
            // 在共享边界统一拒绝基础载荷错误。
            return false;
        }
        // 只有带颜色的 sampled 顶点需要额外的单位颜色域约束。
        match self {
            // position-float2 不携带颜色，保持原有有限值行为。
            Self::PositionF32x2(_) => true,
            // sampled float8 的每个完整顶点都必须携带单位颜色。
            Self::PositionUvColorF32(_) => values.chunks_exact(floats_per_vertex).all(
                // 逐顶点检查颜色字段而不影响位置与 UV 的值域。
                |vertex| {
                    // 颜色字段固定位于 float8 的索引 4 到 7。
                    vertex[4..8].iter().all(
                        // 共享契约只接受闭区间 [0,1] 的颜色通道。
                        |color| *color >= 0.0 && *color <= 1.0,
                    )
                },
            ),
        }
    }

    // 返回编码后的确定字节数量。
    pub(crate) fn size_bytes(&self) -> usize {
        // 浮点序列使用紧密 IEEE f32 排列。
        self.values().len() * std::mem::size_of::<f32>()
    }

    // 在唯一 FramePlan 执行器内编码为当前 host 的紧密字节。
    pub(crate) fn encode_ne_bytes(&self) -> Vec<u8> {
        // 为完整顶点流预留精确容量。
        let mut bytes = Vec::with_capacity(self.size_bytes());
        // 按 Drawing 已冻结的顺序编码每个浮点。
        for value in self.values() {
            // Adapter 与执行器位于同一 host，使用 native-endian 上传。
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        // 返回只在 Device 原语边界存在的字节表示。
        bytes
    }
}

// 保存 FramePlan 唯一允许的索引格式与不可变索引值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FrameIndexPayload {
    // 保存紧密的 host-order u32 索引序列。
    Uint32(Arc<[u32]>),
}

// 为索引载荷提供封闭构造、验证、范围查询与编码入口。
impl FrameIndexPayload {
    // 通过唯一构造器接收共享不可变的 u32 索引值。
    pub(crate) fn uint32(values: impl Into<Arc<[u32]>>) -> Self {
        // 将调用方值收回 FramePlan 的不可变所有权。
        Self::Uint32(values.into())
    }

    // 返回索引载荷的唯一共享格式。
    pub(crate) const fn format(&self) -> IndexFormat {
        // 封闭枚举禁止调用方另传格式。
        match self {
            // 当前唯一载荷变体固定为 Uint32。
            Self::Uint32(_) => IndexFormat::Uint32,
        }
    }

    // 返回只读索引值切片。
    pub(crate) fn values(&self) -> &[u32] {
        // 只投影封闭载荷内部的共享值，不暴露可变存储。
        match self {
            // 返回 u32 序列的只读视图。
            Self::Uint32(values) => values,
        }
    }

    // 验证索引载荷非空且数量可表达为共享 u32 计数。
    pub(crate) fn is_valid(&self) -> bool {
        // 与可用计数投影共享同一非空和 u32 值域门禁。
        self.index_count().is_some()
    }

    // 返回可安全传入 DrawRange 的索引数量。
    pub(crate) fn index_count(&self) -> Option<u32> {
        // 空载荷不得向 DrawRange 伪造可用的零计数。
        if self.values().is_empty() {
            // 用无结果保留索引载荷的非空不变式。
            return None;
        }
        // 索引数量通过 checked 转换收敛到 DrawRange 的 u32 值域。
        u32::try_from(self.values().len()).ok()
    }

    // 返回紧密 u32 索引序列的字节数量。
    pub(crate) fn size_bytes(&self) -> usize {
        // 每个索引固定占用一个 u32 的 native 字节宽度。
        self.values().len() * std::mem::size_of::<u32>()
    }

    // 查询 DrawRange 指定非空子范围中的最大索引值。
    pub(crate) fn max_index_in_range(&self, first: u32, count: u32) -> Option<u32> {
        // 将起始位置转换为平台切片索引并拒绝无法转换的值。
        let start = usize::try_from(first).ok()?;
        // 将范围长度转换为平台切片长度并拒绝无法转换的值。
        let length = usize::try_from(count).ok()?;
        // 使用 checked 加法计算独占末端，避免整数回绕。
        let end = start.checked_add(length)?;
        // 空范围不产生最大索引。
        if length == 0 {
            // 统一将空 DrawRange 映射为无结果。
            return None;
        }
        // 切片越界时返回无结果，合法范围再求最大值。
        self.values().get(start..end)?.iter().copied().max()
    }

    // 将索引序列编码为唯一的 native-endian 紧密字节表示。
    pub(crate) fn encode_ne_bytes(&self) -> Vec<u8> {
        // 为完整索引序列预留精确字节容量。
        let mut bytes = Vec::with_capacity(self.size_bytes());
        // 按值顺序编码每个 u32，不插入填充字节。
        for value in self.values() {
            // FramePlan 与 Device 位于同一 host，使用 native-endian 编码。
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        // 返回仅在上传边界使用的字节表示。
        bytes
    }
}

// 保存 FramePlan 允许上传的封闭 Uniform 语义值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum FrameUniformPayload {
    // 保存实心网格 viewport 与颜色常量。
    Mesh(RhiMeshRasterParams),
    // 保存 sampled、coverage 共用的 viewport 常量。
    Sampled(RhiSampledRasterParams),
    // 保存线性或径向渐变仿射常量。
    Gradient(RhiGradientRasterParams),
    // 保存填充或描边 Shape 常量。
    Shape(RhiShapeRasterParams),
    // 保存盒阴影仿射与软边常量。
    Shadow(RhiShadowRasterParams),
    // 保存双阶段高斯模糊常量。
    Blur(RhiBlurRasterParams),
    // 保存 MSDF viewport、atlas 与距离范围常量。
    Msdf(RhiMsdfRasterParams),
    // 保存扇形 viewport、矩形、颜色与角度常量。
    Sector(RhiSectorRasterParams),
}

// 为 Uniform 载荷提供闭集布局映射和唯一编码入口。
impl FrameUniformPayload {
    // 返回本载荷唯一对应的共享 Uniform 布局。
    pub(crate) const fn layout(self) -> PipelineUniformLayout {
        // 通过枚举变体完成穷尽映射，新增语义时由编译器要求更新。
        match self {
            // Mesh 值只能进入 Mesh 常量布局。
            Self::Mesh(_) => PipelineUniformLayout::Mesh,
            // Sampled 值只能进入 Sampled 常量布局。
            Self::Sampled(_) => PipelineUniformLayout::Sampled,
            // Gradient 值只能进入 Gradient 常量布局。
            Self::Gradient(_) => PipelineUniformLayout::Gradient,
            // Shape 值只能进入 Shape 常量布局。
            Self::Shape(_) => PipelineUniformLayout::Shape,
            // Shadow 值只能进入 Shadow 常量布局。
            Self::Shadow(_) => PipelineUniformLayout::Shadow,
            // Blur 值只能进入 Blur 常量布局。
            Self::Blur(_) => PipelineUniformLayout::Blur,
            // MSDF 值只能进入 MSDF 常量布局。
            Self::Msdf(_) => PipelineUniformLayout::Msdf,
            // Sector 值只能进入 Sector 常量布局。
            Self::Sector(_) => PipelineUniformLayout::Sector,
        }
    }

    // 验证 Uniform 载荷属于共享 FramePlan 值域。
    pub(crate) fn is_valid(self) -> bool {
        // 每个变体直接委托其共享值对象的冻结字段门禁。
        match self {
            // 检查 Mesh 全部字段。
            Self::Mesh(value) => value.as_f32s().iter().all(|field| field.is_finite()),
            // 检查 Sampled 全部字段。
            Self::Sampled(value) => value.as_f32s().iter().all(|field| field.is_finite()),
            // Gradient 由共享值对象同时验证有限值、模式与径向半径。
            Self::Gradient(value) => value.is_valid(),
            // 检查 Shape 全部字段。
            Self::Shape(value) => value.as_f32s().iter().all(|field| field.is_finite()),
            // 检查 Shadow 全部字段。
            Self::Shadow(value) => value.as_f32s().iter().all(|field| field.is_finite()),
            // 检查 Blur 全部字段。
            Self::Blur(value) => value.as_f32s().iter().all(|field| field.is_finite()),
            // 检查 MSDF 全部字段。
            Self::Msdf(value) => value.as_f32s().iter().all(|field| field.is_finite()),
            // 检查 Sector 全部字段。
            Self::Sector(value) => value.as_f32s().iter().all(|field| field.is_finite()),
        }
    }

    // 返回共享 PipelineContract 声明的固定字节数量。
    pub(crate) const fn size_bytes(self) -> usize {
        // 统一从布局契约读取，禁止枚举另存一套大小表。
        self.layout().size_bytes()
    }

    // 在唯一 FramePlan 执行器内编码为当前 host 的紧密字节。
    pub(crate) fn encode_ne_bytes(self) -> Vec<u8> {
        // 每个变体只调用其共享值对象拥有的唯一编码器。
        match self {
            // 编码 Mesh 固定 ABI。
            Self::Mesh(value) => value.encode_ne_bytes(),
            // 编码 Sampled 固定 ABI。
            Self::Sampled(value) => value.encode_ne_bytes(),
            // 编码 Gradient 固定 ABI。
            Self::Gradient(value) => value.encode_ne_bytes(),
            // 编码 Shape 固定 ABI。
            Self::Shape(value) => value.encode_ne_bytes(),
            // 编码 Shadow 固定 ABI。
            Self::Shadow(value) => value.encode_ne_bytes(),
            // 编码 Blur 固定 ABI。
            Self::Blur(value) => value.encode_ne_bytes(),
            // 编码 MSDF 固定 ABI。
            Self::Msdf(value) => value.encode_ne_bytes(),
            // 编码 Sector 固定 ABI。
            Self::Sector(value) => value.encode_ne_bytes(),
        }
    }
}

// 验证 FramePlan 类型化上传不会退回裸字节或错误布局。
#[cfg(test)]
mod tests {
    // 导入被测载荷与共享 viewport。
    use super::*;
    // 导入构造基础 Uniform 所需的共享 viewport。
    use crate::native::present::rhi::RhiViewport;

    // 索引载荷必须拒绝空值并保持共享格式、数量和字节大小一致。
    #[test]
    fn index_payload_validates_shape_and_range() {
        // 构造空索引载荷验证非空门禁。
        let empty = FrameIndexPayload::uint32(Vec::<u32>::new());
        // 空载荷必须无效。
        assert!(!empty.is_valid());
        // 无效载荷不得提供索引数量。
        assert_eq!(empty.index_count(), None);
        // 空载荷的范围查询必须返回无结果。
        assert_eq!(empty.max_index_in_range(0, 1), None);
        // 构造完整的 u32 索引序列。
        let payload = FrameIndexPayload::uint32([7_u32, 2, 9, 4]);
        // 非空且数量可表示的载荷必须有效。
        assert!(payload.is_valid());
        // 载荷格式必须由封闭变体固定为 Uint32。
        assert_eq!(payload.format(), IndexFormat::Uint32);
        // 索引数量必须准确投影为 u32。
        assert_eq!(payload.index_count(), Some(4));
        // 字节大小必须等于元素数乘以 u32 宽度。
        assert_eq!(payload.size_bytes(), 4 * std::mem::size_of::<u32>());
        // 合法非空子范围必须返回所选元素最大值。
        assert_eq!(payload.max_index_in_range(1, 2), Some(9));
        // 覆盖完整序列的范围也必须返回最大值。
        assert_eq!(payload.max_index_in_range(0, 4), Some(9));
        // 空范围不得伪造最大索引。
        assert_eq!(payload.max_index_in_range(0, 0), None);
        // 起点落在末端之后的范围必须被拒绝。
        assert_eq!(payload.max_index_in_range(4, 1), None);
        // 末端溢出载荷长度的范围必须被拒绝。
        assert_eq!(payload.max_index_in_range(3, 2), None);
        // 编码长度必须与类型化字节大小一致。
        let encoded = payload.encode_ne_bytes();
        // 编码不得丢失或增加任何索引字节。
        assert_eq!(encoded.len(), payload.size_bytes());
        // 以同一 native-endian 规则构造精确期望字节。
        let mut expected = Vec::new();
        // 按原始索引顺序拼接每个 u32 的 native-endian 字节。
        for value in [7_u32, 2, 9, 4] {
            // 期望表示必须保持紧密无填充布局。
            expected.extend_from_slice(&value.to_ne_bytes());
        }
        // 实际编码内容必须与唯一编码规则完全一致。
        assert_eq!(encoded, expected);
    }

    // 顶点载荷必须按声明布局验证完整且有限的顶点。
    #[test]
    fn vertex_payload_rejects_partial_or_non_finite_vertices() {
        // 构造完整三角形 position-float2 顶点。
        let valid = FrameVertexPayload::position_f32x2([0.0, 0.0, 1.0, 0.0, 0.0, 1.0]);
        // 完整有限顶点必须通过。
        assert!(valid.is_valid());
        // 编码长度必须等于六个浮点。
        assert_eq!(valid.size_bytes(), 6 * std::mem::size_of::<f32>());
        // 残缺 float8 顶点必须被拒绝。
        assert!(!FrameVertexPayload::position_uv_color_f32([0.0; 7]).is_valid());
        // 非有限 position 顶点必须在进入 Adapter 前被拒绝。
        assert!(!FrameVertexPayload::position_f32x2([f32::NAN, 0.0]).is_valid());
        // position-float2 的六个浮点必须派生三个完整顶点。
        assert_eq!(valid.vertex_count(), Some(3));
        // 残缺 float8 载荷不得派生顶点数量。
        assert_eq!(
            FrameVertexPayload::position_uv_color_f32([0.0; 7]).vertex_count(),
            None
        );
    }

    // float8 顶点载荷必须按完整布局派生顶点数量。
    #[test]
    fn vertex_payload_derives_float8_vertex_count() {
        // 构造两个完整的 position/uv/color 顶点。
        let payload = FrameVertexPayload::position_uv_color_f32([0.0; 16]);
        // 每八个浮点必须派生一个顶点。
        assert_eq!(payload.vertex_count(), Some(2));
    }

    // sampled 顶点颜色必须在共享 FramePlan 的单位域内。
    #[test]
    fn vertex_payload_rejects_non_unit_sampled_colors() {
        // 单位颜色域的两个边界值必须通过。
        assert!(
            FrameVertexPayload::position_uv_color_f32([0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.5, 1.0,])
                .is_valid()
        );
        // 负颜色通道必须在进入任一 Adapter 前被拒绝。
        assert!(
            !FrameVertexPayload::position_uv_color_f32([0.0, 0.0, 0.0, 0.0, -0.01, 0.5, 0.5, 1.0,])
                .is_valid()
        );
        // 超过单位上界的颜色通道必须被拒绝。
        assert!(
            !FrameVertexPayload::position_uv_color_f32([0.0, 0.0, 0.0, 0.0, 0.5, 1.01, 0.5, 1.0,])
                .is_valid()
        );
        // 构造两个完整顶点，证明门禁会逐顶点检查而不是只看首项。
        let mut second_vertex_invalid = [0.0; 16];
        // 首个顶点使用完整合法颜色。
        second_vertex_invalid[4..8].copy_from_slice(&[0.0, 0.5, 1.0, 1.0]);
        // 第二个顶点在蓝通道越过单位上界。
        second_vertex_invalid[14] = 1.01;
        // 任一后续顶点越界都必须拒绝整个类型化载荷。
        assert!(!FrameVertexPayload::position_uv_color_f32(second_vertex_invalid).is_valid());
        // NaN 颜色必须被通用有限值门禁拒绝。
        assert!(
            !FrameVertexPayload::position_uv_color_f32([
                0.0,
                0.0,
                0.0,
                0.0,
                f32::NAN,
                0.5,
                0.5,
                1.0,
            ])
            .is_valid()
        );
        // 无穷颜色必须被通用有限值门禁拒绝。
        assert!(
            !FrameVertexPayload::position_uv_color_f32([
                0.0,
                0.0,
                0.0,
                0.0,
                0.5,
                f32::INFINITY,
                0.5,
                1.0,
            ])
            .is_valid()
        );
    }

    // Uniform 载荷布局和编码长度必须由同一闭集映射拥有。
    #[test]
    fn uniform_payload_layout_owns_exact_encoded_size() {
        // 构造可区分的确定 viewport。
        let viewport = RhiViewport {
            // 保存测试宽度。
            width: 320.0,
            // 保存测试高度。
            height: 180.0,
        };
        // 构造 sampled 类型化常量。
        let payload = FrameUniformPayload::Sampled(RhiSampledRasterParams::new(viewport));
        // 闭集布局必须映射到 Sampled。
        assert_eq!(payload.layout(), PipelineUniformLayout::Sampled);
        // 编码长度必须与同一布局大小完全一致。
        assert_eq!(payload.encode_ne_bytes().len(), payload.size_bytes());
        // 构造器生成的确定字段必须全部有限。
        assert!(payload.is_valid());
    }
}
