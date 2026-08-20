//! Drawing System 在共享 GPU Raster/RHI 边界拥有的 Blur 常量契约。

// 引入共享物理尺寸与裁剪值，避免 Blur 契约依赖任何平台 API 类型。
use super::{RhiExtent, RhiScissor};

// 固定 Blur 高斯权重槽数量，与两套 shader 的十六个 float4 保持一致。
pub(crate) const BLUR_WEIGHT_COUNT: usize = 64;

// 固定 Blur uniform 的三个 header float4 与十六个权重 float4 总字节数。
pub(crate) const BLUR_UNIFORM_BYTES: usize = 304;

// 固定 Blur uniform 的 float 数量。
const BLUR_UNIFORM_FLOATS: usize =
    // 用总字节数除以单个浮点大小得到紧密字段数量。
    BLUR_UNIFORM_BYTES / std::mem::size_of::<f32>();

// 目标与源纹理尺寸组成的 float4 在 Blur ABI 中的起始 float 索引。
pub(crate) const BLUR_SIZES_FLOAT_OFFSET: usize = 0;

// 采样区域原点与尺寸组成的 float4 在 Blur ABI 中的起始索引。
pub(crate) const BLUR_REGION_FLOAT_OFFSET: usize = 4;

// 采样方向、tap 半径与 padding 组成的 float4 在 Blur ABI 中的起始索引。
pub(crate) const BLUR_DIRECTION_TAPS_FLOAT_OFFSET: usize = 8;

// 六十四个高斯权重在 Blur ABI 中的起始索引。
pub(crate) const BLUR_WEIGHTS_FLOAT_OFFSET: usize = 12;

// 保存已经冻结的平台无关 Blur 尺寸、区域、方向与高斯权重。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RhiBlurRasterParams {
    // 三个 header float4 后紧跟十六个权重 float4。
    values: [f32; BLUR_UNIFORM_FLOATS],
}

// 为共享 Blur 常量契约提供唯一字段排列与编码入口。
impl RhiBlurRasterParams {
    // 从已经完成物理坐标 lowering 和归一化的高斯事实构造固定 Blur ABI。
    pub(crate) fn new(
        // 接收当前 render target 的物理尺寸。
        target_extent: RhiExtent,
        // 接收当前 sampled texture 的物理尺寸。
        source_extent: RhiExtent,
        // 接收已经裁到 source 范围的物理采样区域。
        region: RhiScissor,
        // 接收一个物理像素步长的水平或垂直方向。
        direction: [f32; 2],
        // 接收高斯核中心两侧的 tap 半径。
        tap_radius: u32,
        // 接收按负半径到正半径排列并零终止的归一化权重。
        weights: &[f32; BLUR_WEIGHT_COUNT],
    ) -> Self {
        // 从全零数组开始，确保 header padding 与未使用权重槽保持确定值。
        let mut values = [0.0f32; BLUR_UNIFORM_FLOATS];
        // 保存目标 viewport 宽度。
        values[BLUR_SIZES_FLOAT_OFFSET] = target_extent.width as f32;
        // 保存目标 viewport 高度。
        values[BLUR_SIZES_FLOAT_OFFSET + 1] = target_extent.height as f32;
        // 保存 sampled texture 宽度。
        values[BLUR_SIZES_FLOAT_OFFSET + 2] = source_extent.width as f32;
        // 保存 sampled texture 高度。
        values[BLUR_SIZES_FLOAT_OFFSET + 3] = source_extent.height as f32;
        // 保存物理区域左边界。
        values[BLUR_REGION_FLOAT_OFFSET] = region.x as f32;
        // 保存物理区域上边界。
        values[BLUR_REGION_FLOAT_OFFSET + 1] = region.y as f32;
        // 保存物理区域宽度。
        values[BLUR_REGION_FLOAT_OFFSET + 2] = region.width as f32;
        // 保存物理区域高度。
        values[BLUR_REGION_FLOAT_OFFSET + 3] = region.height as f32;
        // 保存每个 tap 的 X 轴像素方向。
        values[BLUR_DIRECTION_TAPS_FLOAT_OFFSET] = direction[0];
        // 保存每个 tap 的 Y 轴像素方向。
        values[BLUR_DIRECTION_TAPS_FLOAT_OFFSET + 1] = direction[1];
        // 保存 shader 从权重中心恢复采样偏移所需的半径。
        values[BLUR_DIRECTION_TAPS_FLOAT_OFFSET + 2] = tap_radius as f32;
        // 把完整高斯权重复制到唯一固定区间。
        values[BLUR_WEIGHTS_FLOAT_OFFSET..BLUR_WEIGHTS_FLOAT_OFFSET + BLUR_WEIGHT_COUNT]
            // 保留调用方已经归一化的顺序和值。
            .copy_from_slice(weights);
        // 返回已经冻结的共享 Blur 值对象。
        Self { values }
    }

    // 返回 Adapter 可按共享字段索引读取的 float ABI。
    pub(crate) const fn as_f32s(&self) -> &[f32; BLUR_UNIFORM_FLOATS] {
        // 借出不可变数组，禁止下层重新组织字段。
        &self.values
    }

    // 把共享 Blur ABI 编码为当前 host 的紧密字节载荷。
    pub(crate) fn encode_ne_bytes(&self) -> Vec<u8> {
        // 为完整 ABI 预留精确容量。
        let mut bytes = Vec::with_capacity(BLUR_UNIFORM_BYTES);
        // 逐个保持 IEEE float 的原始 native-endian 表示。
        for value in self.values {
            // Adapter 与共享层运行在同一 host，因此直接追加 native-endian 字节。
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        // 返回可直接上传到 uniform buffer 的紧密载荷。
        bytes
    }
}

// 验证共享 Blur 字段排列和完整权重区间，禁止两个 Adapter 漂移。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../tests/unit/native/present/rhi/blur__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
