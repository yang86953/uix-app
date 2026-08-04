//! D3D11 薄 RHI 的分析抗锯齿扇形 shader 与 draw ABI。

// 复用 pipeline 父模块的 D3D11 类型、编译辅助和错误类型。
use super::*;

// 与 SECTOR_HLSL 的 SectorCB 保持 16 字节寄存器对齐。
#[repr(C)]
// 允许按值复制到动态 constant buffer。
#[derive(Clone, Copy)]
// 描述一个 viewport、外接矩形、颜色和角度范围。
struct SectorConstants {
    // 记录当前 drawable 的宽高。
    viewport: [f32; 2],
    // 对齐 HLSL 的下一个 float4 寄存器。
    _pad0: [f32; 2],
    // 记录 sector 的外接矩形。
    rect: [f32; 4],
    // 记录待 premultiply 的直通颜色。
    color: [f32; 4],
    // 记录起始角和顺时针 sweep 角。
    angles: [f32; 4],
}

// 为轴对齐原生扇形定义固定的 SectorConstants 与单位 quad shader。
pub(crate) const SECTOR_HLSL: &str = r#"
cbuffer SectorCB : register(b0)
{
    float2 u_viewport;
    float2 _pad0;
    float4 u_rect;
    float4 u_color;
    float4 u_angles;
};

struct VSIn {
    float2 pos : POSITION;
};

struct VSOut {
    float4 pos : SV_POSITION;
    float2 local : TEXCOORD0;
    float2 rect_size : TEXCOORD1;
};

VSOut VSMain(VSIn input)
{
    VSOut output;
    float2 pos = u_rect.xy + input.pos * u_rect.zw;
    float2 ndc = (pos / u_viewport) * 2.0 - 1.0;
    ndc.y = -ndc.y;
    output.pos = float4(ndc, 0.0, 1.0);
    output.local = input.pos * u_rect.zw;
    output.rect_size = u_rect.zw;
    return output;
}

float4 PSMain(VSOut input) : SV_Target
{
    const float TAU = 6.283185307179586;
    float2 unit = (input.local / max(input.rect_size, float2(0.0001, 0.0001)) - 0.5) * 2.0;
    float radius = length(unit);
    float radial_width = max(fwidth(radius), 0.0001);
    float radial_mask = saturate((1.0 - radius) / radial_width + 0.5);
    float angular_mask = 1.0;
    if (u_angles.y < TAU - 0.0001 && radius > 0.0001)
    {
        float angle = atan2(unit.y, unit.x);
        if (angle < 0.0)
            angle += TAU;
        float delta = fmod(angle - u_angles.x + TAU, TAU);
        float edge = min(delta, u_angles.y - delta);
        float angular_width = max(fwidth(angle), 0.0001);
        angular_mask = saturate(edge / angular_width + 0.5);
        if (delta > u_angles.y)
            angular_mask = 0.0;
    }
    float mask = radial_mask * angular_mask;
    if (mask <= 0.0)
        discard;
    float4 color = floor(saturate(u_color) * 255.0 + 0.5);
    float3 premul = floor(color.rgb * color.a / 255.0);
    return float4(premul * mask, color.a * mask) / 255.0;
}
"#;

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

