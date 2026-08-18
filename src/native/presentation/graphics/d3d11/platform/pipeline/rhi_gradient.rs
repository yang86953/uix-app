//! D3D11 薄 RHI 的渐变 draw ABI。

// 复用父 pipeline 的 D3D11 类型、错误和 shader 状态。
use super::*;

// 为 D3D11Pipeline 提供通用 RHI gradient draw 原语。
impl D3d11Pipeline {
    // 执行薄 RHI 的线性/径向渐变 quad draw packet。
    pub(crate) fn draw_rhi_gradient_rect(
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
        base_vertex: i32,
    ) -> Result<()> {
        // 非索引和索引绘制必须恰好选择一种范围。
        if (index.is_some() && index_count == 0) || (index.is_none() && vertex_count == 0) {
            // 返回稳定的参数错误。
            return Err(Error::new(
                Errc::InvalidArgument,
                "D3d11 RHI gradient draw range is empty",
            ));
        }
        // 绑定渐变 shader、常量和 alpha blend 状态。
        // SAFETY: buffer、shader、layout 与 blend 均由当前 device 创建并存活，绑定和 Draw 在 owner thread 执行。
        unsafe {
            context.IASetInputLayout(&self.layout);
            context.IASetVertexBuffers(
                0,
                1,
                Some(&Some(vertex.clone())),
                Some(&vertex_stride),
                Some(&0),
            );
            // None 会清除前一个 packet 留下的索引绑定。
            bind_rhi_index_buffer(context, index);
            context.VSSetShader(&self.vs_grad, None);
            context.PSSetShader(&self.ps_grad, None);
            context.VSSetConstantBuffers(0, Some(&[Some(uniform.clone())]));
            context.PSSetConstantBuffers(0, Some(&[Some(uniform.clone())]));
            context.OMSetBlendState(self.rhi_blend_state(blend), None, self.rhi_sample_mask());
            if index.is_some() {
                // 索引格式已由共享绑定映射，base vertex 继续使用 packet 值。
                context.DrawIndexed(index_count, first_index, base_vertex);
            } else {
                // 单位 quad 直接使用顶点范围。
                context.Draw(vertex_count, first_vertex);
            }
        }
        // 返回编码成功。
        Ok(())
    }
}
