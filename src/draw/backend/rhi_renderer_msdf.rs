//! 通用 GPU Renderer 的 RGBA8 MSDF 字形 lowering。

// 引入共享字节载荷。
use std::sync::Arc;

// 引入最终执行计划所需的 viewport、资源和 context 类型。
use crate::platform::presentation::rhi::{
    BufferDesc, BufferHandle, GraphicsDevice, PipelineBinding, PipelineDesc, PipelineKind,
    RhiExtent, RhiMsdfRasterParams, RhiScissor, RhiTextureRegion, RhiTextureUpload, RhiViewport,
    SamplerDesc, SamplerHandle, TextureDesc, TextureFormat, TextureHandle,
};

// 固定单页尺寸，让 atlas 的资源预算和 adapter 上传粒度保持稳定。
const MSDF_ATLAS_PAGE_SIZE: u32 = 1024;
// 每个 atlas page 预留一像素 gutter，避免线性采样串入相邻字形。
const MSDF_ATLAS_GUTTER: u32 = 1;
// 限制跨帧 MSDF atlas 的 GPU 资源预算，避免字体场景变化导致无界增长。
const MSDF_ATLAS_LIMIT_BYTES: usize = 16 * 1024 * 1024;
// 以四张 1024² RGBA8 page 覆盖完整的 16 MiB 预算。
const MSDF_ATLAS_MAX_PAGES: usize =
    MSDF_ATLAS_LIMIT_BYTES / (MSDF_ATLAS_PAGE_SIZE as usize * MSDF_ATLAS_PAGE_SIZE as usize * 4);

// 使用边列表内容和源尺寸识别一个可复用的 MSDF texture。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct MsdfCacheKey {
    // 保存轮廓边列表的稳定哈希。
    hash: u64,
    // 保存源纹理宽度。
    pixel_w: u32,
    // 保存源纹理高度。
    pixel_h: u32,
}

// 保存一个字形在 atlas 中的页号和归一化 UV。
#[derive(Debug, Clone, Copy)]
pub(super) struct MsdfAtlasPlacement {
    // 保存 atlas page 索引。
    page: usize,
    // 保存字形内容的归一化 UV，不包含 gutter。
    uv: [f32; 4],
}

// 保存一个可跨帧复用的 RGBA8 atlas page。
#[derive(Debug)]
pub(super) struct MsdfAtlasPage {
    // 保存该 page 的 GPU texture 句柄。
    pub(super) texture: TextureHandle,
    // 保存 shelf allocator 的当前横坐标。
    cursor_x: u32,
    // 保存 shelf allocator 的当前纵坐标。
    cursor_y: u32,
    // 保存当前 shelf 已占用的高度。
    row_height: u32,
}

// 保存 atlas placement 及其边列表快照，用于处理哈希碰撞。
#[derive(Debug)]
pub(super) struct MsdfAtlasEntry {
    // 保存生成该 placement 的边列表内容。
    edges: Arc<[f32]>,
    // 保存字形在跨帧 atlas 中的物理位置。
    placement: MsdfAtlasPlacement,
}

// 保存一个已经完成 lowering 的 RGBA8 MSDF 字形 quad。
#[derive(Debug, Clone)]
pub(crate) struct RhiMsdfQuad {
    // 保存字形 AABB 左上角物理坐标，供几何校验使用。
    pub(crate) x: f32,
    // 保存字形 AABB 左上角物理坐标，供几何校验使用。
    pub(crate) y: f32,
    // 保存字形 AABB 物理宽度，供 MSDF 导数和校验使用。
    pub(crate) w: f32,
    // 保存字形 AABB 物理高度，供 MSDF 导数和校验使用。
    pub(crate) h: f32,
    // 保存设备空间四角，承载旋转和剪切后的字形几何。
    pub(crate) corners: [[f32; 2]; 4],
    // 保存与 CPU MSDF 编码一致的距离范围。
    pub(crate) range: f32,
    // 保存本地像素空间的 NonZero 轮廓边列表。
    pub(crate) edges: Arc<[f32]>,
    // 保存 MSDF 源纹理宽度。
    pub(crate) pixel_w: u32,
    // 保存 MSDF 源纹理高度。
    pub(crate) pixel_h: u32,
    // 保存字形颜色和 alpha。
    pub(crate) rgba: [f32; 4],
    // 保存当前字形的物理裁剪矩形。
    pub(crate) scissor: Option<RhiScissor>,
}

