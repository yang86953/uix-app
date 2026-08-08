use super::*;

impl D3d11Pipeline {
    pub(crate) fn bind_grad_pipeline(
        &self,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
    ) {
        let stride = (2 * size_of::<f32>()) as u32;
        let offset = 0u32;
        unsafe {
            context.IASetInputLayout(&self.layout);
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            context.IASetVertexBuffers(
                0,
                1,
                Some(&Some(self.vb_unit.clone())),
                Some(&stride),
                Some(&offset),
            );
            context.VSSetShader(&self.vs_grad, None);
            context.PSSetShader(&self.ps_grad, None);
            context.VSSetConstantBuffers(0, Some(&[Some(self.cb_grad.clone())]));
            context.PSSetConstantBuffers(0, Some(&[Some(self.cb_grad.clone())]));
            context.RSSetState(&self.rasterizer);
            context.OMSetBlendState(&self.blend_alpha, None, 0xffff_ffff);
            let (sx, sy, sw, sh) =
                scissor.unwrap_or((0, 0, viewport_w.ceil() as i32, viewport_h.ceil() as i32));
            let rect = ::windows::Win32::Foundation::RECT {
                left: sx,
                top: sy,
                right: sx + sw.max(0),
                bottom: sy + sh.max(0),
            };
            context.RSSetScissorRects(Some(&[rect]));
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "the shader constants stay explicit at the legacy D3D11 command boundary"
    )]
    pub(crate) fn draw_grad_constants(
        &self,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        local_w: f32,
        local_h: f32,
        corners: [[f32; 2]; 4],
        color_a: [f32; 4],
        color_b: [f32; 4],
        params: [f32; 4],
    ) -> Result<()> {
        // 退化逻辑尺寸或异常 affine 四角不产生无效 GPU draw。
        if local_w <= 0.0
            || local_h <= 0.0
            || !corners
                .iter()
                .flatten()
                .all(|component| component.is_finite())
        {
            return Ok(());
        }
        // 线性 shader 使用逻辑尺寸计算 t，避免 skew 改变渐变语义。
        let mut params = params;
        if params[0] < 0.5 {
            params[2] = local_w;
            params[3] = local_h;
        }
        // 由 TL、TR、BL 四角重建 affine quad 的原点与两条边。
        let edge_x = [corners[1][0] - corners[0][0], corners[1][1] - corners[0][1]];
        let edge_y = [corners[3][0] - corners[0][0], corners[3][1] - corners[0][1]];
        // 将 affine geometry 与渐变参数写入现有 96 字节常量布局。
        let constants = GradientConstants {
            viewport: [viewport_w, viewport_h],
            _pad0: [0.0, 0.0],
            origin_edge_x: [corners[0][0], corners[0][1], edge_x[0], edge_x[1]],
            edge_y: [edge_y[0], edge_y[1], 0.0, 0.0],
            color_a,
            color_b,
            params,
        };
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            context
                .Map(
                    &self.cb_grad,
                    0,
                    D3D11_MAP_WRITE_DISCARD,
                    0,
                    Some(&mut mapped),
                )
                .map_err(|e| d3d_error("Map(cb_grad)", e))?;
            std::ptr::copy_nonoverlapping(
                (&constants as *const GradientConstants).cast::<u8>(),
                mapped.pData.cast(),
                size_of::<GradientConstants>(),
            );
            context.Unmap(&self.cb_grad, 0);
            context.Draw(6, 0);
        }
        Ok(())
    }

    pub fn draw_linear_gradients(
        &mut self,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuLinearGradientRect],
    ) -> Result<()> {
        if rects.is_empty() || viewport_w <= 0.0 || viewport_h <= 0.0 {
            return Ok(());
        }
        self.bind_grad_pipeline(context, viewport_w, viewport_h, scissor);
        for rect in rects {
            // 过滤退化矩形、非法方向、异常 affine 四角和颜色。
            if rect.w <= 0.0
                || rect.h <= 0.0
                || rect.dir > 3
                || !rect
                    .corners
                    .iter()
                    .flatten()
                    .all(|component| component.is_finite())
                || !rect
                    .color_a
                    .iter()
                    .chain(rect.color_b.iter())
                    .all(|component| component.is_finite())
            {
                continue;
            }
            self.draw_grad_constants(
                context,
                viewport_w,
                viewport_h,
                rect.w,
                rect.h,
                rect.corners,
                rect.color_a,
                rect.color_b,
                [0.0, rect.dir as f32, 0.0, 0.0],
            )?;
        }
        Ok(())
    }

    pub fn draw_radial_gradients(
        &mut self,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        grads: &[GpuRadialGradient],
    ) -> Result<()> {
        if grads.is_empty() || viewport_w <= 0.0 || viewport_h <= 0.0 {
            return Ok(());
        }
        self.bind_grad_pipeline(context, viewport_w, viewport_h, scissor);
        for g in grads {
            let outer = g.outer_r.max(0.0);
            // 过滤退化半径、越界半径、异常 affine 四角和颜色。
            if outer <= 0.0
                || !g.inner_r.is_finite()
                || g.inner_r < 0.0
                || g.inner_r > outer
                || !g
                    .corners
                    .iter()
                    .flatten()
                    .all(|component| component.is_finite())
                || !g
                    .color_inner
                    .iter()
                    .chain(g.color_outer.iter())
                    .all(|component| component.is_finite())
            {
                continue;
            }
            self.draw_grad_constants(
                context,
                viewport_w,
                viewport_h,
                outer * 2.0,
                outer * 2.0,
                g.corners,
                g.color_inner,
                g.color_outer,
                [1.0, (g.inner_r / outer).min(1.0) * 0.5, 0.5, 0.0],
            )?;
        }
        Ok(())
    }

    pub(crate) fn ensure_mesh_vb(
        &mut self,
        device: &ID3D11Device,
        float_count: usize,
    ) -> Result<()> {
        if float_count <= self.vb_mesh_capacity_floats {
            return Ok(());
        }
        let mut cap = self.vb_mesh_capacity_floats.max(MESH_VB_INITIAL_FLOATS);
        while cap < float_count {
            cap = cap.saturating_mul(2);
        }
        self.vb_mesh = create_dynamic_vb(device, cap * size_of::<f32>())?;
        self.vb_mesh_capacity_floats = cap;
        Ok(())
    }

    /// Draw CPU-tessellated solid triangle meshes (path fill/stroke).
    pub fn draw_solid_meshes(
        &mut self,
        device: &ID3D11Device,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        meshes: &[GpuSolidMesh],
    ) -> Result<()> {
        if meshes.is_empty() || viewport_w <= 0.0 || viewport_h <= 0.0 {
            return Ok(());
        }
        let (sx, sy, sw, sh) =
            scissor.unwrap_or((0, 0, viewport_w.ceil() as i32, viewport_h.ceil() as i32));
        let scissor_rect = ::windows::Win32::Foundation::RECT {
            left: sx,
            top: sy,
            right: sx + sw.max(0),
            bottom: sy + sh.max(0),
        };
        for mesh in meshes {
            let verts = mesh.vertices.as_ref();
            if verts.len() < 6 || verts.len() % 2 != 0 {
                continue;
            }
            let vert_count = (verts.len() / 2) as u32;
            if !vert_count.is_multiple_of(3) {
                continue;
            }
            self.ensure_mesh_vb(device, verts.len())?;
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            unsafe {
                context
                    .Map(
                        &self.vb_mesh,
                        0,
                        D3D11_MAP_WRITE_DISCARD,
                        0,
                        Some(&mut mapped),
                    )
                    .map_err(|e| d3d_error("Map(vb_mesh)", e))?;
                std::ptr::copy_nonoverlapping(
                    verts.as_ptr().cast::<u8>(),
                    mapped.pData.cast(),
                    std::mem::size_of_val(verts),
                );
                context.Unmap(&self.vb_mesh, 0);
            }
            let constants = MeshConstants {
                viewport: [viewport_w, viewport_h],
                _pad0: [0.0, 0.0],
                color: mesh.rgba,
            };
            let mut mapped_cb = D3D11_MAPPED_SUBRESOURCE::default();
            unsafe {
                context
                    .Map(
                        &self.cb_mesh,
                        0,
                        D3D11_MAP_WRITE_DISCARD,
                        0,
                        Some(&mut mapped_cb),
                    )
                    .map_err(|e| d3d_error("Map(cb_mesh)", e))?;
                std::ptr::copy_nonoverlapping(
                    (&constants as *const MeshConstants).cast::<u8>(),
                    mapped_cb.pData.cast(),
                    size_of::<MeshConstants>(),
                );
                context.Unmap(&self.cb_mesh, 0);

                let stride = (2 * size_of::<f32>()) as u32;
                let offset = 0u32;
                context.IASetInputLayout(&self.layout);
                context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                context.IASetVertexBuffers(
                    0,
                    1,
                    Some(&Some(self.vb_mesh.clone())),
                    Some(&stride),
                    Some(&offset),
                );
                context.VSSetShader(&self.vs_mesh, None);
                context.PSSetShader(&self.ps_mesh, None);
                context.VSSetConstantBuffers(0, Some(&[Some(self.cb_mesh.clone())]));
                context.PSSetConstantBuffers(0, Some(&[Some(self.cb_mesh.clone())]));
                context.RSSetState(&self.rasterizer);
                context.OMSetBlendState(&self.blend_alpha, None, 0xffff_ffff);
                context.RSSetScissorRects(Some(&[scissor_rect]));
                context.Draw(vert_count, 0);
            }
        }
        Ok(())
    }

    // 执行薄 RHI 的实心三角 draw packet，不接收任何 UI 高层语义。
    pub(crate) fn draw_rhi_solid_mesh(
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
    ) -> Result<()> {
        // 该 pipeline 的 shader ABI 只接受位置 float2。
        if vertex_stride != (2 * size_of::<f32>()) as u32 {
            // 把错误留在 RHI adapter，不让 D3D11 读错步长。
            return Err(Error::new(
                Errc::InvalidArgument,
                "D3d11 RHI solid mesh stride must be float2",
            ));
        }
        // 非索引和索引绘制必须恰好选择一种范围。
        if (index.is_some() && index_count == 0) || (index.is_none() && vertex_count == 0) {
            // 返回稳定的参数错误。
            return Err(Error::new(
                Errc::InvalidArgument,
                "D3d11 RHI solid mesh draw range is empty",
            ));
        }
        // 绑定 RHI packet 对应的固定 mesh pipeline。
        unsafe {
            context.IASetInputLayout(&self.layout);
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            context.IASetVertexBuffers(
                0,
                1,
                Some(&Some(vertex.clone())),
                Some(&vertex_stride),
                Some(&0),
            );
            // None 会清除前一个 packet 留下的索引绑定。
            context.IASetIndexBuffer(index, DXGI_FORMAT_R32_UINT, 0);
            context.VSSetShader(&self.vs_mesh, None);
            context.PSSetShader(&self.ps_mesh, None);
            context.VSSetConstantBuffers(0, Some(&[Some(uniform.clone())]));
            context.PSSetConstantBuffers(0, Some(&[Some(uniform.clone())]));
            context.RSSetState(&self.rasterizer);
            context.OMSetBlendState(&self.blend_alpha, None, 0xffff_ffff);
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

    /// Draw axis-aligned box / ambient shadows (SDF coverage, matches CPU).
    pub fn draw_box_shadows(
        &mut self,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        shadows: &[GpuBoxShadow],
    ) -> Result<()> {
        if shadows.is_empty() || viewport_w <= 0.0 || viewport_h <= 0.0 {
            return Ok(());
        }
        let (sx, sy, sw, sh) =
            scissor.unwrap_or((0, 0, viewport_w.ceil() as i32, viewport_h.ceil() as i32));
        let scissor_rect = ::windows::Win32::Foundation::RECT {
            left: sx,
            top: sy,
            right: sx + sw.max(0),
            bottom: sy + sh.max(0),
        };
        let stride = (2 * size_of::<f32>()) as u32;
        let offset = 0u32;
        unsafe {
            context.IASetInputLayout(&self.layout);
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            context.IASetVertexBuffers(
                0,
                1,
                Some(&Some(self.vb_unit.clone())),
                Some(&stride),
                Some(&offset),
            );
            context.VSSetShader(&self.vs_shadow, None);
            context.PSSetShader(&self.ps_shadow, None);
            context.VSSetConstantBuffers(0, Some(&[Some(self.cb_shadow.clone())]));
            context.PSSetConstantBuffers(0, Some(&[Some(self.cb_shadow.clone())]));
            context.RSSetState(&self.rasterizer);
            context.OMSetBlendState(&self.blend_alpha, None, 0xffff_ffff);
            context.RSSetScissorRects(Some(&[scissor_rect]));
        }
        for shadow in shadows {
            if shadow.w <= 0.0 || shadow.h <= 0.0 || shadow.rgba[3] <= 0.0 {
                continue;
            }
            let blur = shadow.blur_x.max(shadow.blur_y).max(0.0);
            let constants = ShadowConstants {
                viewport: [viewport_w, viewport_h],
                _pad0: [0.0, 0.0],
                rect: [
                    shadow.x + shadow.offset_x,
                    shadow.y + shadow.offset_y,
                    shadow.w,
                    shadow.h,
                ],
                color: shadow.rgba,
                radius: shadow.radius,
                params: [blur, if shadow.ambient { 1.0 } else { 0.0 }, 0.0, 0.0],
            };
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            unsafe {
                context
                    .Map(
                        &self.cb_shadow,
                        0,
                        D3D11_MAP_WRITE_DISCARD,
                        0,
                        Some(&mut mapped),
                    )
                    .map_err(|e| d3d_error("Map(cb_shadow)", e))?;
                std::ptr::copy_nonoverlapping(
                    (&constants as *const ShadowConstants).cast::<u8>(),
                    mapped.pData.cast(),
                    size_of::<ShadowConstants>(),
                );
                context.Unmap(&self.cb_shadow, 0);
                context.Draw(6, 0);
            }
        }
        Ok(())
    }
}
