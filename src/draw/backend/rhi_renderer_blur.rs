//! 通用 GPU Renderer 的可分离 blur lowering。

// 引入共享字节载荷，保证两个 blur pass 复用同一份不可变计划数据。
use std::sync::Arc;

// 引入错误、damage 和有限的 RHI 执行类型。
use crate::core::PresentDamage;
use crate::core::error::{Errc, Error, Result};
use crate::native::present::rhi::{
    BufferDesc, BufferHandle, BufferUsage, DrawPacket, GraphicsContextRhi, LoadAction,
    PipelineDesc, RhiColor, RhiExtent, RhiScissor, RhiViewport, SamplerDesc, SamplerHandle,
    TextureDesc, TextureFormat, TextureHandle, pipeline_keys,
};

// 引入父 renderer 已导入的有序 FramePlan 类型和资源缓存。
use super::{FramePlan, FramePlanCommand, RenderPassPlan, RenderTargetRef, RhiRenderer};

// 为 blur 常量生成稳定的 76-float ABI：三组 float4 加 64 个高斯权重。
fn encode_blur_constants(
    extent: RhiExtent,
    region: RhiScissor,
    direction: [f32; 2],
    tap_radius: i32,
    weights: &[f32; 64],
) -> Arc<[u8]> {
    // 复用 D3D11 BlurCB 的 16-byte 对齐布局。
    let mut values = [0.0f32; 76];
    // 保存目标和源纹理的物理尺寸。
    values[0] = extent.width as f32;
    values[1] = extent.height as f32;
    values[2] = extent.width as f32;
    values[3] = extent.height as f32;
    // 保存本次 pass 的源区域。
    values[4] = region.x as f32;
    values[5] = region.y as f32;
    values[6] = region.width as f32;
    values[7] = region.height as f32;
    // 保存像素采样方向和 tap 半径。
    values[8] = direction[0];
    values[9] = direction[1];
    values[10] = tap_radius as f32;
    // 复制最多 64 个已经归一化的高斯权重。
    values[12..].copy_from_slice(weights);
    // 通过通用 renderer 的统一编码入口生成上传载荷。
    RhiRenderer::encode_f32s(&values)
}

// 为当前物理区域生成 blur shader 使用的 NDC 全屏子矩形。
fn blur_region_vertices(extent: RhiExtent, region: RhiScissor) -> Arc<[u8]> {
    // 将左上角坐标转换到 D3D11 的 NDC 横坐标。
    let left = region.x as f32 / extent.width as f32 * 2.0 - 1.0;
    // 将右下边界转换到 D3D11 的 NDC 横坐标。
    let right = (region.x + region.width) as f32 / extent.width as f32 * 2.0 - 1.0;
    // 将顶部坐标转换到左上原点对应的 NDC 纵坐标。
    let top = 1.0 - region.y as f32 / extent.height as f32 * 2.0;
    // 将底部边界转换到左上原点对应的 NDC 纵坐标。
    let bottom = 1.0 - (region.y + region.height) as f32 / extent.height as f32 * 2.0;
    // 以两个三角形覆盖当前区域，顶点 ABI 为 float2。
    let vertices = [
        left, bottom, right, bottom, right, top, left, bottom, right, top, left, top,
    ];
    // 复用 renderer 的 host-endian float 编码。
    RhiRenderer::encode_f32s(&vertices)
}