// 引入父 renderer 的执行器。
use super::RhiRenderer;

// 为 MSDF 字形准备固定 pipeline、共享顶点 buffer、常量 buffer 和 sampler。
impl RhiRenderer {
    // 准备 MSDF 字形的固定资源槽。
    pub(super) fn ensure_msdf_resources(
        &mut self,
        device: &mut dyn GraphicsDevice,
        vertex_bytes: usize,
    ) -> Result<(PipelineBinding, BufferHandle, BufferHandle, SamplerHandle), crate::core::Error>
    {
        // 复用普通 sampled quad 的 float8 顶点 ABI 和 buffer 容量管理。
        let (_, vertex_buffer, _, _) =
            self.ensure_textured_resources_with_capacity(device, vertex_bytes)?;
        // 首次使用时创建 MSDF 专用 pipeline。
        let pipeline = if let Some(pipeline) = self.msdf_pipeline {
            // 复用已经登记的 MSDF pipeline。
            pipeline
        } else {
            // 选择跨 Adapter 固定的 MSDF pipeline 语义。
            let pipeline = device.create_pipeline(PipelineDesc {
                kind: PipelineKind::MsdfGlyphQuad,
            })?;
            // 缓存 MSDF pipeline 句柄。
            self.msdf_pipeline = Some(pipeline);
            // 返回新创建的 MSDF pipeline。
            pipeline
        };
        // 首次使用时创建 32 字节 MSDF 常量 buffer。
        let uniform = if let Some(uniform) = self.msdf_uniform {
            // 复用已经登记的 MSDF uniform。
            uniform
        } else {
            // MSDFConstants = viewport、source extent、range 和 padding。
            let uniform = device.create_buffer(BufferDesc::uniform(
                // 常量容量只来自共享 MSDF Uniform ABI。
                PipelineKind::MsdfGlyphQuad.contract().uniform.size_bytes(),
            ))?;
            // 缓存 MSDF uniform 句柄。
            self.msdf_uniform = Some(uniform);
            // 返回新创建的 MSDF uniform。
            uniform
        };
        // 首次使用时创建线性 clamp sampler，保留 MSDF 的距离连续性。
        let sampler = if let Some(sampler) = self.msdf_sampler {
            // 复用已经登记的 MSDF sampler。
            sampler
        } else {
            // 线性采样使 MSDF 在缩放与仿射变换时保持距离场连续。
            let sampler = device.create_sampler(SamplerDesc::linear_clamp())?;
            // 缓存 MSDF sampler 句柄。
            self.msdf_sampler = Some(sampler);
            // 返回新创建的 MSDF sampler。
            sampler
        };
        // 返回 MSDF draw 所需的固定资源。
        Ok((pipeline, vertex_buffer, uniform, sampler))
    }

    // 把轮廓边列表编码为 RGBA8 MSDF 源纹理。
    pub(super) fn encode_msdf(quad: &RhiMsdfQuad) -> Option<Arc<[u8]>> {
        // 复用字体资源层定义的 CPU MSDF 公式，保证 soft 和 GPU 输入一致。
        crate::draw::resources::font::glyph_outline::msdf_from_edges(
            quad.edges.as_ref(),
            quad.pixel_w as usize,
            quad.pixel_h as usize,
        )
        .map(Arc::<[u8]>::from)
    }

