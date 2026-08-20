//! D3D11 薄 RHI 的采样 quad draw 编码。

// 复用 pipeline 父模块的 D3D11 类型、错误和 shader 状态。
use super::*;

// 为 D3D11 pipeline 提供共享的 float8 sampled quad draw 实现。
impl D3d11Pipeline {
    // 使用指定像素 shader 编码一个已经完成绑定的 sampled quad。
    fn draw_rhi_sampled_quad(
        &self,
        context: &ID3D11DeviceContext,
        vertex: &ID3D11Buffer,
        vertex_stride: u32,
        index: Option<&D3d11IndexBinding>,
        uniform: &ID3D11Buffer,
        texture: &ID3D11ShaderResourceView,
        sampler: &ID3D11SamplerState,
        pixel_shader: &ID3D11PixelShader,
        shader_name: &str,
        blend_state: &ID3D11BlendState,
        vertex_count: u32,
        index_count: u32,
        first_vertex: u32,
        first_index: u32,
    ) -> Result<()> {
        // 非索引和索引绘制必须恰好选择一种范围。
        if (index.is_some() && index_count == 0) || (index.is_none() && vertex_count == 0) {
            // 返回稳定的参数错误。
            return Err(Error::new(
                Errc::InvalidArgument,
                format!("D3d11 RHI {shader_name} quad draw range is empty"),
            ));
        }
        // 绑定通用 sampled quad 的输入布局、资源和 blend 状态。
        // SAFETY: sampled texture、sampler、buffer 与 shader 均由当前 device 创建并存活，调用只发生在 owner thread。
        unsafe {
            // 输入布局与已有 glyph 的 float8 ABI 完全一致。
            context.IASetInputLayout(&self.layout_glyph);
            // 绑定本次 packet 的顶点 buffer 和步长。
            context.IASetVertexBuffers(
                0,
                1,
                Some(&Some(vertex.clone())),
                Some(&vertex_stride),
                Some(&0),
            );
            // 清理可能由前一个 packet 留下的索引绑定。
            bind_rhi_index_buffer(context, index);
            // 复用按 viewport uniform 计算位置的 glyph vertex shader。
            context.VSSetShader(&self.vs_glyph, None);
            // 根据 RHI pipeline 语义选择颜色纹理或 R8 coverage pixel shader。
            context.PSSetShader(pixel_shader, None);
            // 绑定通用 viewport uniform。
            context.VSSetConstantBuffers(0, Some(&[Some(uniform.clone())]));
            // MSDF 像素 shader 也从同一 slot 读取 source extent 和 range。
            context.PSSetConstantBuffers(0, Some(&[Some(uniform.clone())]));
            // 绑定当前 texture SRV。
            context.PSSetShaderResources(0, Some(&[Some(texture.clone())]));
            // 绑定当前 sampler。
            context.PSSetSamplers(0, Some(&[Some(sampler.clone())]));
            // 根据 pipeline 语义选择 SrcOver 或 Additive blend。
            context.OMSetBlendState(blend_state, None, self.rhi_sample_mask());
            // 按 packet 的索引形态编码实际 draw。
            if index.is_some() {
                // 共同 DrawRange 不暴露 base vertex，D3D11 固定使用零偏移。
                context.DrawIndexed(index_count, first_index, 0);
            } else {
                // 非索引 packet 直接使用顶点范围。
                context.Draw(vertex_count, first_vertex);
            }
            // 解除 SRV 绑定，避免下一次把同一纹理作为 RTV 时触发 hazard。
            context.PSSetShaderResources(0, Some(&[None]));
            // 解除 sampler 绑定，保持 pass 结束后的状态最小化。
            context.PSSetSamplers(0, Some(&[None]));
            // 解除像素 shader 的 uniform 绑定，保持 pass 状态边界清晰。
            context.PSSetConstantBuffers(0, Some(&[None]));
        }
        // 返回编码成功。
        Ok(())
    }

    // 执行薄 RHI 的 BGRA 颜色纹理 quad draw packet。
    pub(crate) fn draw_rhi_textured_quad(
        &self,
        context: &ID3D11DeviceContext,
        vertex: &ID3D11Buffer,
        vertex_stride: u32,
        index: Option<&D3d11IndexBinding>,
        uniform: &ID3D11Buffer,
        texture: &ID3D11ShaderResourceView,
        sampler: &ID3D11SamplerState,
        blend: PipelineBlend,
        vertex_count: u32,
        index_count: u32,
        first_vertex: u32,
        first_index: u32,
    ) -> Result<()> {
        // 从共享 pipeline 契约映射唯一 blend state。
        let blend_state = self.rhi_blend_state(blend);
        // 把颜色纹理 draw 委托给共享 sampled quad 编码器。
        self.draw_rhi_sampled_quad(
            context,
            vertex,
            vertex_stride,
            index,
            uniform,
            texture,
            sampler,
            &self.ps_rhi_textured,
            "textured",
            blend_state,
            vertex_count,
            index_count,
            first_vertex,
            first_index,
        )
    }

    // 执行薄 RHI 的 R8 字形覆盖率 quad draw packet。
    pub(crate) fn draw_rhi_coverage_quad(
        &self,
        context: &ID3D11DeviceContext,
        vertex: &ID3D11Buffer,
        vertex_stride: u32,
        index: Option<&D3d11IndexBinding>,
        uniform: &ID3D11Buffer,
        texture: &ID3D11ShaderResourceView,
        sampler: &ID3D11SamplerState,
        blend: PipelineBlend,
        vertex_count: u32,
        index_count: u32,
        first_vertex: u32,
        first_index: u32,
    ) -> Result<()> {
        // 选择已有的 coverage quantization 与 premultiply shader。
        self.draw_rhi_sampled_quad(
            context,
            vertex,
            vertex_stride,
            index,
            uniform,
            texture,
            sampler,
            &self.ps_glyph,
            "glyph",
            self.rhi_blend_state(blend),
            vertex_count,
            index_count,
            first_vertex,
            first_index,
        )
    }

    // 执行薄 RHI 的 RGBA8 MSDF 字形 quad draw packet。
    pub(crate) fn draw_rhi_msdf_quad(
        &self,
        context: &ID3D11DeviceContext,
        vertex: &ID3D11Buffer,
        vertex_stride: u32,
        index: Option<&D3d11IndexBinding>,
        uniform: &ID3D11Buffer,
        texture: &ID3D11ShaderResourceView,
        sampler: &ID3D11SamplerState,
        blend: PipelineBlend,
        vertex_count: u32,
        index_count: u32,
        first_vertex: u32,
        first_index: u32,
    ) -> Result<()> {
        // 选择 MSDF median/fwidth coverage shader 和 premultiplied blend。
        self.draw_rhi_sampled_quad(
            context,
            vertex,
            vertex_stride,
            index,
            uniform,
            texture,
            sampler,
            &self.ps_msdf,
            "msdf",
            self.rhi_blend_state(blend),
            vertex_count,
            index_count,
            first_vertex,
            first_index,
        )
    }
}
