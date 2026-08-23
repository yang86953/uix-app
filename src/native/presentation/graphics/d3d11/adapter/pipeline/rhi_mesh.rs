// 复用 D3D11 pipeline 的底层资源与错误类型。
use super::*;

// 为 D3D11 pipeline 编码薄 RHI 实心网格 draw packet。
impl D3d11Pipeline {
    // 执行薄 RHI 的实心三角 draw packet，不接收任何 UI 高层语义。
    pub(crate) fn draw_rhi_solid_mesh(
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
            // 返回稳定的参数错误。
            return Err(Error::new(
                Errc::InvalidArgument,
                "D3d11 RHI solid mesh draw range is empty",
            ));
        }
        // 绑定 RHI packet 对应的固定 mesh pipeline。
        // SAFETY: mesh 资源与 pipeline 状态由当前 device 创建并存活，绑定和 Draw 在 immediate context owner thread 执行。
        unsafe {
            context.IASetInputLayout(&self.layout_mesh);
            context.IASetVertexBuffers(
                0,
                1,
                Some(&Some(vertex.clone())),
                Some(&vertex_stride),
                Some(&0),
            );
            // None 会清除前一个 packet 留下的索引绑定。
            bind_rhi_index_buffer(context, index);
            context.VSSetShader(&self.vs_mesh, None);
            context.PSSetShader(&self.ps_mesh, None);
            context.VSSetConstantBuffers(0, Some(&[Some(uniform.clone())]));
            context.PSSetConstantBuffers(0, Some(&[Some(uniform.clone())]));
            context.OMSetBlendState(self.rhi_blend_state(blend), None, self.rhi_sample_mask());
            if index.is_some() {
                // 共同 DrawRange 不暴露 base vertex，D3D11 固定使用零偏移。
                context.DrawIndexed(index_count, first_index, 0);
            } else {
                // 非索引 packet 直接使用顶点范围。
                context.Draw(vertex_count, first_vertex);
            }
        }
        // 返回编码成功。
        Ok(())
    }
}