    // 根据轮廓边列表生成跨帧 texture cache key。
    fn msdf_cache_key(quad: &RhiMsdfQuad) -> MsdfCacheKey {
        // 使用 FNV-1a 风格的轻量哈希，避免在热路径复制完整边列表。
        let mut hash = 14_695_981_039_346_656_037_u64;
        // 将每个有限 f32 的 bit pattern 纳入 key，保持 NaN 之外的几何精确区分。
        for edge in quad.edges.iter() {
            // 轮廓验证已在 mixed validation 阶段拒绝非有限边。
            hash ^= u64::from(edge.to_bits());
            // 采用 FNV 素数推进哈希状态。
            hash = hash.wrapping_mul(1_099_511_628_211_u64);
        }
        // 把源纹理尺寸也纳入 key，避免不同栅格分辨率复用错误纹理。
        hash ^= u64::from(quad.pixel_w);
        hash = hash.wrapping_mul(1_099_511_628_211_u64);
        hash ^= u64::from(quad.pixel_h);
        // 返回不包含 range 的 key，因为 range 只影响 shader 常量而不影响源纹理。
        MsdfCacheKey {
            hash,
            pixel_w: quad.pixel_w,
            pixel_h: quad.pixel_h,
        }
    }

    // 对比边列表内容，避免极小概率的哈希碰撞造成错误字形复用。
    fn msdf_edges_equal(left: &[f32], right: &[f32]) -> bool {
        // 长度不同的轮廓不可能对应同一份 MSDF 数据。
        left.len() == right.len()
            // 使用 bit pattern 比较，保持 -0 与距离生成输入的精确语义。
            && left
                .iter()
                .zip(right.iter())
                .all(|(left, right)| left.to_bits() == right.to_bits())
    }

