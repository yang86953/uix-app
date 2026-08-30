//! 字形 atlas 共享基础设施 — R8 coverage 与 RGBA8 MSDF 共用的内容键与 shelf 分配器。

// 引入薄 RHI 的资源描述与句柄。
use crate::core::Result;
use crate::platform::presentation::rhi::{
    GraphicsDevice, RhiExtent, TextureDesc, TextureFormat, TextureHandle,
};

// atlas 内容键的 FNV-1a 初始偏移。
pub(super) const FNV1A_OFFSET: u64 = 14_695_981_039_346_656_037;
// FNV-1a 混合素数。
const FNV1A_PRIME: u64 = 1_099_511_628_211;

// 推进一次 FNV-1a 混合，避免热路径在两套字形管线各写一份哈希循环。
pub(super) fn fnv1a_mix(hash: u64, word: u64) -> u64 {
    (hash ^ word).wrapping_mul(FNV1A_PRIME)
}

// 使用内容哈希和源尺寸识别一份可复用的字形纹理。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct GlyphAtlasKey {
    // 保存字形内容的稳定哈希。
    pub(super) hash: u64,
    // 保存源纹理宽度。
    pub(super) pixel_w: u32,
    // 保存源纹理高度。
    pub(super) pixel_h: u32,
}

impl GlyphAtlasKey {
    // 把源纹理尺寸并入内容哈希，避免不同栅格分辨率复用错误纹理。
    pub(super) fn finish(mut hash: u64, pixel_w: u32, pixel_h: u32) -> Self {
        hash = fnv1a_mix(hash, u64::from(pixel_w));
        hash = fnv1a_mix(hash, u64::from(pixel_h));
        Self {
            hash,
            pixel_w,
            pixel_h,
        }
    }
}

// 保存一个字形在 atlas 中的页号和归一化 UV。
#[derive(Debug, Clone, Copy)]
pub(super) struct GlyphAtlasPlacement {
    // 保存 atlas page 索引。
    pub(super) page: usize,
    // 保存字形内容的归一化 UV，不包含 gutter。
    pub(super) uv: [f32; 4],
}

// 保存一个可跨帧复用的字形 atlas page 及其 shelf 分配状态。
#[derive(Debug)]
pub(super) struct GlyphAtlasPage {
    // 保存该 page 的 GPU texture 句柄。
    pub(super) texture: TextureHandle,
    // 保存 shelf allocator 的当前横坐标。
    cursor_x: u32,
    // 保存 shelf allocator 的当前纵坐标。
    cursor_y: u32,
    // 保存当前 shelf 已占用的高度。
    row_height: u32,
}

impl GlyphAtlasPage {
    // 创建固定尺寸的字形 atlas page；atlas page 只承担 sampled texture，
    // 不把字形资源伪装成 render target。
    pub(super) fn create(
        device: &mut dyn GraphicsDevice,
        page_size: u32,
        format: TextureFormat,
    ) -> Result<Self> {
        let extent = RhiExtent::new(page_size, page_size);
        let texture = device.create_texture(TextureDesc::new(extent, format))?;
        // 返回从左上角开始的空 shelf。
        Ok(Self {
            texture,
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
        })
    }

    // 在当前 page 上执行 shelf 分配；当前 shelf 放不下时换行，page 垂直
    // 空间不足时返回 `None` 且不推进 cursor。
    pub(super) fn pack(&mut self, page_size: u32, width: u32, height: u32) -> Option<(u32, u32)> {
        if self.cursor_x.saturating_add(width) > page_size {
            self.cursor_x = 0;
            self.cursor_y = self.cursor_y.saturating_add(self.row_height);
            self.row_height = 0;
        }
        if self.cursor_y.saturating_add(height) > page_size {
            return None;
        }
        let position = (self.cursor_x, self.cursor_y);
        self.cursor_x = self.cursor_x.saturating_add(width);
        self.row_height = self.row_height.max(height);
        Some(position)
    }
}
