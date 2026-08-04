// 复用 D3D11 pipeline 的资源、输入布局、shader 与错误辅助。
use super::*;

// 持有兼容层当前可复用的 BGRA 图片纹理及其采样视图。
#[derive(Default)]
pub(super) struct D3d11ImageOwner {
    // 保存图片纹理本体，确保 SRV 生命周期覆盖 draw。
    texture: Option<ID3D11Texture2D>,
    // 保存图片纹理的 shader resource view。
    srv: Option<ID3D11ShaderResourceView>,
    // 记录当前纹理宽度，避免相同尺寸重复创建资源。
    width: u32,
    // 记录当前纹理高度，避免相同尺寸重复创建资源。
    height: u32,
}

// 为 D3D11 legacy compatibility queue 提供 BGRA affine image blit。
impl D3d11Pipeline {
    // 确保动态图片纹理与本次上传尺寸一致。
    fn ensure_image_texture(
        &mut self,
        device: &ID3D11Device,
        width: u32,
        height: u32,
    ) -> Result<()> {
        // 相同尺寸的纹理可以直接复用并只更新其内容。
        if self.image.texture.is_some()
            && self.image.srv.is_some()
            && self.image.width == width
            && self.image.height == height
        {
            return Ok(());
        }
        // 先释放旧 SRV 与纹理，避免尺寸变化后采样旧资源。
        self.image.srv = None;
        self.image.texture = None;
        // 为紧密 BGRA8 row-major payload 创建可采样纹理。
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width.max(1),
            Height: height.max(1),
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        // 接收新建纹理对象。
        let mut texture = None;
        // SAFETY: device 属于当前 owner thread，desc 与输出槽均为本次调用局部值。
        unsafe {
            device
                .CreateTexture2D(&desc, None, Some(&mut texture))
                .map_err(|error| d3d_error("CreateTexture2D(image)", error))?;
        }
        // 驱动必须返回有效的图片纹理。
        let texture = texture
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no image texture"))?;
        // 使用 BGRA 格式创建完整 mip 级别的 SRV。
        let srv_desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            ViewDimension: D3D11_SRV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_SRV {
                    MostDetailedMip: 0,
                    MipLevels: 1,
                },
            },
        };
        // 接收新建 SRV 对象。
        let mut srv = None;
        // SAFETY: texture 是刚创建的同一 device 资源，srv 输出槽有效。
        unsafe {
            device
                .CreateShaderResourceView(&texture, Some(&srv_desc), Some(&mut srv))
                .map_err(|error| d3d_error("CreateShaderResourceView(image)", error))?;
        }
        // 驱动必须返回有效的图片 SRV。
        let srv =
            srv.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no image SRV"))?;
        // 记录新资源及其尺寸，供后续 draw 复用。
        self.image.texture = Some(texture);
        // 保存 SRV 供采样阶段绑定。
        self.image.srv = Some(srv);
        // 保存纹理宽度。
        self.image.width = width;
        // 保存纹理高度。
        self.image.height = height;
        // 返回资源准备完成。
        Ok(())
    }

    // 将四角 affine quad 写入 D3D11 glyph-compatible 顶点 ABI。
    fn push_image_quad(vertices: &mut [GlyphVertex; 6], corners: [[f32; 2]; 4], opacity: f32) {
        // 左上三角形的第一个顶点。
        vertices[0] = GlyphVertex {
            pos: corners[0],
            uv: [0.0, 0.0],
            color: [opacity; 4],
        };
        // 右上三角形的第一个顶点。
        vertices[1] = GlyphVertex {
            pos: corners[1],
            uv: [1.0, 0.0],
            color: [opacity; 4],
        };
        // 左下三角形的第二个顶点。
        vertices[2] = GlyphVertex {
            pos: corners[2],
            uv: [1.0, 1.0],
            color: [opacity; 4],
        };
        // 第二个三角形复用左上角。
        vertices[3] = GlyphVertex {
            pos: corners[0],
            uv: [0.0, 0.0],
            color: [opacity; 4],
        };
        // 第二个三角形复用右下角。
        vertices[4] = GlyphVertex {
            pos: corners[2],
            uv: [1.0, 1.0],
            color: [opacity; 4],
        };
        // 第二个三角形的左下角。
        vertices[5] = GlyphVertex {
            pos: corners[3],
            uv: [0.0, 1.0],
            color: [opacity; 4],
        };
    }

    // 执行兼容层 BGRA 图片的 affine、opacity 与 SrcOver/Additive draw。
    pub(crate) fn draw_image_blits(
        &mut self,
        device: &ID3D11Device,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        blits: &[GpuImageBlit],
    ) -> Result<()> {
        // 空 batch 或无效 viewport 不产生 D3D11 状态变化。
        if blits.is_empty() || viewport_w <= 0.0 || viewport_h <= 0.0 {
            return Ok(());
        }
        // 图片 quad 复用 glyph 动态 vertex buffer 的单 quad 容量。
        self.ensure_glyph_vb(device, 1)?;
        // 将逻辑 scissor 转成 D3D11 的右下角坐标。
        let (sx, sy, sw, sh) =
            scissor.unwrap_or((0, 0, viewport_w.ceil() as i32, viewport_h.ceil() as i32));
        // 构造当前 batch 的 scissor rectangle。
        let scissor_rect = RECT {
            left: sx,
            top: sy,
            right: sx + sw.max(0),
            bottom: sy + sh.max(0),
        };
        // 写入共享 glyph vertex shader 所需的 viewport 常量。
        let constants = GlyphConstants {
            viewport: [viewport_w, viewport_h],
            _pad0: [0.0, 0.0],
        };
        // 映射 viewport constant buffer。
        let mut mapped_cb = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: context 与动态 buffer 均由当前 owner thread 持有。
        unsafe {
            context
                .Map(
                    &self.cb_glyph,
                    0,
                    D3D11_MAP_WRITE_DISCARD,
                    0,
                    Some(&mut mapped_cb),
                )
                .map_err(|error| d3d_error("Map(cb_glyph image)", error))?;
            // 拷贝与 GlyphCB ABI 完全一致的 16 字节常量。
            std::ptr::copy_nonoverlapping(
                (&constants as *const GlyphConstants).cast::<u8>(),
                mapped_cb.pData.cast(),
                size_of::<GlyphConstants>(),
            );
            // 结束 constant buffer 映射。
            context.Unmap(&self.cb_glyph, 0);
        }
        // 绑定 image 使用的 glyph-compatible vertex/shader 状态。
        let stride = size_of::<GlyphVertex>() as u32;
        // 顶点数据从 buffer 起始位置读取。
        let offset = 0u32;
        // SAFETY: 所有 D3D11 状态对象均来自同一 device，绑定参数生命周期覆盖调用。
        unsafe {
            // 绑定 position/uv/color 输入布局。
            context.IASetInputLayout(&self.layout_glyph);
            // 图片 quad 使用两个三角形。
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            // 绑定可复用的动态顶点 buffer。
            context.IASetVertexBuffers(
                0,
                1,
                Some(&Some(self.vb_glyph.clone())),
                Some(&stride),
                Some(&offset),
            );
            // 使用 viewport-aware glyph vertex shader。
            context.VSSetShader(&self.vs_glyph, None);
            // 使用通用 premultiplied sampled texture pixel shader。
            context.PSSetShader(&self.ps_rhi_textured, None);
            // 绑定 viewport 常量。
            context.VSSetConstantBuffers(0, Some(&[Some(self.cb_glyph.clone())]));
            // 绑定采样器与 rasterizer。
            context.PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));
            context.RSSetState(&self.rasterizer);
            // 应用本次兼容图片 batch 的 scissor。
            context.RSSetScissorRects(Some(&[scissor_rect]));
        }
        // 逐个上传图片内容并保持 painter order。
        for blit in blits {
            // 计算并校验紧密 BGRA payload 的像素数量。
            let expected = (blit.pixel_w as usize).checked_mul(blit.pixel_h as usize);
            // 丢弃无法表达的尺寸、几何、透明度和 payload，保持与 OpenGL owner 一致。
            if blit.pixel_w == 0
                || blit.pixel_h == 0
                || blit.pixel_w > i32::MAX as u32
                || blit.pixel_h > i32::MAX as u32
                || expected != Some(blit.pixels.len())
                || blit.w <= 0.0
                || blit.h <= 0.0
                || !blit.x.is_finite()
                || !blit.y.is_finite()
                || !blit.w.is_finite()
                || !blit.h.is_finite()
                || !blit.opacity.is_finite()
                || blit
                    .corners
                    .iter()
                    .flatten()
                    .any(|component| !component.is_finite())
            {
                continue;
            }
            // 透明图片无需上传和 draw。
            let opacity = blit.opacity.clamp(0.0, 1.0);
            // 透明度为零时保持严格 no-op。
            if opacity <= 0.0 {
                continue;
            }
            // 确保当前源尺寸的动态纹理已经创建。
            self.ensure_image_texture(device, blit.pixel_w, blit.pixel_h)?;
            // 获取当前图片纹理对象。
            let Some(texture) = self.image.texture.as_ref() else {
                return Err(Error::new(
                    Errc::PlatformError,
                    "D3d11Pipeline: image texture missing",
                ));
            };
            // 以完整纹理范围上传紧密 BGRA payload。
            let upload_box = D3D11_BOX {
                left: 0,
                top: 0,
                front: 0,
                right: blit.pixel_w,
                bottom: blit.pixel_h,
                back: 1,
            };
            // 计算每行字节数，溢出时报告明确的参数错误。
            let row_pitch = blit.pixel_w.checked_mul(4).ok_or_else(|| {
                Error::new(
                    Errc::InvalidArgument,
                    "D3d11Pipeline: image row pitch overflow",
                )
            })?;
            // SAFETY: texture 未作为当前 RT 绑定，payload 长度已按像素数校验。
            unsafe {
                context.UpdateSubresource(
                    texture,
                    0,
                    Some(&upload_box),
                    blit.pixels.as_ptr().cast(),
                    row_pitch,
                    0,
                );
            }
            // 构造当前图片的 affine 六顶点。
            let mut vertices = [GlyphVertex {
                pos: [0.0, 0.0],
                uv: [0.0, 0.0],
                color: [0.0; 4],
            }; 6];
            // 将 opacity 作为 sampled premultiplied color 的统一 tint。
            Self::push_image_quad(&mut vertices, blit.corners, opacity);
            // 映射动态图片顶点 buffer。
            let mut mapped_vb = D3D11_MAPPED_SUBRESOURCE::default();
            // SAFETY: vb_glyph 是动态 buffer，当前 draw 前没有未结束映射。
            unsafe {
                context
                    .Map(
                        &self.vb_glyph,
                        0,
                        D3D11_MAP_WRITE_DISCARD,
                        0,
                        Some(&mut mapped_vb),
                    )
                    .map_err(|error| d3d_error("Map(vb_glyph image)", error))?;
                // 拷贝六个 affine 图片顶点。
                std::ptr::copy_nonoverlapping(
                    vertices.as_ptr().cast::<u8>(),
                    mapped_vb.pData.cast(),
                    size_of::<[GlyphVertex; 6]>(),
                );
                // 结束顶点 buffer 映射。
                context.Unmap(&self.vb_glyph, 0);
            }
            // 获取当前纹理对应的 SRV。
            let Some(srv) = self.image.srv.as_ref() else {
                return Err(Error::new(
                    Errc::PlatformError,
                    "D3d11Pipeline: image SRV missing",
                ));
            };
            // 按图片的 blend 语义选择 premultiplied SrcOver 或 Additive。
            let blend = if blit.additive {
                &self.blend_additive
            } else {
                &self.blend_premultiplied
            };
            // 绑定当前 SRV、blend state 并提交六顶点 draw。
            unsafe {
                context.PSSetShaderResources(0, Some(&[Some(srv.clone())]));
                context.OMSetBlendState(blend, None, 0xffff_ffff);
                context.Draw(6, 0);
                // 解绑 SRV，允许下一轮尺寸变化释放旧纹理。
                context.PSSetShaderResources(0, Some(&[None]));
            }
        }
        // 返回图片 batch 编码成功。
        Ok(())
    }
}