    // 获取或创建一个跨帧复用的 MSDF atlas placement；无空间时返回本帧临时资源。
    pub(super) fn ensure_msdf_texture(
        &mut self,
        device: &mut dyn GraphicsDevice,
        quad: &RhiMsdfQuad,
    ) -> crate::core::Result<(TextureHandle, [f32; 4], RhiExtent, bool)> {
        // 先查找同一轮廓和尺寸的持久 atlas placement。
        let key = Self::msdf_cache_key(quad);
        if let Some(entry) = self.msdf_atlas_cache.get(&key) {
            // 哈希相同还要检查完整边列表，避免错误命中。
            if Self::msdf_edges_equal(entry.edges.as_ref(), quad.edges.as_ref()) {
                // 检查 placement 页仍由 renderer 持有，避免 stale handle 进入计划。
                let page = self
                    .msdf_atlas_pages
                    .get(entry.placement.page)
                    .ok_or_else(|| super::rhi_invalid("RhiRenderer MSDF atlas page is missing"))?;
                // true 表示调用方不能在本帧结束时销毁该 atlas page。
                return Ok((
                    page.texture,
                    entry.placement.uv,
                    RhiExtent::new(MSDF_ATLAS_PAGE_SIZE, MSDF_ATLAS_PAGE_SIZE),
                    true,
                ));
            }
        }
        // 哈希碰撞时移除旧 placement；旧槽位不会重新暴露给错误的轮廓。
        self.msdf_atlas_cache.remove(&key);
        // 只有能完成合法 MSDF 编码时才进入 atlas 上传阶段。
        let payload = Self::encode_msdf(quad)
            .ok_or_else(|| super::rhi_invalid("RhiRenderer mixed MSDF generation failed"))?;
        // 字形尺寸带 gutter 后超过 page 时走本帧临时 texture。
        let alloc_width = quad
            .pixel_w
            .checked_add(MSDF_ATLAS_GUTTER * 2)
            .ok_or_else(|| super::rhi_invalid("RhiRenderer MSDF atlas width overflows"))?;
        let alloc_height = quad
            .pixel_h
            .checked_add(MSDF_ATLAS_GUTTER * 2)
            .ok_or_else(|| super::rhi_invalid("RhiRenderer MSDF atlas height overflows"))?;
        if alloc_width > MSDF_ATLAS_PAGE_SIZE || alloc_height > MSDF_ATLAS_PAGE_SIZE {
            // 单个超大字形不能破坏固定 atlas 的预算，保留本帧临时资源。
            return Self::create_transient_msdf_texture(device, quad, payload.as_ref());
        }
        // 先在已有 page 中寻找可容纳当前字形的 shelf。
        let mut packed = None;
        // 记录本次是否追加了新 page，便于上传失败时只回收半成品。
        let mut created_page = false;
        for (page_index, page) in self.msdf_atlas_pages.iter_mut().enumerate() {
            if let Some((x, y)) = Self::pack_atlas_slot(page, alloc_width, alloc_height) {
                packed = Some((page_index, x, y));
                break;
            }
        }
        // 无空槽时按预算创建下一个 page。
        if packed.is_none() && self.msdf_atlas_pages.len() < MSDF_ATLAS_MAX_PAGES {
            let page = Self::create_msdf_atlas_page(device)?;
            self.msdf_atlas_pages.push(page);
            created_page = true;
            let page_index = self.msdf_atlas_pages.len() - 1;
            packed = Self::pack_atlas_slot(
                self.msdf_atlas_pages
                    .last_mut()
                    .ok_or_else(|| super::rhi_invalid("RhiRenderer MSDF atlas page missing"))?,
                alloc_width,
                alloc_height,
            )
            .map(|(x, y)| (page_index, x, y));
        }
        // 四页均没有空间时仍保证当前绘制可完成，但不继续扩大常驻预算。
        let Some((page_index, slot_x, slot_y)) = packed else {
            // atlas 满载时返回本帧临时 texture，不让旧 placement 被重排破坏。
            return Self::create_transient_msdf_texture(device, quad, payload.as_ref());
        };
        // 生成带四周 gutter 的 RGBA8 子区域，阻断相邻字形的线性采样污染。
        let padded = Self::pad_msdf_payload(payload.as_ref(), quad.pixel_w, quad.pixel_h)?;
        let upload_extent = RhiExtent::new(alloc_width, alloc_height);
        let page_texture = self
            .msdf_atlas_pages
            .get(page_index)
            .ok_or_else(|| super::rhi_invalid("RhiRenderer MSDF atlas page is missing"))?
            .texture;
        if let Err(error) = device.update_texture(RhiTextureUpload::new(
            // 更新当前 Device 拥有的 atlas 页。
            page_texture,
            // 把 atlas 槽位原点与补边范围封闭为一个区域。
            RhiTextureRegion::from_xy(slot_x, slot_y, upload_extent),
            // 上传已经补边的 MSDF 像素。
            &padded,
        )) {
            // 新 page 上传失败时立即回收其纹理；已有 page 的槽位则留作失败诊断。
            if created_page {
                let page = self.msdf_atlas_pages.pop();
                if let Some(page) = page {
                    let _ = device.destroy_texture(page.texture);
                }
            }
            return Err(error);
        }
        // 只把不含 gutter 的内容区域暴露给 shader。
        let inverse = 1.0 / MSDF_ATLAS_PAGE_SIZE as f32;
        let placement = MsdfAtlasPlacement {
            page: page_index,
            uv: [
                (slot_x + MSDF_ATLAS_GUTTER) as f32 * inverse,
                (slot_y + MSDF_ATLAS_GUTTER) as f32 * inverse,
                (slot_x + MSDF_ATLAS_GUTTER + quad.pixel_w) as f32 * inverse,
                (slot_y + MSDF_ATLAS_GUTTER + quad.pixel_h) as f32 * inverse,
            ],
        };
        // 记录边列表和 placement，后续帧只复用 atlas page 与 UV。
        self.msdf_atlas_cache.insert(
            key,
            MsdfAtlasEntry {
                edges: quad.edges.clone(),
                placement,
            },
        );
        // true 表示 atlas page 已由 renderer cache 接管生命周期。
        Ok((
            page_texture,
            placement.uv,
            RhiExtent::new(MSDF_ATLAS_PAGE_SIZE, MSDF_ATLAS_PAGE_SIZE),
            true,
        ))
    }

    // 创建固定尺寸的 RGBA8 MSDF atlas page。
    fn create_msdf_atlas_page(
        device: &mut dyn GraphicsDevice,
    ) -> crate::core::Result<MsdfAtlasPage> {
        // atlas page 只承担 sampled texture，不把字形资源伪装成 render target。
        let extent = RhiExtent::new(MSDF_ATLAS_PAGE_SIZE, MSDF_ATLAS_PAGE_SIZE);
        let texture = device.create_texture(TextureDesc::new(
            // atlas page 使用固定物理尺寸。
            extent,
            // atlas page 保存 RGBA8 距离场。
            TextureFormat::Rgba8Unorm,
        ))?;
        // 返回从左上角开始的空 shelf。
        Ok(MsdfAtlasPage {
            texture,
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
        })
    }