// 为 D3D11 pipeline 编码一个 SectorConstants 单位 quad draw。
impl D3d11Pipeline {
    // 执行兼容层的原生 sector batch，复用薄 RHI 的单位 quad 与 shader。
    pub(crate) fn draw_sectors(
        &mut self,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        sectors: &[GpuSector],
    ) -> Result<()> {
        // 空 batch 或无效 viewport 不产生 D3D11 状态变化。
        if sectors.is_empty() || viewport_w <= 0.0 || viewport_h <= 0.0 {
            return Ok(());
        }
        // 沿用其它兼容绘制的 scissor 约定，避免 sector 绕过裁剪边界。
        let (sx, sy, sw, sh) =
            scissor.unwrap_or((0, 0, viewport_w.ceil() as i32, viewport_h.ceil() as i32));
        // 将逻辑 scissor 转换为 D3D11 的右下角坐标。
        let scissor_rect = RECT {
            left: sx,
            top: sy,
            right: sx + sw.max(0),
            bottom: sy + sh.max(0),
        };
        // 为所有 sector 绑定同一个单位 quad、shader 与 premultiplied blend。
        unsafe {
            context.RSSetScissorRects(Some(&[scissor_rect]));
        }
        // 逐个上传常量并提交六顶点单位 quad。
        for sector in sectors {
            // 拒绝退化几何、越界角度和异常颜色，保持与 OpenGL owner 一致。
            if !sector.cx.is_finite()
                || !sector.cy.is_finite()
                || !sector.radius.is_finite()
                || sector.radius <= 0.0
                || !sector.start_angle.is_finite()
                || !sector.sweep_angle.is_finite()
                || sector.start_angle < 0.0
                || sector.start_angle >= std::f32::consts::TAU
                || sector.sweep_angle <= 0.0
                || sector.sweep_angle > std::f32::consts::TAU
                || sector.rgba.iter().any(|component| !component.is_finite())
            {
                continue;
            }
            // 按 SectorCB 的 HLSL 16 字节寄存器布局构造常量。
            let constants = SectorConstants {
                viewport: [viewport_w, viewport_h],
                _pad0: [0.0, 0.0],
                rect: [
                    sector.cx - sector.radius,
                    sector.cy - sector.radius,
                    sector.radius * 2.0,
                    sector.radius * 2.0,
                ],
                color: sector.rgba,
                angles: [sector.start_angle, sector.sweep_angle, 0.0, 0.0],
            };
            // 复用既有动态常量 buffer；sector shader 只读取前 64 字节。
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            unsafe {
                context
                    .Map(&self.cb, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mapped))
                    .map_err(|error| d3d_error("Map(cb sector)", error))?;
                std::ptr::copy_nonoverlapping(
                    (&constants as *const SectorConstants).cast::<u8>(),
                    mapped.pData.cast(),
                    size_of::<SectorConstants>(),
                );
                context.Unmap(&self.cb, 0);
            }
            // 使用 RHI draw ABI，确保 shader、layout 和混合状态保持同一所有权。
            self.draw_rhi_sector(
                context,
                &self.vb_unit,
                (2 * size_of::<f32>()) as u32,
                None,
                &self.cb,
                6,
                0,
                0,
                0,
                0,
            )?;
        }
        // 返回 sector batch 编码成功。
        Ok(())
    }

    // 执行薄 RHI 的原生扇形 draw packet。
    pub(crate) fn draw_rhi_sector(
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
        // 扇形单位 quad 的顶点 ABI 是 position float2。
        if vertex_stride != (2 * size_of::<f32>()) as u32 {
            return Err(Error::new(
                Errc::InvalidArgument,
                "D3d11 RHI sector stride must be float2",
            ));
        }
        // 非索引和索引绘制必须恰好选择一种范围。
        if (index.is_some() && index_count == 0) || (index.is_none() && vertex_count == 0) {
            return Err(Error::new(
                Errc::InvalidArgument,
                "D3d11 RHI sector draw range is empty",
            ));
        }
        // 绑定固定输入布局、扇形 shader、常量和 premultiplied blend。
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
            context.IASetIndexBuffer(index, DXGI_FORMAT_R32_UINT, 0);
            context.VSSetShader(&self.vs_sector, None);
            context.PSSetShader(&self.ps_sector, None);
            context.VSSetConstantBuffers(0, Some(&[Some(uniform.clone())]));
            context.PSSetConstantBuffers(0, Some(&[Some(uniform.clone())]));
            context.RSSetState(&self.rasterizer);
            context.OMSetBlendState(&self.blend_premultiplied, None, 0xffff_ffff);
            if index.is_some() {
                context.DrawIndexed(index_count, first_index, base_vertex);
            } else {
                context.Draw(vertex_count, first_vertex);
            }
        }
        // 返回编码成功。
        Ok(())
    }
}
