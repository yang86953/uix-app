//! Drawing System 在共享 GPU Raster/RHI 边界拥有的 MSDF 常量契约。

// 引入共享物理尺寸与 viewport，避免 MSDF 契约依赖平台 API 类型。
use super::{RhiExtent, RhiViewport};

// 固定两个 float4 组成的 MSDF uniform 总字节数。
pub(crate) const MSDF_UNIFORM_BYTES: usize = 32;

// 固定 MSDF uniform 的 float 数量。
const MSDF_UNIFORM_FLOATS: usize =
    // 用总字节数除以单个浮点大小得到紧密字段数量。
    MSDF_UNIFORM_BYTES / std::mem::size_of::<f32>();

// viewport 宽高在 MSDF ABI 中的起始 float 索引。
pub(crate) const MSDF_VIEWPORT_FLOAT_OFFSET: usize = 0;

// sampled atlas 宽高在 MSDF ABI 中的起始 float 索引。
pub(crate) const MSDF_TEXTURE_SIZE_FLOAT_OFFSET: usize = 2;

// MSDF 编码距离范围在 ABI 中的 float 索引。
pub(crate) const MSDF_RANGE_FLOAT_OFFSET: usize = 4;

// 保存已经冻结的平台无关 viewport、atlas 尺寸与距离范围。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RhiMsdfRasterParams {
    // 两个 float4 同时包含五个有效字段和三个确定性 padding。
    values: [f32; MSDF_UNIFORM_FLOATS],
}

// 为共享 MSDF 常量契约提供唯一字段排列与编码入口。
impl RhiMsdfRasterParams {
    // 从已经完成物理 lowering 的 viewport、纹理尺寸和距离范围构造固定 ABI。
    pub(crate) fn new(
        // 接收当前 render target 的物理 viewport。
        viewport: RhiViewport,
        // 接收当前绑定 atlas 或临时字形纹理的物理尺寸。
        texture_extent: RhiExtent,
        // 接收与 CPU MSDF 编码一致的距离范围。
        distance_range: f32,
    ) -> Self {
        // 从全零数组开始，确保寄存器 padding 保持确定值。
        let mut values = [0.0f32; MSDF_UNIFORM_FLOATS];
        // 保存 viewport 宽度。
        values[MSDF_VIEWPORT_FLOAT_OFFSET] = viewport.width;
        // 保存 viewport 高度。
        values[MSDF_VIEWPORT_FLOAT_OFFSET + 1] = viewport.height;
        // 保存 sampled texture 宽度。
        values[MSDF_TEXTURE_SIZE_FLOAT_OFFSET] = texture_extent.width as f32;
        // 保存 sampled texture 高度。
        values[MSDF_TEXTURE_SIZE_FLOAT_OFFSET + 1] = texture_extent.height as f32;
        // 保存 shader 将编码距离换算到屏幕像素所需的范围。
        values[MSDF_RANGE_FLOAT_OFFSET] = distance_range;
        // 返回已经冻结的共享 MSDF 值对象。
        Self { values }
    }

    // 返回 Adapter 可按共享字段索引读取的 float ABI。
    pub(crate) const fn as_f32s(&self) -> &[f32; MSDF_UNIFORM_FLOATS] {
        // 借出不可变数组，禁止下层重新组织字段。
        &self.values
    }

    // 把共享 MSDF ABI 编码为当前 host 的紧密字节载荷。
    pub(crate) fn encode_ne_bytes(&self) -> Vec<u8> {
        // 为完整 ABI 预留精确容量。
        let mut bytes = Vec::with_capacity(MSDF_UNIFORM_BYTES);
        // 逐个保持 IEEE float 的原始 native-endian 表示。
        for value in self.values {
            // Adapter 与共享层运行在同一 host，因此直接追加 native-endian 字节。
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        // 返回可直接上传到 uniform buffer 的紧密载荷。
        bytes
    }
}
