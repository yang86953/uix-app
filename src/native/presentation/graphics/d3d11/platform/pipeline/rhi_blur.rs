//! D3D11 薄 RHI 的单方向高斯 blur draw ABI。

#![allow(dead_code)]

// 复用 D3D11 pipeline 的资源类型、错误辅助和常量布局。
use super::*;

// 为 D3d11Pipeline 编码 RHI BLUR_PASS draw packet。
impl D3d11Pipeline {
    // 使用 FramePlan 提供的 BlurConstants，在当前 render target 上写入一个区域。
    pub(crate) fn draw_rhi_blur_pass(
        &self,
        context: &ID3D11DeviceContext,
        vertex: &ID3D11Buffer,
        vertex_stride: u32,
        index: Option<&ID3D11Buffer>,
        uniform: &ID3D11Buffer,
        texture: &ID3D11ShaderResourceView,
        sampler: &ID3D11SamplerState,
        target: &ID3D11RenderTargetView,
        vertex_count: u32,
        index_count: u32,
        first_vertex: u32,
        first_index: u32,
        base_vertex: i32,
    ) -> Result<()> {
        // blur 区域顶点固定为 position float2。
        if vertex_stride != (2 * size_of::<f32>()) as u32 {
            // 阻止 adapter 以错误 stride 读取 NDC 顶点。
            return Err(Error::new(
                Errc::InvalidArgument,
                "D3d11 RHI blur vertex stride must be float2",
            ));
        }
        // 非索引和索引 draw 必须恰好选择一种范围。
        if (index.is_some() && index_count == 0) || (index.is_none() && vertex_count == 0) {
            // 返回稳定的空 draw 错误。
            return Err(Error::new(
                Errc::InvalidArgument,
                "D3d11 RHI blur draw range is empty",
            ));
        }
        // 绑定已经由 FramePlan 验证过的 blur shader ABI 和资源。
        unsafe {
            // Blur VS 输入与 RECT position layout 相同。
            context.IASetInputLayout(&self.layout);
            // 区域 quad 固定由两个三角形组成。
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            // 绑定本次 blur 的 NDC vertex buffer。
            context.IASetVertexBuffers(
                0,
                1,
                Some(&Some(vertex.clone())),
                Some(&vertex_stride),
                Some(&0),
            );
            // 清理可能遗留的 index binding。
            context.IASetIndexBuffer(index, DXGI_FORMAT_R32_UINT, 0);
            // 绑定当前 pass 的目标 RTV。
            context.OMSetRenderTargets(Some(&[Some(target.clone())]), None);
            // 绑定 blur 专用 vertex shader。
            context.VSSetShader(&self.vs_blit, None);
            // 绑定 blur 专用 pixel shader。
            context.PSSetShader(&self.ps_blur, None);
            // BlurCB 同时由 VS 和 PS 读取。
            context.VSSetConstantBuffers(0, Some(&[Some(uniform.clone())]));
            context.PSSetConstantBuffers(0, Some(&[Some(uniform.clone())]));
            // 绑定当前方向需要采样的 source SRV 和 sampler。
            context.PSSetShaderResources(0, Some(&[Some(texture.clone())]));
            context.PSSetSamplers(0, Some(&[Some(sampler.clone())]));
            // 复用无剔除 rasterizer，保留 RHI 设置的 viewport/scissor。
            context.RSSetState(&self.rasterizer);
            // blur 是覆盖写入，不能把高斯 taps 再按 alpha 混合叠加。
            context.OMSetBlendState(&self.blend_replace, None, 0xffff_ffff);
            // 按 packet 的索引形态执行 draw。
            if index.is_some() {
                // 索引 ABI 固定为 uint32。
                context.DrawIndexed(index_count, first_index, base_vertex);
            } else {
                // 非索引 packet 使用六顶点区域 quad。
                context.Draw(vertex_count, first_vertex);
            }
            // 解除 SRV，避免下一 pass 把同一资源作为 RTV 时产生 hazard。
            context.PSSetShaderResources(0, Some(&[None]));
            // 解除 sampler，保持 pass 结束后的状态最小化。
            context.PSSetSamplers(0, Some(&[None]));
        }
        // 返回 blur draw 编码成功。
        Ok(())
    }
}
