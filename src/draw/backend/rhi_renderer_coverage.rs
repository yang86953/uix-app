//! 通用 GPU Renderer 的 R8 字形 coverage lowering。

// 引入共享字节载荷。
use std::sync::Arc;

// 引入 RHI 计划执行所需的资源描述与句柄。
use crate::platform::presentation::rhi::{
    BufferHandle, DrawBufferBindings, DrawPacket, DrawRange, DrawRasterState, DrawSamplingBinding,
    GraphicsDevice, LoadAction, PipelineBinding, PipelineDesc, PipelineKind, RhiExtent,
    RhiTextureRegion, RhiTextureUpload, SampledTextureBinding, SamplerDesc, SamplerHandle,
    TextureDesc, TextureFormat, TextureHandle,
};

// 复用 renderer 主模块的计划类型和 coverage payload。
use super::{
    FramePlanCommand, FrameUniformPayload, FrameVertexPayload, RhiCoverageQuad, RhiRenderer,
};

// R8 atlas 使用四张 1024² 页面，固定在 4 MiB 预算内。
const COVERAGE_ATLAS_PAGE_SIZE: u32 = 1024;
const COVERAGE_ATLAS_MAX_PAGES: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct CoverageCacheKey {
    hash: u64,
    pixel_w: u32,
    pixel_h: u32,
}

// 标识一份仍由 atlas 条目持有的不可变 coverage 载荷。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct CoverageIdentityKey {
    // 保存 Arc 切片的数据起始地址；对应条目持有 Arc 时地址不会被复用。
    data: usize,
    // 保存切片长度，避免零长或不同布局共享同一数据地址时混淆。
    len: usize,
    // 保存像素宽度，继续服从 coverage 资源契约。
    pixel_w: u32,
    // 保存像素高度，继续服从 coverage 资源契约。
    pixel_h: u32,
}

#[derive(Debug, Clone, Copy)]
struct CoverageAtlasPlacement {
    page: usize,
    uv: [f32; 4],
}

#[derive(Debug)]
pub(super) struct CoverageAtlasPage {
    pub(super) texture: TextureHandle,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
}

#[derive(Debug)]
pub(super) struct CoverageAtlasEntry {
    coverage: Arc<[u8]>,
    placement: CoverageAtlasPlacement,
}

// 为 coverage shader 创建或复用 R8 专用的 pipeline、buffer 和 sampler。
impl RhiRenderer {
    // 准备与普通 sampled quad 共用顶点/viewport ABI 的 coverage 资源。
    pub(super) fn ensure_coverage_resources(
        &mut self,
        device: &mut dyn GraphicsDevice,
        vertex_bytes: usize,
    ) -> Result<(PipelineBinding, BufferHandle, BufferHandle, SamplerHandle), crate::core::Error>
    {
        // 复用普通 sampled quad 的 float8 顶点 buffer 和 viewport uniform。
        let (_, vertex_buffer, uniform_buffer, _) =
            self.ensure_textured_resources_with_capacity(device, vertex_bytes)?;
        // 首次使用时创建 R8 coverage 专用 pipeline。
        let pipeline = if let Some(pipeline) = self.coverage_pipeline {
            // 复用已经登记的 coverage pipeline。
            pipeline
        } else {
            // 选择只由 RHI 契约定义的 coverage pipeline 语义。
            let pipeline = device.create_pipeline(PipelineDesc {
                kind: PipelineKind::GlyphCoverageQuad,
            })?;
            // 缓存 coverage pipeline 句柄。
            self.coverage_pipeline = Some(pipeline);
            pipeline
        };
        // 首次使用时创建点采样 sampler，保持旧 R8 atlas 的像素边界语义。
        let sampler = if let Some(sampler) = self.coverage_sampler {
            // 复用已登记的点采样 sampler。
            sampler
        } else {
            // coverage 采样不跨 glyph 像素做线性插值。
            let sampler = device.create_sampler(SamplerDesc::nearest_clamp())?;
            // 缓存 coverage sampler 句柄。
            self.coverage_sampler = Some(sampler);
            sampler
        };
        // 返回 coverage draw 所需的固定资源。
        Ok((pipeline, vertex_buffer, uniform_buffer, sampler))
    }

