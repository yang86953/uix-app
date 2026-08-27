//! Drawing System 在共享 GPU Raster/RHI 边界拥有的基础图元常量契约。

// 引入共享物理 viewport，避免基础图元契约依赖平台 API 类型。
use super::RhiViewport;

// 固定两个 float4 组成的 Mesh uniform 总字节数。
pub(crate) const MESH_UNIFORM_BYTES: usize = 32;
// 固定两个 float4 组成的 sampled/coverage uniform 总字节数。
pub(crate) const SAMPLED_UNIFORM_BYTES: usize = 32;
// 固定四个 float4 组成的 Sector uniform 总字节数。
pub(crate) const SECTOR_UNIFORM_BYTES: usize = 64;

// viewport 宽高在 Mesh ABI 中的起始 float 索引。
pub(crate) const MESH_VIEWPORT_FLOAT_OFFSET: usize = 0;
// straight-alpha 颜色在 Mesh ABI 中的起始 float 索引。
pub(crate) const MESH_COLOR_FLOAT_OFFSET: usize = 4;
// viewport 宽高在 sampled/coverage ABI 中的起始 float 索引。
pub(crate) const SAMPLED_VIEWPORT_FLOAT_OFFSET: usize = 0;
// 最终 surface 合成圆角半径在 sampled ABI 中的起始 float 索引。
pub(crate) const SAMPLED_CORNER_RADIUS_FLOAT_OFFSET: usize = 4;
// viewport 宽高在 Sector ABI 中的起始 float 索引。
pub(crate) const SECTOR_VIEWPORT_FLOAT_OFFSET: usize = 0;
// 扇形外接矩形在 Sector ABI 中的起始 float 索引。
pub(crate) const SECTOR_RECT_FLOAT_OFFSET: usize = 4;
// straight-alpha 颜色在 Sector ABI 中的起始 float 索引。
pub(crate) const SECTOR_COLOR_FLOAT_OFFSET: usize = 8;
// 起始角与扫过角在 Sector ABI 中的起始 float 索引。
pub(crate) const SECTOR_ANGLES_FLOAT_OFFSET: usize = 12;

// 把固定 float 数组编码为当前 host 的紧密字节载荷。
fn encode_ne_bytes<const N: usize>(values: &[f32; N]) -> Vec<u8> {
    // 为完整数组预留精确容量。
    let mut bytes = Vec::with_capacity(N * std::mem::size_of::<f32>());
    // 逐个保持 IEEE float 的原始 native-endian 表示。
    for value in values {
        // Adapter 与共享层运行在同一 host，因此直接追加 native-endian 字节。
        bytes.extend_from_slice(&value.to_ne_bytes());
    }
    // 返回可直接上传到 uniform buffer 的紧密载荷。
    bytes
}

// 保存平台无关的 Mesh viewport、padding 与 straight-alpha 颜色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RhiMeshRasterParams {
    // 两个 float4 依次保存 viewport/padding 与颜色。
    values: [f32; 8],
}

// 为共享 Mesh 常量契约提供唯一字段排列与编码入口。
impl RhiMeshRasterParams {
    // 从已经完成物理 lowering 的 viewport 与颜色构造固定 ABI。
    pub(crate) fn new(
        // 接收当前 render target 的物理 viewport。
        viewport: RhiViewport,
        // 接收当前 mesh 的 straight-alpha 颜色。
        rgba: [f32; 4],
    ) -> Self {
        // 返回包含确定性零 padding 的完整 Mesh ABI。
        Self {
            // 按 shader 寄存器顺序排列全部字段。
            values: [
                // 保存 viewport 宽度。
                viewport.width,
                // 保存 viewport 高度。
                viewport.height,
                // 固定第一个 padding。
                0.0,
                // 固定第二个 padding。
                0.0,
                // 保存红通道。
                rgba[0],
                // 保存绿通道。
                rgba[1],
                // 保存蓝通道。
                rgba[2],
                // 保存 alpha 通道。
                rgba[3],
            ],
        }
    }

    // 返回 Adapter 可按共享字段索引读取的 float ABI。
    pub(crate) const fn as_f32s(&self) -> &[f32; 8] {
        // 借出不可变数组，禁止下层重新组织字段。
        &self.values
    }