// 为 RhiRenderer 增加 blur 资源缓存和两阶段执行入口。
impl RhiRenderer {
    // 对已捕获的 overlay backdrop 执行原位双 pass blur。
    pub(crate) fn execute_overlay_backdrop_blur(
        // 复用 renderer 内唯一的 blur 资源缓存。
        &mut self,
        // 通过组合 RHI owner 执行全部资源与提交命令。
        context: &mut dyn GraphicsContextRhi,
        // backdrop texture 同时是第一 pass 的源和最终写回目标。
        backdrop: TextureHandle,
        // 使用创建快照时登记的物理 extent。
        extent: RhiExtent,
        // 区域已经由 backend 从逻辑空间 lower 到物理 scissor。
        region: RhiScissor,
        // 半径保持与 Picture blur 相同的 sigma 语义。
        radius: f32,
    ) -> Result<()> {
        // 复用唯一通用双 pass 实现，避免 overlay 形成平行 shader 路径。
        self.execute_blur_without_present(
            // 传递 owner-thread 组合 context。
            context,
            // 无帧离屏事务不消费最终 present damage。
            PresentDamage::Full,
            // 水平 pass 从已捕获 backdrop 采样。
            backdrop,
            // 两个 pass 都使用快照的完整物理尺寸。
            extent,
            // 只改写调用方要求的物理区域。
            region,
            // 复用统一高斯核构造。
            radius,
            // retained surface 与 backdrop 统一使用预乘 BGRA。
            TextureFormat::Bgra8Unorm,
            // 垂直 pass 原位写回同一 backdrop texture。
            RenderTargetRef::Texture(
                // opaque texture 与 render-target 句柄保持同一资源身份。
                crate::native::present::rhi::RenderTargetHandle::from_raw(backdrop.raw()),
            ),
        )
    }

    // 确保 blur pipeline、区域 quad、uniform 和 sampler 已存在。
    fn ensure_blur_resources(
        &mut self,
        context: &mut dyn GraphicsContextRhi,
    ) -> Result<(
        crate::native::present::rhi::PipelineHandle,
        BufferHandle,
        BufferHandle,
        SamplerHandle,
    )> {
        // 首次使用时创建固定的单方向 blur pipeline。
        let pipeline = if let Some(pipeline) = self.blur_pipeline {
            // 复用已经登记的 pipeline。
            pipeline
        } else {
            // 只选择通用层定义的 blur ABI。
            let pipeline = context.create_pipeline(PipelineDesc {
                key: pipeline_keys::BLUR_PASS,
            })?;
            // 缓存 blur pipeline 句柄。
            self.blur_pipeline = Some(pipeline);
            pipeline
        };
        // 区域 quad 固定使用六个 float2 顶点。
        let vertex_buffer = if let Some(buffer) = self.blur_vertex_buffer {
            // 复用已有区域 vertex buffer。
            buffer
        } else {
            // 创建动态位置 buffer，区域坐标由每次 blur 的物理 region 决定。
            let buffer = context.create_buffer(BufferDesc {
                size_bytes: 6 * 2 * std::mem::size_of::<f32>(),
                stride_bytes: (2 * std::mem::size_of::<f32>()) as u32,
                usage: BufferUsage::Vertex,
            })?;
            // 缓存区域 vertex buffer 句柄。
            self.blur_vertex_buffer = Some(buffer);
            buffer
        };
        // BlurConstants 固定为 76 个 float，即 304 字节。
        let uniform_buffer = if let Some(uniform) = self.blur_uniform {
            // 复用已有 blur uniform。
            uniform
        } else {
            // 按 D3D11 和其他 adapter 的 16-byte cbuffer ABI 创建资源。
            let uniform = context.create_buffer(BufferDesc {
                size_bytes: 76 * std::mem::size_of::<f32>(),
                stride_bytes: 0,
                usage: BufferUsage::Uniform,
            })?;
            // 缓存 blur uniform 句柄。
            self.blur_uniform = Some(uniform);
            uniform
        };
        // 首次使用时创建线性 clamp sampler。
        let sampler = if let Some(sampler) = self.blur_sampler {
            // 复用已有 blur sampler。
            sampler
        } else {
            // 高斯采样需要线性过滤，边界保持 clamp。
            let sampler = context.create_sampler(SamplerDesc { linear: true })?;
            // 缓存 blur sampler 句柄。
            self.blur_sampler = Some(sampler);
            sampler
        };
        // 返回本次两个 pass 共用的固定资源。
        Ok((pipeline, vertex_buffer, uniform_buffer, sampler))
    }

