//! D3D11 薄 RHI 的解析抗锯齿线段 draw ABI。

use super::*;
use crate::native::presentation::graphics::d3d_shader_source::LINE_HLSL;

// 编译共享 Line HLSL 的 VS/PS。
pub(super) fn create_line_shaders(
    device: &ID3D11Device,
) -> Result<(ID3D11VertexShader, ID3D11PixelShader)> {
    let vs_blob = compile_shader(LINE_HLSL, c"VSMain", c"vs_4_0")?;
    let ps_blob = compile_shader(LINE_HLSL, c"PSMain", c"ps_4_0")?;
    let mut vs = None;
    // SAFETY: 编译结果在同步创建期间保持存活，device 属于当前 owner thread。
    unsafe {
        device
            .CreateVertexShader(
                std::slice::from_raw_parts(
                    vs_blob.GetBufferPointer() as *const u8,
                    vs_blob.GetBufferSize(),
                ),
                None,
                Some(&mut vs),
            )
            .map_err(|error| d3d_error("CreateVertexShader(line)", error))?;
    }
    let vs = vs.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no line VS"))?;
    let mut ps = None;
    // SAFETY: 编译结果在同步创建期间保持存活，device 属于当前 owner thread。
    unsafe {
        device
            .CreatePixelShader(
                std::slice::from_raw_parts(
                    ps_blob.GetBufferPointer() as *const u8,
                    ps_blob.GetBufferSize(),
                ),
                None,
                Some(&mut ps),
            )
            .map_err(|error| d3d_error("CreatePixelShader(line)", error))?;
    }
    let ps = ps.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no line PS"))?;
    Ok((vs, ps))
}

impl D3d11Pipeline {
    // 编码一个已经通过共享 ABI 校验的解析线段 draw packet。
    pub(crate) fn draw_rhi_line(
        &self,
        context: &ID3D11DeviceContext,
        vertex: &ID3D11Buffer,
        vertex_stride: u32,
        index: Option<&D3d11IndexBinding>,
        uniform: &ID3D11Buffer,
        blend: PipelineBlend,
        vertex_count: u32,
        index_count: u32,
        first_vertex: u32,
        first_index: u32,
    ) -> Result<()> {
        if (index.is_some() && index_count == 0) || (index.is_none() && vertex_count == 0) {
            return Err(Error::new(
                Errc::InvalidArgument,
                "D3d11 RHI line draw range is empty",
            ));
        }
        // SAFETY: 资源、固定 shader ABI 和 owner thread 已由调用边界验证。
        unsafe {
            context.IASetInputLayout(&self.layout);
            context.IASetVertexBuffers(
                0,
                1,
                Some(&Some(vertex.clone())),
                Some(&vertex_stride),
                Some(&0),
            );
            bind_rhi_index_buffer(context, index);
            context.VSSetShader(&self.vs_line, None);
            context.PSSetShader(&self.ps_line, None);
            context.VSSetConstantBuffers(0, Some(&[Some(uniform.clone())]));
            context.PSSetConstantBuffers(0, Some(&[Some(uniform.clone())]));
            context.OMSetBlendState(self.rhi_blend_state(blend), None, self.rhi_sample_mask());
            if index.is_some() {
                context.DrawIndexed(index_count, first_index, 0);
            } else {
                context.Draw(vertex_count, first_vertex);
            }
        }
        Ok(())
    }
}