    // 在一个 page 上执行带右下 gutter 的 shelf 分配。
    fn pack_atlas_slot(page: &mut MsdfAtlasPage, width: u32, height: u32) -> Option<(u32, u32)> {
        // 当前 shelf 放不下时换到下一行。
        if page.cursor_x.saturating_add(width) > MSDF_ATLAS_PAGE_SIZE {
            page.cursor_x = 0;
            page.cursor_y = page.cursor_y.saturating_add(page.row_height);
            page.row_height = 0;
        }
        // page 没有足够的垂直空间时不推进 cursor。
        if page.cursor_y.saturating_add(height) > MSDF_ATLAS_PAGE_SIZE {
            return None;
        }
        // 保留当前 slot，并让下一项与其保持完整 gutter 间距。
        let position = (page.cursor_x, page.cursor_y);
        page.cursor_x = page.cursor_x.saturating_add(width);
        page.row_height = page.row_height.max(height);
        Some(position)
    }

    // 为字形内容复制四周 gutter，避免 atlas page 内相邻 UV 串色。
    fn pad_msdf_payload(payload: &[u8], width: u32, height: u32) -> crate::core::Result<Vec<u8>> {
        // 计算源和目标 row bytes，并拒绝算术溢出。
        let source_row = (width as usize)
            .checked_mul(4)
            .ok_or_else(|| super::rhi_invalid("RhiRenderer MSDF source row overflows"))?;
        let target_width = width
            .checked_add(MSDF_ATLAS_GUTTER * 2)
            .ok_or_else(|| super::rhi_invalid("RhiRenderer MSDF padded width overflows"))?;
        let target_height = height
            .checked_add(MSDF_ATLAS_GUTTER * 2)
            .ok_or_else(|| super::rhi_invalid("RhiRenderer MSDF padded height overflows"))?;
        let target_row = (target_width as usize)
            .checked_mul(4)
            .ok_or_else(|| super::rhi_invalid("RhiRenderer MSDF target row overflows"))?;
        let expected = source_row
            .checked_mul(height as usize)
            .ok_or_else(|| super::rhi_invalid("RhiRenderer MSDF source payload overflows"))?;
        if payload.len() != expected {
            return Err(super::rhi_invalid(
                "RhiRenderer MSDF source payload is invalid",
            ));
        }
        let total = target_row
            .checked_mul(target_height as usize)
            .ok_or_else(|| super::rhi_invalid("RhiRenderer MSDF padded payload overflows"))?;
        let mut padded = vec![0; total];
        // 复制源像素，同时复制每行左右边界像素。
        for row in 0..height as usize {
            let source = &payload[row * source_row..(row + 1) * source_row];
            let target = (row + MSDF_ATLAS_GUTTER as usize) * target_row;
            let inner = target + MSDF_ATLAS_GUTTER as usize * 4;
            padded[inner..inner + source_row].copy_from_slice(source);
            padded[target..target + 4].copy_from_slice(&source[..4]);
            padded[inner + source_row..inner + source_row + 4]
                .copy_from_slice(&source[source_row - 4..source_row]);
        }
        // 复制首尾两行，保证垂直方向的线性采样也只看到边界像素。
        let first = MSDF_ATLAS_GUTTER as usize * target_row;
        let last = (MSDF_ATLAS_GUTTER as usize + height as usize - 1) * target_row;
        let first_row = padded[first..first + target_row].to_vec();
        padded[..target_row].copy_from_slice(&first_row);
        let bottom = (MSDF_ATLAS_GUTTER as usize + height as usize) * target_row;
        let last_row = padded[last..last + target_row].to_vec();
        padded[bottom..bottom + target_row].copy_from_slice(&last_row);
        Ok(padded)
    }