    // 执行两个有序 blur pass，且不触发 surface present。
    pub(crate) fn execute_blur_without_present(
        &mut self,
        context: &mut dyn GraphicsContextRhi,
        damage: PresentDamage,
        source: TextureHandle,
        extent: RhiExtent,
        region: RhiScissor,
        radius: f32,
        format: TextureFormat,
        target: RenderTargetRef,
    ) -> Result<()> {
        // 先拒绝无法形成有效纹理和高斯核的参数。
        if source.raw() == 0 || !extent.is_positive() || !radius.is_finite() || radius < 0.5 {
            // 返回稳定的参数错误，不让 adapter 猜测空 blur。
            return Err(Error::new(
                Errc::InvalidArgument,
                "RHI blur source, extent or radius is invalid",
            ));
        }
        // 将请求区域裁到源纹理范围，避免 NDC 和 scissor 溢出。
        let max_x = extent.width.min(i32::MAX as u32) as i32;
        let max_y = extent.height.min(i32::MAX as u32) as i32;
        let x = region.x.clamp(0, max_x);
        let y = region.y.clamp(0, max_y);
        let width = region.width.clamp(0, max_x.saturating_sub(x));
        let height = region.height.clamp(0, max_y.saturating_sub(y));
        // 完全不可见的区域是有序 no-op，不创建 scratch 资源。
        if width <= 0 || height <= 0 {
            // 不改变任何 target 内容。
            return Ok(());
        }
        // 计算与 legacy blur 相同的 sigma 和最多 63 taps。
        let sigma = radius / 3.0;
        let tap_radius = (sigma * 3.0).ceil().min(31.0) as i32;
        // 小半径不需要进入两个 GPU pass。
        if tap_radius < 1 || !sigma.is_finite() || sigma <= 0.0 {
            // 保持小半径的稳定 no-op 语义。
            return Ok(());
        }
        // 构造归一化的一维高斯核，未使用槽位保持零终止。
        let mut weights = [0.0f32; 64];
        let tap_count = (2 * tap_radius + 1) as usize;
        let mut sum = 0.0f32;
        // 只遍历有效核槽位，并同时保留中心距离所需的索引。
        for (index, weight) in weights.iter_mut().enumerate().take(tap_count) {
            // 计算当前 tap 相对中心的距离。
            let distance = index as f32 - tap_radius as f32;
            // 保存未归一化的高斯权重。
            *weight = (-distance * distance / (2.0 * sigma * sigma)).exp();
            // 累加归一化因子。
            sum += *weight;
        }
        // 极端浮点参数不能继续生成 shader uniform。
        if !sum.is_finite() || sum <= 0.0 {
            // 返回稳定的参数错误而不是上传 NaN。
            return Err(Error::new(
                Errc::InvalidArgument,
                "RHI blur gaussian weights are invalid",
            ));
        }
        // 归一化当前使用的 taps。
        for weight in weights.iter_mut().take(tap_count) {
            // 保持所有 pass 的颜色能量一致。
            *weight /= sum;
        }
        // 准备通用 blur 资源，失败时不创建临时纹理。
        let (pipeline, vertex_buffer, uniform_buffer, sampler) =
            self.ensure_blur_resources(context)?;
        // 创建本次两个 pass 使用的临时颜色 render target。
        let scratch = context.create_texture(TextureDesc { extent, format })?;
        // 将临时 texture 映射为通用 render target 身份。
        let scratch_target = RenderTargetRef::Texture(
            crate::native::present::rhi::RenderTargetHandle::from_raw(scratch.raw()),
        );
        // 将原始区域规整为经过边界验证的整数 scissor。
        let region = RhiScissor {
            x,
            y,
            width,
            height,
        };
        // 两个 pass 都使用完整 target viewport，scissor 限制实际改写区域。
        let viewport = RhiViewport {
            width: extent.width as f32,
            height: extent.height as f32,
        };
        // 区域 quad 的 NDC 顶点确保 shader 在局部区域内生成正确 UV。
        let vertices = blur_region_vertices(extent, region);
        // 水平 pass 读取源 texture，写入 scratch texture。
        let mut horizontal = RenderPassPlan::new(
            scratch_target,
            LoadAction::Clear(RhiColor([0.0, 0.0, 0.0, 0.0])),
        );
        // 设置水平 pass 的完整 viewport。
        horizontal.push(FramePlanCommand::SetViewport(viewport));
        // 限制水平 pass 只覆盖请求区域。
        horizontal.push(FramePlanCommand::SetScissor(Some(region)));
        // 上传当前区域的 NDC 顶点。
        horizontal.push(FramePlanCommand::UpdateBuffer {
            buffer: vertex_buffer,
            offset: 0,
            data: vertices.clone(),
        });
        // 上传水平采样方向和高斯常量。
        horizontal.push(FramePlanCommand::UpdateBuffer {
            buffer: uniform_buffer,
            offset: 0,
            data: encode_blur_constants(extent, region, [1.0, 0.0], tap_radius, &weights),
        });
        // 绑定原始 source texture。
        horizontal.push(FramePlanCommand::BindTexture {
            slot: 0,
            texture: source,
            sampler,
        });
        // 追加固定六顶点 blur draw packet。
        horizontal.push(FramePlanCommand::Draw(DrawPacket {
            pipeline,
            vertex_buffer,
            index_buffer: None,
            uniform_buffer: Some(uniform_buffer),
            vertex_count: 6,
            index_count: 0,
            first_vertex: 0,
            first_index: 0,
            base_vertex: 0,
        }));
        // 垂直 pass 读取 scratch texture，写回原始 target。
        let mut vertical = RenderPassPlan::new(target, LoadAction::Load);
        // 设置垂直 pass 的完整 viewport。
        vertical.push(FramePlanCommand::SetViewport(viewport));
        // 限制垂直 pass 只覆盖请求区域。
        vertical.push(FramePlanCommand::SetScissor(Some(region)));
        // 复用相同的区域顶点。
        vertical.push(FramePlanCommand::UpdateBuffer {
            buffer: vertex_buffer,
            offset: 0,
            data: vertices,
        });
        // 上传垂直采样方向和高斯常量。
        vertical.push(FramePlanCommand::UpdateBuffer {
            buffer: uniform_buffer,
            offset: 0,
            data: encode_blur_constants(extent, region, [0.0, 1.0], tap_radius, &weights),
        });
        // 绑定水平 pass 生成的 scratch texture。
        vertical.push(FramePlanCommand::BindTexture {
            slot: 0,
            texture: scratch,
            sampler,
        });
        // 追加固定六顶点 blur draw packet。
        vertical.push(FramePlanCommand::Draw(DrawPacket {
            pipeline,
            vertex_buffer,
            index_buffer: None,
            uniform_buffer: Some(uniform_buffer),
            vertex_count: 6,
            index_count: 0,
            first_vertex: 0,
            first_index: 0,
            base_vertex: 0,
        }));
        // 以严格顺序组装水平和垂直两个 pass。
        let mut plan = FramePlan::new(context.token(), damage);
        // 保留 source → scratch 的先后关系。
        plan.push_pass(horizontal);
        // 保留 scratch → target 的先后关系。
        plan.push_pass(vertical);
        // 只执行离屏或 no-present surface segment，不提前交换主窗口。
        let execution = super::execute_plan_without_present(context, &plan, target);
        // 两个 pass 结束后立即检查式释放 scratch texture。
        let cleanup = context.destroy_texture(scratch);
        // 优先保留执行失败；清理失败同样不能被当作完整成功。
        match (execution, cleanup) {
            // pass 失败时返回原始执行错误。
            (Err(error), _) => Err(error),
            // pass 成功但 scratch 清理失败时返回资源错误。
            (Ok(()), Err(error)) => Err(error),
            // 两阶段执行和资源清理都成功。
            (Ok(()), Ok(())) => Ok(()),
        }
    }
}

// 将两阶段命令事实与资源生命周期测试拆到独立文件。
#[cfg(test)]
// 测试与本模块共享私有 blur ABI，但不进入生产构建。
#[path = "rhi_renderer_blur_tests.rs"]
// 编译离屏 blur 的 recording-context 回归。
mod tests;
