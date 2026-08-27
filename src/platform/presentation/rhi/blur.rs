//! Drawing System 在共享 GPU Raster/RHI 边界拥有的 Blur 几何与常量合同。

// 引入稳定参数错误，Blur 门禁必须先于任何原生 Adapter。
use crate::core::error::{Errc, Error, Result};

// 引入共享纹理与裁剪值，避免 Blur 合同依赖任何平台 API 类型。
use super::{RhiExtent, RhiScissor, RhiTextureRegion, RhiTextureRegionBounds};

// 固定 Blur 高斯权重槽数量，与三套 shader 的十六个 float4 保持一致。
pub(crate) const BLUR_WEIGHT_COUNT: usize = 64;

// 固定 Blur uniform 的两个 header float4 与十六个权重 float4 总字节数。
pub(crate) const BLUR_UNIFORM_BYTES: usize = 288;

// 固定 Blur uniform 的 float 数量。
const BLUR_UNIFORM_FLOATS: usize =
    // 用总字节数除以单个浮点大小得到紧密字段数量。
    BLUR_UNIFORM_BYTES / std::mem::size_of::<f32>();

// 源采样域像素中心 UV 边界在 Blur ABI 中的起始 float 索引。
pub(crate) const BLUR_UV_BOUNDS_FLOAT_OFFSET: usize = 0;

// 已归一化 texel step、tap 半径与 padding 在 Blur ABI 中的起始索引。
pub(crate) const BLUR_TEXEL_STEP_TAPS_FLOAT_OFFSET: usize = 4;

// 六十四个高斯权重在 Blur ABI 中的起始索引。
pub(crate) const BLUR_WEIGHTS_FLOAT_OFFSET: usize = 8;

// 封闭 Blur 两个可分离方向，禁止调用方传入任意或未归一化向量。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RhiBlurDirection {
    // 沿 sampled texture 的一个物理 texel 横向推进。
    Horizontal,
    // 沿 sampled texture 的一个物理 texel 纵向推进。
    Vertical,
}

// 保存已经验证的一对源采样域与目标写入区域。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RhiBlurPassGeometry {
    // 保存 sampled texture 的完整物理尺寸。
    source_extent: RhiExtent,
    // 保存已经验证且非空的源采样域。
    source: RhiTextureRegionBounds,
    // 保存 render target 的完整物理尺寸。
    target_extent: RhiExtent,
    // 保存已经验证且与源域等尺寸的目标区域。
    destination: RhiTextureRegionBounds,
}

// 为 Blur 几何提供唯一构造门禁和 API 无关投影。
impl RhiBlurPassGeometry {
    // 一次绑定输入纹理、源采样域、目标纹理与目标区域。
    pub(crate) fn new(
        // sampled texture 的完整物理尺寸。
        source_extent: RhiExtent,
        // 只允许在这个源域内取样，tap 到边界时钳到边缘像素中心。
        source: RhiTextureRegion,
        // render target 的完整物理尺寸。
        target_extent: RhiExtent,
        // 只允许在这个区域写入，尺寸必须与源域一一对应。
        destination: RhiTextureRegion,
    ) -> Result<Self> {
        // 当前产品 Blur 不缩放；两个区域必须拥有唯一相同的物理尺寸。
        if source.extent() != destination.extent() {
            // 禁止 Adapter 各自补偿缩放或不同区域大小。
            return Err(Error::new(
                Errc::InvalidArgument,
                "RHI blur source and destination regions must have equal extents",
            ));
        }
        // 源域先通过共享纹理边界验证，拒绝空、溢出和越界区域。
        let source = source.validate_within(source_extent)?;
        // 目标区域使用完全相同的门禁，不允许原生 scissor 再裁一次。
        let destination = destination.validate_within(target_extent)?;
        // 四项权威事实同时建立后才允许生成顶点或 uniform。
        Ok(Self {
            source_extent,
            source,
            target_extent,
            destination,
        })
    }

    // 返回 sampled texture 的完整物理尺寸。
    pub(crate) const fn source_extent(self) -> RhiExtent {
        self.source_extent
    }

    // 返回唯一源采样域。
    pub(crate) const fn source_region(self) -> RhiTextureRegion {
        self.source.region()
    }

    // 返回 render target 的完整物理尺寸。
    pub(crate) const fn target_extent(self) -> RhiExtent {
        self.target_extent
    }

    // 返回唯一目标写入区域。
    pub(crate) const fn destination_region(self) -> RhiTextureRegion {
        self.destination.region()
    }

    // 把已验证目标区域机械投影为 DrawRasterState 使用的 scissor。
    pub(crate) const fn destination_scissor(self) -> RhiScissor {
        // 两个纹理 extent 已属于 i32 原生共同值域，区域边界可无损转换。
        RhiScissor {
            x: self.destination.region().origin().x() as i32,
            y: self.destination.region().origin().y() as i32,
            width: self.destination.region().extent().width as i32,
            height: self.destination.region().extent().height as i32,
        }
    }