    // 为无法进入固定 atlas 的单个字形创建本帧临时 texture。
    fn create_transient_msdf_texture(
        device: &mut dyn GraphicsDevice,
        quad: &RhiMsdfQuad,
        payload: &[u8],
    ) -> crate::core::Result<(TextureHandle, [f32; 4], RhiExtent, bool)> {
        // 临时资源使用原始字形 extent 和完整 UV。
        let extent = RhiExtent::new(quad.pixel_w, quad.pixel_h);
        let texture = device.create_texture(TextureDesc::new(
            // 临时纹理采用字形实际物理尺寸。
            extent,
            // 临时纹理保存 RGBA8 距离场。
            TextureFormat::Rgba8Unorm,
        ))?;
        // 把临时资源、完整字形范围与距离场载荷封闭成上传命令。
        let upload = RhiTextureUpload::full(texture, extent, payload);
        // 上传失败时立即回收半成品 texture。
        if let Err(error) = device.update_texture(upload) {
            let _ = device.destroy_texture(texture);
            return Err(error);
        }
        // false 表示 mixed executor 要在本帧结束时销毁该 texture。
        Ok((texture, [0.0, 0.0, 1.0, 1.0], extent, false))
    }

    // 在 graphics context 关闭前释放跨帧 MSDF atlas pages。
    pub(crate) fn release_msdf_atlas(
        &mut self,
        context: &mut dyn GraphicsDevice,
    ) -> crate::core::Result<()> {
        // 先移出 entry，确保即使某个 destroy 失败也不会重复使用旧句柄。
        let _entries = std::mem::take(&mut self.msdf_atlas_cache);
        // 移出所有 atlas page，确保销毁失败时不会重复使用旧句柄。
        let pages = std::mem::take(&mut self.msdf_atlas_pages);
        // 逐项销毁 GPU texture，并保留首个 adapter 错误。
        let mut first_error = None;
        for page in pages {
            // 继续释放后续资源，避免一个失败导致 atlas 整体泄漏。
            if let Err(error) = context.destroy_texture(page.texture) {
                // 只保留首个错误，方便定位根因。
                first_error.get_or_insert(error);
            }
        }
        // 返回 cache 释放结果。
        first_error.map_or(Ok(()), Err)
    }

    // 生成携带 atlas placement UV 的 position/uv/color float8 顶点。
    pub(super) fn msdf_quad_vertices_with_uv(quad: &RhiMsdfQuad, uv: [f32; 4]) -> [f32; 48] {
        // 保存顶点颜色，交给 MSDF shader 做 coverage 与 premultiply。
        let color = quad.rgba;
        // 返回左上、右上、右下、左上、右下、左下两个三角形。
        [
            quad.corners[0][0],
            quad.corners[0][1],
            uv[0],
            uv[1],
            color[0],
            color[1],
            color[2],
            color[3],
            quad.corners[1][0],
            quad.corners[1][1],
            uv[2],
            uv[1],
            color[0],
            color[1],
            color[2],
            color[3],
            quad.corners[2][0],
            quad.corners[2][1],
            uv[2],
            uv[3],
            color[0],
            color[1],
            color[2],
            color[3],
            quad.corners[0][0],
            quad.corners[0][1],
            uv[0],
            uv[1],
            color[0],
            color[1],
            color[2],
            color[3],
            quad.corners[2][0],
            quad.corners[2][1],
            uv[2],
            uv[3],
            color[0],
            color[1],
            color[2],
            color[3],
            quad.corners[3][0],
            quad.corners[3][1],
            uv[0],
            uv[3],
            color[0],
            color[1],
            color[2],
            color[3],
        ]
    }

    // 把 MSDF shader 事实映射为共享 RHI 常量值对象。
    pub(super) fn msdf_uniform(
        viewport: RhiViewport,
        quad: &RhiMsdfQuad,
        texture_extent: RhiExtent,
    ) -> RhiMsdfRasterParams {
        // 由共享值对象唯一排列 viewport、atlas extent、range 和 padding。
        RhiMsdfRasterParams::new(viewport, texture_extent, quad.range)
    }
}

// 为 MSDF atlas 的固定预算、gutter 和 shelf 代际提供低层单元测试。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../tests/unit/draw/backend/rhi_renderer_msdf__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
