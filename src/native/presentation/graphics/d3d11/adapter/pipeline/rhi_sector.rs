//! D3D11 薄 RHI 的分析抗锯齿扇形 draw ABI。

// 复用 pipeline 父模块的 D3D11 类型、编译辅助和错误类型。
use super::*;
use crate::native::presentation::graphics::d3d_shader_source::SECTOR_HLSL;

// 编译 SectorHLSL 的 VS/PS，避免把 shader 对象暴露到通用 RHI。
pub(super) fn create_sector_shaders(
    device: &ID3D11Device,
) -> Result<(ID3D11VertexShader, ID3D11PixelShader)> {
    // 分别编译顶点和像素入口，确保两阶段都在 probe 阶段暴露错误。
    let vs_blob = compile_shader(SECTOR_HLSL, c"VSMain", c"vs_4_0")?;
    let ps_blob = compile_shader(SECTOR_HLSL, c"PSMain", c"ps_4_0")?;
    // 创建顶点 shader 的输出槽。
    let mut vs = None;
    // SAFETY: blob 由本函数刚编译，device 属于当前 owner thread。
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
            .map_err(|error| d3d_error("CreateVertexShader(sector)", error))?;
    }
    // 拒绝驱动返回的空顶点 shader。
    let vs = vs.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no sector VS"))?;
    // 创建像素 shader 的输出槽。
    let mut ps = None;
    // SAFETY: blob 由本函数刚编译，device 属于当前 owner thread。
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
            .map_err(|error| d3d_error("CreatePixelShader(sector)", error))?;
    }
    // 返回已经创建的扇形 shader 对。
    let ps = ps.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no sector PS"))?;
    Ok((vs, ps))
}

// 为 D3D11 pipeline 编码薄 RHI sector draw packet。
impl D3d11Pipeline {
    // 执行薄 RHI 的原生扇形 draw packet。
    pub(crate) fn draw_rhi_sector(
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
        // 非索引和索引绘制必须恰好选择一种范围。
        if (index.is_some() && index_count == 0) || (index.is_none() && vertex_count == 0) {
            return Err(Error::new(
                Errc::InvalidArgument,
                "D3d11 RHI sector draw range is empty",
            ));
        }
        // 绑定固定输入布局、扇形 shader、常量和 premultiplied blend。
        // SAFETY: sector 资源与固定 shader ABI 已在上方验证，全部 COM 对象存活且 context 位于 owner thread。
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
            context.VSSetShader(&self.vs_sector, None);
            context.PSSetShader(&self.ps_sector, None);
            context.VSSetConstantBuffers(0, Some(&[Some(uniform.clone())]));
            context.PSSetConstantBuffers(0, Some(&[Some(uniform.clone())]));
            context.OMSetBlendState(self.rhi_blend_state(blend), None, self.rhi_sample_mask());
            if index.is_some() {
                // 共同 DrawRange 不暴露 base vertex，D3D11 固定使用零偏移。
                context.DrawIndexed(index_count, first_index, 0);
            } else {
                context.Draw(vertex_count, first_vertex);
            }
        }
        // 返回编码成功。
        Ok(())
    }
}