    // 生成六个 position-float2/uv-float2 顶点；Adapter 只透传，不再解释 region。
    pub(crate) fn vertex_values(self) -> [f32; 24] {
        // 目标位置只从目标区域与目标 extent 派生。
        let destination = self.destination.region();
        let destination_origin = destination.origin();
        let destination_extent = destination.extent();
        let left = destination_origin.x() as f32 / self.target_extent.width as f32 * 2.0 - 1.0;
        let right = (destination_origin.x() + destination_extent.width) as f32
            / self.target_extent.width as f32
            * 2.0
            - 1.0;
        let top = 1.0 - destination_origin.y() as f32 / self.target_extent.height as f32 * 2.0;
        let bottom = 1.0
            - (destination_origin.y() + destination_extent.height) as f32
                / self.target_extent.height as f32
                * 2.0;

        // 源 UV 只从源采样域与输入纹理 extent 派生，使用像素边界供插值定位中心。
        let source = self.source.region();
        let source_origin = source.origin();
        let source_extent = source.extent();
        let uv_left = source_origin.x() as f32 / self.source_extent.width as f32;
        let uv_right =
            (source_origin.x() + source_extent.width) as f32 / self.source_extent.width as f32;
        let uv_top = source_origin.y() as f32 / self.source_extent.height as f32;
        let uv_bottom =
            (source_origin.y() + source_extent.height) as f32 / self.source_extent.height as f32;

        // 每个顶点直接绑定最终 position 与绝对 source UV，region 原点不会进入 shader 公式。
        [
            left, bottom, uv_left, uv_bottom, right, bottom, uv_right, uv_bottom, right, top,
            uv_right, uv_top, left, bottom, uv_left, uv_bottom, right, top, uv_right, uv_top, left,
            top, uv_left, uv_top,
        ]
    }

    // 从同一几何事实生成一个方向的唯一 Blur uniform。
    pub(crate) fn raster_params(
        self,
        // 方向是封闭水平或垂直选择，不允许 Adapter 归一化。
        direction: RhiBlurDirection,
        // 高斯核中心两侧的 tap 半径。
        tap_radius: u32,
        // 按负半径到正半径排列并零终止的归一化权重。
        weights: &[f32; BLUR_WEIGHT_COUNT],
    ) -> RhiBlurRasterParams {
        // 源域像素中心是所有 taps 允许访问的闭区间。
        let source = self.source.region();
        let origin = source.origin();
        let extent = source.extent();
        let inverse_width = 1.0 / self.source_extent.width as f32;
        let inverse_height = 1.0 / self.source_extent.height as f32;
        let uv_bounds = [
            (origin.x() as f32 + 0.5) * inverse_width,
            (origin.y() as f32 + 0.5) * inverse_height,
            (origin.x() + extent.width) as f32 * inverse_width - 0.5 * inverse_width,
            (origin.y() + extent.height) as f32 * inverse_height - 0.5 * inverse_height,
        ];
        // 一个 tap 恰好推进 sampled texture 的一个物理 texel。
        let texel_step = match direction {
            RhiBlurDirection::Horizontal => [inverse_width, 0.0],
            RhiBlurDirection::Vertical => [0.0, inverse_height],
        };
        // 只有共享几何门禁能够建立最终常量排列。
        RhiBlurRasterParams::new(uv_bounds, texel_step, tap_radius, weights)
    }
}

// 保存已经冻结的平台无关 Blur 源域边界、步长与高斯权重。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RhiBlurRasterParams {
    // 两个 header float4 后紧跟十六个权重 float4。
    values: [f32; BLUR_UNIFORM_FLOATS],
}

// 为共享 Blur 常量合同提供唯一字段排列与编码入口。
impl RhiBlurRasterParams {
    // 只允许 RhiBlurPassGeometry 在完整验证后构造固定 Blur ABI。
    fn new(
        uv_bounds: [f32; 4],
        texel_step: [f32; 2],
        tap_radius: u32,
        weights: &[f32; BLUR_WEIGHT_COUNT],
    ) -> Self {
        // 从全零数组开始，确保 header padding 与未使用权重槽保持确定值。
        let mut values = [0.0f32; BLUR_UNIFORM_FLOATS];
        // 保存源采样域四个像素中心 UV 边界。
        values[BLUR_UV_BOUNDS_FLOAT_OFFSET..BLUR_UV_BOUNDS_FLOAT_OFFSET + 4]
            .copy_from_slice(&uv_bounds);
        // 保存已经除以 source extent 的单 texel 步长。
        values[BLUR_TEXEL_STEP_TAPS_FLOAT_OFFSET] = texel_step[0];
        values[BLUR_TEXEL_STEP_TAPS_FLOAT_OFFSET + 1] = texel_step[1];
        // 保存 shader 从权重中心恢复采样偏移所需的半径。
        values[BLUR_TEXEL_STEP_TAPS_FLOAT_OFFSET + 2] = tap_radius as f32;
        // 把完整高斯权重复制到唯一固定区间。
        values[BLUR_WEIGHTS_FLOAT_OFFSET..BLUR_WEIGHTS_FLOAT_OFFSET + BLUR_WEIGHT_COUNT]
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