    // 把单通道 coverage 编码为紧密 R8 上传载荷。
    pub(super) fn encode_coverage(values: &[u8]) -> Arc<[u8]> {
        // 保持 coverage 原始字节值，不进行颜色或 alpha 转换。
        Arc::from(values.to_vec())
    }

    // 生成 position/uv/color float8 的两个三角形。
    pub(super) fn coverage_quad_vertices(quad: &RhiCoverageQuad) -> [f32; 48] {
        Self::coverage_quad_vertices_with_uv(quad, [0.0, 0.0, 1.0, 1.0])
    }

    // 生成带 atlas placement UV 的 position/uv/color float8 顶点。
    pub(super) fn coverage_quad_vertices_with_uv(
        quad: &RhiCoverageQuad,
        uv: [f32; 4],
    ) -> [f32; 48] {
        // 保存顶点颜色，交给旧 glyph shader 做量化和 premultiply。
        let color = quad.rgba;
        // 返回左上、右上、右下、左下组成的三角列表。
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

    // 获取或创建一个跨帧 R8 coverage atlas placement。
    pub(super) fn ensure_coverage_texture(
        &mut self,
        device: &mut dyn GraphicsDevice,
        quad: &RhiCoverageQuad,
    ) -> crate::core::Result<(TextureHandle, [f32; 4], bool)> {
        let identity = Self::coverage_identity_key(quad);
        // 上游字形缓存跨帧复用同一不可变 Arc 时，直接恢复既有内容键。
        if let Some(key) = self.coverage_atlas_identity_cache.get(&identity).copied() {
            if let Some(entry) = self.coverage_atlas_cache.get(&key) {
                // 身份索引只能命中仍由主条目持有的同一 Arc，避免地址复用误判。
                if Arc::ptr_eq(&entry.coverage, &quad.coverage) {
                    let page = self
                        .coverage_atlas_pages
                        .get(entry.placement.page)
                        .ok_or_else(|| {
                            super::rhi_invalid("RhiRenderer coverage atlas page is missing")
                        })?;
                    return Ok((page.texture, entry.placement.uv, true));
                }
            }
            // 主条目若已被替换，立即清除失效身份而不是保留悬空快路径。
            self.coverage_atlas_identity_cache.remove(&identity);
        }
        let key = Self::coverage_cache_key(quad);
        if let Some(entry) = self.coverage_atlas_cache.get(&key) {
            if entry.coverage.as_ref() == quad.coverage.as_ref() {
                // 只登记主条目实际持有的 Arc；临时同内容 Arc 不扩张身份索引。
                if Arc::ptr_eq(&entry.coverage, &quad.coverage) {
                    self.coverage_atlas_identity_cache.insert(identity, key);
                }
                let page = self
                    .coverage_atlas_pages
                    .get(entry.placement.page)
                    .ok_or_else(|| {
                        super::rhi_invalid("RhiRenderer coverage atlas page is missing")
                    })?;
                return Ok((page.texture, entry.placement.uv, true));
            }
        }
        if let Some(previous) = self.coverage_atlas_cache.remove(&key) {
            // 内容哈希碰撞替换前同步移除旧条目的唯一身份索引。
            let previous_identity = CoverageIdentityKey {
                data: previous.coverage.as_ptr() as usize,
                len: previous.coverage.len(),
                pixel_w: key.pixel_w,
                pixel_h: key.pixel_h,
            };
            self.coverage_atlas_identity_cache
                .remove(&previous_identity);
        }
        if quad.pixel_w > COVERAGE_ATLAS_PAGE_SIZE || quad.pixel_h > COVERAGE_ATLAS_PAGE_SIZE {
            return Self::create_transient_coverage_texture(device, quad);
        }
        let mut packed = None;
        let mut created_page = false;
        for (page_index, page) in self.coverage_atlas_pages.iter_mut().enumerate() {
            if let Some((x, y)) = Self::pack_coverage_slot(page, quad.pixel_w, quad.pixel_h) {
                packed = Some((page_index, x, y));
                break;
            }
        }
        if packed.is_none() && self.coverage_atlas_pages.len() < COVERAGE_ATLAS_MAX_PAGES {
            self.coverage_atlas_pages
                .push(Self::create_coverage_atlas_page(device)?);
            created_page = true;
            let page_index = self.coverage_atlas_pages.len() - 1;
            packed = Self::pack_coverage_slot(
                self.coverage_atlas_pages
                    .last_mut()
                    .ok_or_else(|| super::rhi_invalid("RhiRenderer coverage atlas page missing"))?,
                quad.pixel_w,
                quad.pixel_h,
            )
            .map(|(x, y)| (page_index, x, y));
        }
        let Some((page_index, slot_x, slot_y)) = packed else {
            return Self::create_transient_coverage_texture(device, quad);
        };
        let page_texture = self
            .coverage_atlas_pages
            .get(page_index)
            .ok_or_else(|| super::rhi_invalid("RhiRenderer coverage atlas page is missing"))?
            .texture;
        if let Err(error) = device.update_texture(RhiTextureUpload::new(
            page_texture,
            RhiTextureRegion::from_xy(slot_x, slot_y, RhiExtent::new(quad.pixel_w, quad.pixel_h)),
            quad.coverage.as_ref(),
        )) {
            if created_page {
                if let Some(page) = self.coverage_atlas_pages.pop() {
                    let _ = device.destroy_texture(page.texture);
                }
            }
            return Err(error);
        }
        let inverse = 1.0 / COVERAGE_ATLAS_PAGE_SIZE as f32;
        let placement = CoverageAtlasPlacement {
            page: page_index,
            uv: [
                slot_x as f32 * inverse,
                slot_y as f32 * inverse,
                (slot_x + quad.pixel_w) as f32 * inverse,
                (slot_y + quad.pixel_h) as f32 * inverse,
            ],
        };
        self.coverage_atlas_cache.insert(
            key,
            CoverageAtlasEntry {
                coverage: quad.coverage.clone(),
                placement,
            },
        );
        // 身份索引与主条目同生，不拥有第二份 coverage 或延长独立生命周期。
        self.coverage_atlas_identity_cache.insert(identity, key);
        Ok((page_texture, placement.uv, true))
    }

    // 从不可变 Arc 与尺寸契约构造常数时间的稳态身份键。
    fn coverage_identity_key(quad: &RhiCoverageQuad) -> CoverageIdentityKey {
        CoverageIdentityKey {
            data: quad.coverage.as_ptr() as usize,
            len: quad.coverage.len(),
            pixel_w: quad.pixel_w,
            pixel_h: quad.pixel_h,
        }
    }

    fn coverage_cache_key(quad: &RhiCoverageQuad) -> CoverageCacheKey {
        let mut hash = 14_695_981_039_346_656_037_u64;
        for byte in quad.coverage.iter() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(1_099_511_628_211_u64);
        }
        hash ^= u64::from(quad.pixel_w);
        hash = hash.wrapping_mul(1_099_511_628_211_u64);
        hash ^= u64::from(quad.pixel_h);
        CoverageCacheKey {
            hash,
            pixel_w: quad.pixel_w,
            pixel_h: quad.pixel_h,
        }
    }

