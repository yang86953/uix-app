//! FramePlan 拥有的类型化顶点与 Uniform 上传载荷。

// 使用共享引用计数切片保存不可变顶点事实，避免帧计划复制大网格。
use std::sync::Arc;

// 引入已经由共享 RHI 冻结字段语义的各类常量值对象。
use crate::native::present::rhi::{
    PipelineUniformLayout, PipelineVertexLayout, RhiBlurRasterParams, RhiGradientRasterParams,
    RhiMeshRasterParams, RhiMsdfRasterParams, RhiSampledRasterParams, RhiSectorRasterParams,
    RhiShadowRasterParams, RhiShapeRasterParams,
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

    // 验证载荷非空、有限并包含完整顶点。
    pub(crate) fn is_valid(&self) -> bool {
        // 读取本布局每个顶点包含的浮点数量。
        let floats_per_vertex = self.layout().stride_bytes() as usize / std::mem::size_of::<f32>();
        // 读取唯一浮点序列。
        let values = self.values();
        // 空载荷或残缺顶点都不能进入 Adapter。
        !values.is_empty()
            // 每个顶点必须严格匹配共享 stride。
            && values.len() % floats_per_vertex == 0
            // 非有限几何会让不同 rasterizer 产生未定义差异。
            && values.iter().all(|value| value.is_finite())
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

    // 验证全部 Uniform 浮点都保持有限。
    pub(crate) fn is_finite(self) -> bool {
        // 每个变体直接借用其共享值对象的冻结字段数组。
        match self {
            // 检查 Mesh 全部字段。
            Self::Mesh(value) => value.as_f32s().iter().all(|field| field.is_finite()),
            // 检查 Sampled 全部字段。
            Self::Sampled(value) => value.as_f32s().iter().all(|field| field.is_finite()),
            // 检查 Gradient 全部字段。
            Self::Gradient(value) => value.as_f32s().iter().all(|field| field.is_finite()),
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
        assert!(payload.is_finite());
    }
}
