//! D3D11 薄 RHI 的圆角与描边矩形 draw 编码。

// 复用 pipeline 父模块的 D3D11 类型、错误和 shader 状态。
use super::*;

// 为 D3D11 pipeline 提供 RectConstants 的通用 RHI draw 原语。
impl D3d11Pipeline {
    // 执行薄 RHI 的圆角或描边矩形 quad draw packet。
    pub(crate) fn draw_rhi_shape_rect(
        &self,
        context: &ID3D11DeviceContext,
        vertex: &ID3D11Buffer,
        vertex_stride: u32,
        index: Option<&ID3D11Buffer>,
        uniform: &ID3D11Buffer,
        vertex_count: u32,
        index_count: u32,
        first_vertex: u32,
        first_index: u32,
        base_vertex: i32,
        additive: bool,
    ) -> Result<()> {
        // shape quad 的顶点 ABI 是 position float2。
        if vertex_stride != (2 * size_of::<f32>()) as u32 {
            // 把错误留在 RHI adapter，不让 D3D11 读错属性布局。
            return Err(Error::new(
                Errc::InvalidArgument,
                "D3d11 RHI shape rect stride must be float2",
            ));
        }
        // 非索引和索引绘制必须恰好选择一种范围。
        if (index.is_some() && index_count == 0) || (index.is_none() && vertex_count == 0) {
            // 返回稳定的参数错误。
            return Err(Error::new(
                Errc::InvalidArgument,
                "D3d11 RHI shape rect draw range is empty",
            ));
        }
        // 绑定现有 RECT_HLSL 与本次 RectConstants uniform。
        unsafe {
            // 输入布局与单位 quad 的 float2 ABI 一致。
            context.IASetInputLayout(&self.layout);
            // 所有 shape 都由两个三角形组成。
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            // 绑定本次 packet 的顶点 buffer。
            context.IASetVertexBuffers(
                0,
                1,
                Some(&Some(vertex.clone())),
                Some(&vertex_stride),
                Some(&0),
            );
            // 清理可能由前一个 packet 留下的索引绑定。
            context.IASetIndexBuffer(index, DXGI_FORMAT_R32_UINT, 0);
            // 绑定已有的矩形 SDF vertex/pixel shader。
            context.VSSetShader(&self.vs_rect, None);
            context.PSSetShader(&self.ps_rect, None);
            // 两个 shader stage 读取同一个 80 字节常量 buffer。
            context.VSSetConstantBuffers(0, Some(&[Some(uniform.clone())]));
            context.PSSetConstantBuffers(0, Some(&[Some(uniform.clone())]));
            // 复用无剔除 rasterizer 和当前 shape pipeline 的 blend 语义。
            context.RSSetState(&self.rasterizer);
            context.OMSetBlendState(
                if additive {
                    &self.blend_additive
                } else {
                    &self.blend_premultiplied
                },
                None,
                0xffff_ffff,
            );
            // 按 packet 的索引形态编码实际 draw。
            if index.is_some() {
                // 索引 ABI 固定为 uint32，base vertex 保留 D3D11 原生语义。
                context.DrawIndexed(index_count, first_index, base_vertex);
            } else {
                // 非索引 packet 直接使用顶点范围。
                context.Draw(vertex_count, first_vertex);
            }
        }
        // 返回编码成功。
        Ok(())
    }
}