    fn create_coverage_atlas_page(
        device: &mut dyn GraphicsDevice,
    ) -> crate::core::Result<CoverageAtlasPage> {
        let texture = device.create_texture(TextureDesc::new(
            RhiExtent::new(COVERAGE_ATLAS_PAGE_SIZE, COVERAGE_ATLAS_PAGE_SIZE),
            TextureFormat::R8Unorm,
        ))?;
        Ok(CoverageAtlasPage {
            texture,
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
        })
    }

    fn pack_coverage_slot(
        page: &mut CoverageAtlasPage,
        width: u32,
        height: u32,
    ) -> Option<(u32, u32)> {
        if page.cursor_x.saturating_add(width) > COVERAGE_ATLAS_PAGE_SIZE {
            page.cursor_x = 0;
            page.cursor_y = page.cursor_y.saturating_add(page.row_height);
            page.row_height = 0;
        }
        if page.cursor_y.saturating_add(height) > COVERAGE_ATLAS_PAGE_SIZE {
            return None;
        }
        let position = (page.cursor_x, page.cursor_y);
        page.cursor_x = page.cursor_x.saturating_add(width);
        page.row_height = page.row_height.max(height);
        Some(position)
    }

    fn create_transient_coverage_texture(
        device: &mut dyn GraphicsDevice,
        quad: &RhiCoverageQuad,
    ) -> crate::core::Result<(TextureHandle, [f32; 4], bool)> {
        let extent = RhiExtent::new(quad.pixel_w, quad.pixel_h);
        let texture = device.create_texture(TextureDesc::new(extent, TextureFormat::R8Unorm))?;
        if let Err(error) = device.update_texture(RhiTextureUpload::full(
            texture,
            extent,
            quad.coverage.as_ref(),
        )) {
            let _ = device.destroy_texture(texture);
            return Err(error);
        }
        Ok((texture, [0.0, 0.0, 1.0, 1.0], false))
    }

