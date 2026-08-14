//! D3D11 薄 RHI 的固定 draw ABI 分派。

// 引入统一错误和结果类型。
use crate::core::error::{Errc, Error, Result};
// 引入 draw packet 与有限资源语义。
use crate::native::present::rhi::{BufferUsage, DrawPacket, TextureFormat};

// 引入父模块的 D3D11 context、资源状态和错误辅助。
use super::{D3d11Context, rhi_invalid, rhi_not_implemented};

// 为 D3D11 context 编码通用 draw packet。
impl D3d11Context {
    // 执行一个已经选择固定 pipeline 与资源句柄的通用 draw packet。
    pub(super) fn draw_rhi_packet(&mut self, packet: DrawPacket) -> Result<()> {
        // draw 必须发生在显式 render pass 内。
        if !self.rhi_device.pass_open {
            // 返回稳定的状态错误。
            return Err(Error::new(
                Errc::InvalidState,
                "D3d11 RHI draw without active render pass",
            ));
        }
        // 先解析 pipeline，随后把 COM 句柄复制出来以结束资源表借用。
        let pipeline_key = self.rhi_device.pipeline(packet.pipeline)?.key;
        // 只允许已经登记的有限 pipeline key，禁止静默落到旧 UI draw_* 路径。
        if pipeline_key != crate::native::present::rhi::pipeline_keys::SOLID_MESH
            && pipeline_key != crate::native::present::rhi::pipeline_keys::TEXTURED_QUAD
            && pipeline_key != crate::native::present::rhi::pipeline_keys::TEXTURED_QUAD_ADDITIVE
            && pipeline_key != crate::native::present::rhi::pipeline_keys::GRADIENT_RECT
            && pipeline_key != crate::native::present::rhi::pipeline_keys::GLYPH_COVERAGE_QUAD
            && pipeline_key != crate::native::present::rhi::pipeline_keys::SHAPE_RECT
            && pipeline_key != crate::native::present::rhi::pipeline_keys::SHAPE_RECT_ADDITIVE
            && pipeline_key != crate::native::present::rhi::pipeline_keys::BOX_SHADOW
            && pipeline_key != crate::native::present::rhi::pipeline_keys::BLUR_PASS
            && pipeline_key != crate::native::present::rhi::pipeline_keys::MSDF_GLYPH_QUAD
            && pipeline_key != crate::native::present::rhi::pipeline_keys::SECTOR
        {
            // 返回稳定的未实现错误。
            return Err(rhi_not_implemented("D3d11 RHI pipeline draw key"));
        }
        // 解析顶点 buffer 并保留通用 ABI 所需的步长。
        let vertex = self.rhi_device.buffer(packet.vertex_buffer)?;
        // 拒绝错误用途的顶点资源。
        if vertex.usage != BufferUsage::Vertex {
            // 返回稳定的资源类型错误。
            return Err(rhi_invalid("D3d11 RHI draw vertex buffer has wrong usage"));
        }
        // 复制顶点 COM 对象，结束资源表借用。
        let vertex_native = vertex.native.clone();
        // 复制通用顶点步长。
        let vertex_stride = vertex.stride_bytes;
        // 解析 packet 的 uniform buffer。
        let uniform_handle = packet
            .uniform_buffer
            .ok_or_else(|| rhi_invalid("D3d11 RHI draw uniform is missing"))?;
        // 读取 uniform 资源事实。
        let uniform = self.rhi_device.buffer(uniform_handle)?;
        // 拒绝错误用途的 uniform 资源。
        if uniform.usage != BufferUsage::Uniform {
            // 返回稳定的资源类型错误。
            return Err(rhi_invalid("D3d11 RHI draw uniform buffer has wrong usage"));
        }
        // 复制 uniform COM 对象，结束资源表借用。
        let uniform_native = uniform.native.clone();
        // 索引 buffer 如存在必须使用固定 uint32 ABI。
        let index_native = if let Some(index_handle) = packet.index_buffer {
            // 解析索引资源。
            let index = self.rhi_device.buffer(index_handle)?;
            // 校验索引资源用途和固定步长。
            if index.usage != BufferUsage::Index || index.stride_bytes != 4 {
                // 返回稳定的资源类型错误。
                return Err(rhi_invalid("D3d11 RHI draw index buffer ABI is not uint32"));
            }
            // 复制索引 COM 对象，结束资源表借用。
            Some(index.native.clone())
        } else {
            // 非索引 packet 不绑定 index buffer。
            None
        };
        // 按通用 pipeline key 选择已经验证过的 D3D11 shader ABI。
        match pipeline_key {
            // 实心 mesh 使用 32 字节 MeshConstants（viewport、padding、颜色）。
            key if key == crate::native::present::rhi::pipeline_keys::SOLID_MESH => {
                // 拒绝不匹配的 solid uniform 布局。
                if uniform.size_bytes != 32 {
                    // 返回稳定的参数错误。
                    return Err(rhi_invalid("D3d11 RHI solid mesh uniform size is invalid"));
                }
                // 把已解析的低层对象交给 D3D11 pipeline 编码 draw。
                self.pipeline.draw_rhi_solid_mesh(
                    &self.context,
                    &vertex_native,
                    vertex_stride,
                    index_native.as_ref(),
                    &uniform_native,
                    packet.vertex_count,
                    packet.index_count,
                    packet.first_vertex,
                    packet.first_index,
                    packet.base_vertex,
                )
            }
            // 采样 quad 使用 16 字节 viewport uniform 和 t0/s0 绑定。
            key if key == crate::native::present::rhi::pipeline_keys::TEXTURED_QUAD
                || key == crate::native::present::rhi::pipeline_keys::TEXTURED_QUAD_ADDITIVE =>
            {
                // 拒绝不匹配的 textured uniform 布局。
                if uniform.size_bytes != 16 {
                    // 返回稳定的参数错误。
                    return Err(rhi_invalid("D3d11 RHI textured uniform size is invalid"));
                }
                // 读取 draw 前由 BindTexture 写入的纹理身份。
                let texture_handle = self
                    .rhi_device
                    .bound_texture
                    .ok_or_else(|| rhi_invalid("D3d11 RHI textured draw has no texture"))?;
                // 读取 draw 前由 BindTexture 写入的 sampler 身份。
                let sampler_handle = self
                    .rhi_device
                    .bound_sampler
                    .ok_or_else(|| rhi_invalid("D3d11 RHI textured draw has no sampler"))?;
                // 解析 SRV 并复制 COM 句柄，结束资源表借用。
                let texture = self.rhi_device.texture(texture_handle)?;
                // 复制 sampled texture view。
                let texture_srv = texture.srv.clone();
                // 解析 sampler state 并复制 COM 句柄。
                let sampler = self.rhi_device.sampler(sampler_handle)?;
                // 复制 sampler state。
                let sampler_native = sampler.native.clone();
                // 只有 Additive key 才切换到加法 blend。
                let additive =
                    key == crate::native::present::rhi::pipeline_keys::TEXTURED_QUAD_ADDITIVE;
                // 把已解析的低层对象交给 D3D11 pipeline 编码 draw。
                self.pipeline.draw_rhi_textured_quad(
                    &self.context,
                    &vertex_native,
                    vertex_stride,
                    index_native.as_ref(),
                    &uniform_native,
                    &texture_srv,
                    &sampler_native,
                    additive,
                    packet.vertex_count,
                    packet.index_count,
                    packet.first_vertex,
                    packet.first_index,
                    packet.base_vertex,
                )
            }
            // R8 字形覆盖率 quad 复用 16 字节 viewport uniform 和 t0/s0 绑定。
            key if key == crate::native::present::rhi::pipeline_keys::GLYPH_COVERAGE_QUAD => {
                // 拒绝不匹配的 glyph uniform 布局。
                if uniform.size_bytes != 16 {
                    // 返回稳定的参数错误。
                    return Err(rhi_invalid("D3d11 RHI glyph uniform size is invalid"));
                }
                // 读取 draw 前由 BindTexture 写入的纹理身份。
                let texture_handle = self
                    .rhi_device
                    .bound_texture
                    .ok_or_else(|| rhi_invalid("D3d11 RHI glyph draw has no texture"))?;
                // 读取 draw 前由 BindTexture 写入的 sampler 身份。
                let sampler_handle = self
                    .rhi_device
                    .bound_sampler
                    .ok_or_else(|| rhi_invalid("D3d11 RHI glyph draw has no sampler"))?;
                // 解析纹理格式，防止 R8 pipeline 误采样颜色纹理。
                let texture = self.rhi_device.texture(texture_handle)?;
                // 强制 coverage ABI 使用单通道纹理。
                if texture.format != TextureFormat::R8Unorm {
                    // 返回稳定的资源类型错误。
                    return Err(rhi_invalid("D3d11 RHI glyph texture must be R8Unorm"));
                }
                // 复制 R8 SRV，结束资源表借用。
                let texture_srv = texture.srv.clone();
                // 解析 sampler state 并复制 COM 句柄。
                let sampler = self.rhi_device.sampler(sampler_handle)?;
                // 复制 sampler state。
                let sampler_native = sampler.native.clone();
                // 把已解析的低层对象交给 glyph coverage shader 编码 draw。
                self.pipeline.draw_rhi_coverage_quad(
                    &self.context,
                    &vertex_native,
                    vertex_stride,
                    index_native.as_ref(),
                    &uniform_native,
                    &texture_srv,
                    &sampler_native,
                    packet.vertex_count,
                    packet.index_count,
                    packet.first_vertex,
                    packet.first_index,
                    packet.base_vertex,
                )
            }
            // RGBA8 MSDF 字形 quad 使用 32 字节 viewport/extent/range uniform。
            key if key == crate::native::present::rhi::pipeline_keys::MSDF_GLYPH_QUAD => {
                // 拒绝不匹配的 MSDF uniform 布局。
                if uniform.size_bytes != 32 {
                    // 返回稳定的参数错误。
                    return Err(rhi_invalid("D3d11 RHI MSDF uniform size is invalid"));
                }
                // 读取 draw 前由 BindTexture 写入的纹理身份。
                let texture_handle = self
                    .rhi_device
                    .bound_texture
                    .ok_or_else(|| rhi_invalid("D3d11 RHI MSDF draw has no texture"))?;
                // 读取 draw 前由 BindTexture 写入的 sampler 身份。
                let sampler_handle = self
                    .rhi_device
                    .bound_sampler
                    .ok_or_else(|| rhi_invalid("D3d11 RHI MSDF draw has no sampler"))?;
                // MSDF shader 只接受 RGBA8 距离纹理。
                let texture = self.rhi_device.texture(texture_handle)?;
                if texture.format != TextureFormat::Rgba8Unorm {
                    // 返回稳定的资源类型错误。
                    return Err(rhi_invalid("D3d11 RHI MSDF texture must be Rgba8Unorm"));
                }
                // 复制 RGBA8 SRV，结束资源表借用。
                let texture_srv = texture.srv.clone();
                // 解析 sampler state 并复制 COM 句柄。
                let sampler = self.rhi_device.sampler(sampler_handle)?;
                // 复制 sampler state。
                let sampler_native = sampler.native.clone();
                // 把已解析的低层对象交给 D3D11 MSDF shader 编码 draw。
                self.pipeline.draw_rhi_msdf_quad(
                    &self.context,
                    &vertex_native,
                    vertex_stride,
                    index_native.as_ref(),
                    &uniform_native,
                    &texture_srv,
                    &sampler_native,
                    packet.vertex_count,
                    packet.index_count,
                    packet.first_vertex,
                    packet.first_index,
                    packet.base_vertex,
                )
            }
            // 圆角/描边矩形使用前 80 字节 RectConstants，共用 96 字节资源。
            key if key == crate::native::present::rhi::pipeline_keys::SHAPE_RECT
                || key == crate::native::present::rhi::pipeline_keys::SHAPE_RECT_ADDITIVE =>
            {
                // 拒绝不匹配的 shape uniform 布局。
                if uniform.size_bytes != 80 && uniform.size_bytes != 96 {
                    // 返回稳定的参数错误。
                    return Err(rhi_invalid("D3d11 RHI shape rect uniform size is invalid"));
                }
                // 把已经由通用 renderer 验证的常量交给矩形 SDF shader。
                self.pipeline.draw_rhi_shape_rect(
                    &self.context,
                    &vertex_native,
                    vertex_stride,
                    index_native.as_ref(),
                    &uniform_native,
                    packet.vertex_count,
                    packet.index_count,
                    packet.first_vertex,
                    packet.first_index,
                    packet.base_vertex,
                    key == crate::native::present::rhi::pipeline_keys::SHAPE_RECT_ADDITIVE,
                )
            }
            // 原生扇形使用 64 字节 SectorConstants 和 position float2 quad。
            key if key == crate::native::present::rhi::pipeline_keys::SECTOR => {
                // 拒绝不匹配的 sector uniform 布局。
                if uniform.size_bytes != crate::native::present::rhi::SECTOR_UNIFORM_BYTES {
                    return Err(rhi_invalid("D3d11 RHI sector uniform size is invalid"));
                }
                // 把固定扇形 ABI 交给 D3D11 sector shader 编码。
                self.pipeline.draw_rhi_sector(
                    &self.context,
                    &vertex_native,
                    vertex_stride,
                    index_native.as_ref(),
                    &uniform_native,
                    packet.vertex_count,
                    packet.index_count,
                    packet.first_vertex,
                    packet.first_index,
                    packet.base_vertex,
                )
            }
            // 仿射阴影使用 96 字节 AffineShadowConstants。
            key if key == crate::native::present::rhi::pipeline_keys::BOX_SHADOW => {
                // 拒绝不匹配的 shadow uniform 布局。
                if uniform.size_bytes != 96 {
                    // 返回稳定的参数错误。
                    return Err(rhi_invalid("D3d11 RHI shadow uniform size is invalid"));
                }
                // 把已经由通用 renderer 验证的常量交给阴影 SDF shader。
                self.pipeline.draw_rhi_shadow(
                    &self.context,
                    &vertex_native,
                    vertex_stride,
                    index_native.as_ref(),
                    &uniform_native,
                    packet.vertex_count,
                    packet.index_count,
                    packet.first_vertex,
                    packet.first_index,
                    packet.base_vertex,
                )
            }
            // 线性/径向渐变使用 96 字节 affine GradientConstants。
            key if key == crate::native::present::rhi::pipeline_keys::GRADIENT_RECT => {
                // 拒绝不匹配的 gradient uniform 布局。
                if uniform.size_bytes != 96 {
                    // 返回稳定的参数错误。
                    return Err(rhi_invalid("D3d11 RHI gradient uniform size is invalid"));
                }
                // 把已经由通用 renderer 验证的常量交给渐变 shader。
                self.pipeline.draw_rhi_gradient_rect(
                    &self.context,
                    &vertex_native,
                    vertex_stride,
                    index_native.as_ref(),
                    &uniform_native,
                    packet.vertex_count,
                    packet.index_count,
                    packet.first_vertex,
                    packet.first_index,
                    packet.base_vertex,
                )
            }
            // 单方向 blur 使用 304 字节 BlurConstants 和 float2 区域 quad。
            key if key == crate::native::present::rhi::pipeline_keys::BLUR_PASS => {
                // 拒绝不匹配的 blur uniform 布局。
                if uniform.size_bytes != 76 * std::mem::size_of::<f32>() {
                    // 返回稳定的参数错误。
                    return Err(rhi_invalid("D3d11 RHI blur uniform size is invalid"));
                }
                // 读取 draw 前由 BindTexture 写入的纹理身份。
                let texture_handle = self
                    .rhi_device
                    .bound_texture
                    .ok_or_else(|| rhi_invalid("D3d11 RHI blur draw has no texture"))?;
                // 读取 draw 前由 BindTexture 写入的 sampler 身份。
                let sampler_handle = self
                    .rhi_device
                    .bound_sampler
                    .ok_or_else(|| rhi_invalid("D3d11 RHI blur draw has no sampler"))?;
                // 解析 source texture 并拒绝 R8 coverage 误用 blur ABI。
                let texture = self.rhi_device.texture(texture_handle)?;
                if texture.format == TextureFormat::R8Unorm {
                    // blur 只接受颜色纹理，避免单通道格式被解释为 premultiplied color。
                    return Err(rhi_invalid("D3d11 RHI blur source must be a color texture"));
                }
                // 复制 source SRV，结束资源表借用。
                let texture_srv = texture.srv.clone();
                // 解析 sampler state 并复制 COM 句柄。
                let sampler = self.rhi_device.sampler(sampler_handle)?;
                // 复制 sampler state。
                let sampler_native = sampler.native.clone();
                // 复制当前 pass 已绑定的 RTV。
                let target = self
                    .rhi_device
                    .active_target
                    .as_ref()
                    .cloned()
                    .ok_or_else(|| rhi_invalid("D3d11 RHI blur draw has no target"))?;
                // 把已经完成 region lowering 的对象交给 D3D11 blur ABI。
                self.pipeline.draw_rhi_blur_pass(
                    &self.context,
                    &vertex_native,
                    vertex_stride,
                    index_native.as_ref(),
                    &uniform_native,
                    &texture_srv,
                    &sampler_native,
                    &target,
                    packet.vertex_count,
                    packet.index_count,
                    packet.first_vertex,
                    packet.first_index,
                    packet.base_vertex,
                )
            }
            // 前置条件已经排除了其他 key，这里只为穷尽匹配保留 typed error。
            _ => Err(rhi_not_implemented("D3d11 RHI pipeline draw key")),
        }
    }
}