    // 把共享 Mesh ABI 编码为当前 host 的紧密字节载荷。
    pub(crate) fn encode_ne_bytes(&self) -> Vec<u8> {
        // 复用同文件的固定数组编码器。
        encode_ne_bytes(&self.values)
    }
}

// 保存平台无关的 sampled/coverage viewport 与确定性 padding。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RhiSampledRasterParams {
    // 两个 float4 保存 viewport、padding 与可选 surface 圆角半径。
    values: [f32; 8],
}

// 为 sampled 和 coverage 共用常量提供唯一字段排列与编码入口。
impl RhiSampledRasterParams {
    // 从已经完成物理 lowering 的 viewport 构造固定 ABI。
    pub(crate) fn new(
        // 接收当前 render target 的物理 viewport。
        viewport: RhiViewport,
    ) -> Self {
        // 返回包含确定性零 padding 的完整 sampled ABI。
        Self {
            // 普通 sampled、coverage 与 MSDF 不应用窗口边界遮罩。
            values: [
                viewport.width,
                viewport.height,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
            ],
        }
    }

    // 构造只用于 retained texture 最终合成的圆角 surface 常量。
    pub(crate) fn with_surface_corner_radius(viewport: RhiViewport, radius: f32) -> Self {
        let mut params = Self::new(viewport);
        // 半径已经由平台 appearance 契约规范化为物理像素。
        params.values[SAMPLED_CORNER_RADIUS_FLOAT_OFFSET] = radius.max(0.0);
        params
    }

    // 返回 Adapter 可按共享字段索引读取的 float ABI。
    pub(crate) const fn as_f32s(&self) -> &[f32; 8] {
        // 借出不可变数组，禁止下层重新组织字段。
        &self.values
    }

    // 把共享 sampled ABI 编码为当前 host 的紧密字节载荷。
    pub(crate) fn encode_ne_bytes(&self) -> Vec<u8> {
        // 复用同文件的固定数组编码器。
        encode_ne_bytes(&self.values)
    }
}

// 保存平台无关的 Sector viewport、矩形、颜色与角度。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RhiSectorRasterParams {
    // 四个 float4 依次保存 viewport、rect、color 和 angles。
    values: [f32; 16],
}

// 为共享 Sector 常量契约提供唯一字段排列与编码入口。
impl RhiSectorRasterParams {
    // 从已经完成物理 lowering 的扇形事实构造固定 ABI。
    pub(crate) fn new(
        // 接收当前 render target 的物理 viewport。
        viewport: RhiViewport,
        // 接收扇形的物理外接矩形左、上、宽、高。
        rect: [f32; 4],
        // 接收扇形的 straight-alpha 颜色。
        rgba: [f32; 4],
        // 接收从正 X 轴开始的起始角和正向扫过角。
        angles: [f32; 2],
    ) -> Self {
        // 从全零数组开始，确保 viewport 与角度 padding 保持确定值。
        let mut values = [0.0f32; 16];
        // 保存 viewport 宽度。
        values[SECTOR_VIEWPORT_FLOAT_OFFSET] = viewport.width;
        // 保存 viewport 高度。
        values[SECTOR_VIEWPORT_FLOAT_OFFSET + 1] = viewport.height;
        // 保存扇形外接矩形。
        values[SECTOR_RECT_FLOAT_OFFSET..SECTOR_RECT_FLOAT_OFFSET + 4].copy_from_slice(&rect);
        // 保存扇形 straight-alpha 颜色。
        values[SECTOR_COLOR_FLOAT_OFFSET..SECTOR_COLOR_FLOAT_OFFSET + 4].copy_from_slice(&rgba);
        // 保存起始角。
        values[SECTOR_ANGLES_FLOAT_OFFSET] = angles[0];
        // 保存正向扫过角。
        values[SECTOR_ANGLES_FLOAT_OFFSET + 1] = angles[1];
        // 返回已经冻结的共享 Sector 值对象。
        Self { values }
    }

    // 返回 Adapter 可按共享字段索引读取的 float ABI。
    pub(crate) const fn as_f32s(&self) -> &[f32; 16] {
        // 借出不可变数组，禁止下层重新组织字段。
        &self.values
    }

    // 把共享 Sector ABI 编码为当前 host 的紧密字节载荷。
    pub(crate) fn encode_ne_bytes(&self) -> Vec<u8> {
        // 复用同文件的固定数组编码器。
        encode_ne_bytes(&self.values)
    }
}