    pub(crate) fn release_coverage_atlas(
        &mut self,
        context: &mut dyn GraphicsDevice,
    ) -> crate::core::Result<()> {
        // 身份索引不得比持有 Arc 的主 atlas 条目活得更久。
        let _identities = std::mem::take(&mut self.coverage_atlas_identity_cache);
        let _entries = std::mem::take(&mut self.coverage_atlas_cache);
        let pages = std::mem::take(&mut self.coverage_atlas_pages);
        let mut first_error = None;
        for page in pages {
            if let Err(error) = context.destroy_texture(page.texture) {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    // 执行一帧 R8 字形 coverage quad RHI 计划。
    pub(crate) fn execute_coverage_quads(
        &mut self,
        mut frame: super::RhiRendererFrame<'_>,
        viewport: super::RhiViewport,
        load: LoadAction,
        quads: &[RhiCoverageQuad],
    ) -> Result<(), crate::core::Error> {
        // 空列表不应伪造一次 present。
        if quads.is_empty() {
            // 返回稳定的参数错误。
            return Err(super::rhi_invalid(
                "RhiRenderer cannot execute an empty coverage list",
            ));
        }
        // 拒绝非有限 viewport，避免计划构造和 adapter 结果分叉。
        if !viewport.is_valid() {
            // 返回稳定的参数错误。
            return Err(super::rhi_invalid(
                "RhiRenderer coverage viewport is invalid",
            ));
        }
        // 在创建任何 native texture 前验证所有 coverage quad 的静态载荷。
        for quad in quads {
            // 目标矩形和颜色必须是有限的正值。
            if !quad.x.is_finite()
                || !quad.y.is_finite()
                || !quad.w.is_finite()
                || !quad.h.is_finite()
                || quad.w <= 0.0
                || quad.h <= 0.0
                || quad
                    .corners
                    .iter()
                    .flatten()
                    .any(|coordinate| !coordinate.is_finite())
                || quad.rgba.iter().any(|channel| !channel.is_finite())
            {
                // 返回稳定的参数错误。
                return Err(super::rhi_invalid(
                    "RhiRenderer coverage quad geometry is invalid",
                ));
            }
            // 覆盖率纹理尺寸必须严格为正。
            if quad.pixel_w == 0 || quad.pixel_h == 0 {
                // 返回稳定的参数错误。
                return Err(super::rhi_invalid("RhiRenderer coverage extent is empty"));
            }
            // 防止 coverage 数量乘法溢出或 payload 与描述不一致。
            let pixel_count = (quad.pixel_w as usize)
                .checked_mul(quad.pixel_h as usize)
                .ok_or_else(|| super::rhi_invalid("RhiRenderer coverage extent overflows"))?;
            if quad.coverage.len() != pixel_count {
                // 只接受紧密排列的 R8 coverage，避免行步长被误解。
                return Err(super::rhi_invalid(
                    "RhiRenderer coverage payload length is invalid",
                ));
            }
            // 显式 scissor 必须已经完成物理坐标 lowering。
            if quad.scissor.is_some_and(|scissor| !scissor.is_valid()) {
                // 返回稳定的参数错误。
                return Err(super::rhi_invalid(
                    "RhiRenderer coverage scissor is invalid",
                ));
            }
        }
        // 准备 coverage pipeline、共享 vertex/uniform 和点采样 sampler。
        let vertex_bytes = quads
            .len()
            .checked_mul(6 * 8 * std::mem::size_of::<f32>())
            .ok_or_else(|| super::rhi_invalid("RhiRenderer coverage vertex capacity overflows"))?;
        let (pipeline, vertex_buffer, uniform_buffer, sampler) =
            self.ensure_coverage_resources(frame.device(), vertex_bytes)?;
        // 为本次帧逐项创建、上传并记录临时 R8 texture。
        let mut textures = Vec::with_capacity(quads.len());
        for quad in quads {
            // 创建只含 shader resource view 的 R8 coverage texture。
            let texture = match frame.device().create_texture(TextureDesc::new(
                // coverage 资源采用字形实际像素范围。
                RhiExtent::new(quad.pixel_w, quad.pixel_h),
                // coverage 使用单通道 R8 格式。
                TextureFormat::R8Unorm,
            )) {
                // 资源成功创建后进入统一清理列表。
                Ok(texture) => texture,
                // 创建失败时先释放已创建资源，再返回原始错误。
                Err(error) => {
                    let _ = Self::destroy_textures(frame.device(), &textures);
                    return Err(error);
                }
            };
            // 上传紧密的 R8 coverage 字节。
            let upload = Self::encode_coverage(quad.coverage.as_ref());
            // 资源上传失败时不能把半成品 texture 留在 adapter。
            if let Err(error) = frame.device().update_texture(RhiTextureUpload::full(
                // 更新刚由同一 Device 创建的 coverage 纹理。
                texture,
                // 上传范围使用 coverage 的物理像素尺寸。
                RhiExtent::new(quad.pixel_w, quad.pixel_h),
                // 保留单通道 coverage 载荷。
                &upload,
            )) {
                // 把当前失败资源加入清理列表。
                textures.push(texture);
                // 尝试释放所有已经创建的 coverage 资源。
                let _ = Self::destroy_textures(frame.device(), &textures);
                // 保留上传失败的真实错误。
                return Err(error);
            }
            // 记录上传完成且可以进入 FramePlan 的 coverage texture。
            textures.push(texture);
        }
        // 创建不携带 target/load 的 coverage pass 命令包。
        let mut pass = frame.new_pass();
        // 每个 glyph 以独立 texture binding 和 scissor 保留 painter order。
        for (quad, texture) in quads.iter().zip(textures.iter().copied()) {
            // 生成当前 glyph 的顶点数据。
            let vertices = Self::coverage_quad_vertices(quad);
            // 上传当前 glyph 的类型化 float8 顶点数据。
            pass.push(FramePlanCommand::UploadVertex {
                buffer: vertex_buffer,
                data: FrameVertexPayload::position_uv_color_f32(vertices),
            });
            // 上传当前 pass 的类型化物理 viewport uniform。
            pass.push(FramePlanCommand::UploadUniform {
                buffer: uniform_buffer,
                data: FrameUniformPayload::Sampled(super::RhiRenderer::sampled_uniform(viewport)),
            });
            // 追加六顶点的非索引 coverage quad draw packet。
            pass.push(FramePlanCommand::Draw(DrawPacket::new(
                pipeline,
                DrawBufferBindings::new(vertex_buffer, uniform_buffer),
                // 将当前 coverage 采样绑定封装进完整绘制包。
                DrawSamplingBinding::sampled(SampledTextureBinding::for_pipeline(
                    texture, sampler, pipeline,
                )),
                // Coverage quad 固化当前 viewport 与对应 scissor。
                DrawRasterState::new(viewport, quad.scissor),
                // Coverage quad 使用封闭的六顶点非索引范围。
                DrawRange::vertices(6),
            )));
        }
        // 将 pass 追加到封闭帧唯一拥有的计划中并保留 glyph painter order。
        frame.push_pass(load, pass);
        // 封闭帧决定最终 Surface present 或 Offscreen submit。
        let execution = frame.execute();
        // 计划结束后释放本次 glyph 的临时 texture。
        let cleanup = Self::destroy_textures(frame.device(), &textures);
        // 优先返回绘制或 present 失败；否则报告资源清理失败。
        match (execution, cleanup) {
            // 计划失败时保留原始执行错误。
            (Err(error), _) => Err(error),
            // 计划成功但清理失败时仍不能伪造完整成功。
            (Ok(_), Err(error)) => Err(error),
            // 计划与资源清理均成功。
            (Ok(_), Ok(())) => Ok(()),
        }
    }
}
