//! 通用 GPU Renderer 的可分离 blur lowering。

// 引入稳定错误类型。
use crate::core::error::{Errc, Error, Result};
use crate::native::present::rhi::{
    BLUR_WEIGHT_COUNT, BufferDesc, BufferHandle, DrawBufferBindings, DrawPacket, DrawRange,
    GraphicsDevice, LoadAction, PipelineDesc, PipelineKind, RhiBlurRasterParams, RhiColor,
    RhiExtent, RhiScissor, RhiViewport, SampledTextureBinding, SamplerDesc, SamplerHandle,
    TextureDesc, TextureFormat, TextureHandle,
};

// 引入父 renderer 已导入的有序 FramePlan 类型和资源缓存。
use super::{
    FramePlanCommand, FrameUniformPayload, FrameVertexPayload, RenderPassPlan, RenderTargetRef,
    RhiRenderer,
};

// 把 Drawing 的 Blur 事实映射为共享 RHI 常量值对象。
fn blur_uniform(
    // 接收当前 target 与 source 共用的物理尺寸。
    extent: RhiExtent,
    // 接收已经裁到 source 范围的物理区域。
    region: RhiScissor,
    // 接收本次 pass 的水平或垂直像素方向。
    direction: [f32; 2],
    // 接收高斯核中心两侧的 tap 半径。
    tap_radius: u32,
    // 接收由 Drawing Blur Module 计算并归一化的完整权重槽。
    weights: &[f32; BLUR_WEIGHT_COUNT],
) -> RhiBlurRasterParams {
    // 由共享值对象唯一排列目标、source、区域、方向和全部权重。
    RhiBlurRasterParams::new(
        // 当前实现的两个 pass 都写入完整同尺寸 target。
        extent,     // 当前 source 与 target 使用同一物理 extent。
        extent,     // 传入已经裁到纹理范围的物理区域。
        region,     // 传入本次水平或垂直像素方向。
        direction,  // 传入高斯核中心两侧的 tap 半径。
        tap_radius, // 传入已经归一化并零终止的完整权重槽。
        weights,
    )
}

// 为当前物理区域生成 blur shader 使用的 NDC 全屏子矩形。
fn blur_region_vertices(extent: RhiExtent, region: RhiScissor) -> FrameVertexPayload {
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
    // FramePlan 保存类型化 position-float2，而不是过早编码的字节。
    FrameVertexPayload::position_f32x2(vertices)
}

// 为 RhiRenderer 增加 blur 资源缓存和两阶段执行入口。
impl RhiRenderer {
    // 对已捕获的 overlay backdrop 执行原位双 pass blur。
    pub(crate) fn execute_overlay_backdrop_blur(
        // 复用 renderer 内唯一的 blur 资源缓存。
        &mut self,
        // 只借用资源、命令与 submit 所需的 Device 角色。
        device: &mut dyn GraphicsDevice,
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
            // 传递 owner-thread Device 角色。
            device,
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
            // 最终写回目标保留同一类型化 texture 身份。
            backdrop,
        )
    }

    // 确保 blur pipeline、区域 quad、uniform 和 sampler 已存在。
    fn ensure_blur_resources(
        &mut self,
        device: &mut dyn GraphicsDevice,
    ) -> Result<(
        crate::native::present::rhi::PipelineBinding,
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
            let pipeline = device.create_pipeline(PipelineDesc {
                kind: PipelineKind::BlurPass,
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
            let buffer = device.create_buffer(BufferDesc::vertex(
                // 保存六个 float2 顶点的精确容量。
                6 * 2 * std::mem::size_of::<f32>(),
                // 步长只来自共享 Blur 顶点 ABI。
                PipelineKind::BlurPass.contract().vertex.stride_bytes(),
            ))?;
            // 缓存区域 vertex buffer 句柄。
            self.blur_vertex_buffer = Some(buffer);
            buffer
        };
        // BlurConstants 容量完全服从共享 RHI 值对象。
        let uniform_buffer = if let Some(uniform) = self.blur_uniform {
            // 复用已有 blur uniform。
            uniform
        } else {
            // 按 D3D11 和其他 adapter 的 16-byte cbuffer ABI 创建资源。
            let uniform = device.create_buffer(BufferDesc::uniform(
                // 常量容量只来自共享 Blur Uniform ABI。
                PipelineKind::BlurPass.contract().uniform.size_bytes(),
            ))?;
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
            let sampler = device.create_sampler(SamplerDesc::linear_clamp())?;
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
        device: &mut dyn GraphicsDevice,
        source: TextureHandle,
        extent: RhiExtent,
        region: RhiScissor,
        radius: f32,
        format: TextureFormat,
        target: TextureHandle,
    ) -> Result<()> {
        // 先拒绝无法形成有效纹理和高斯核的参数。
        if source.raw() == 0 || !extent.is_valid() || !radius.is_finite() || radius < 0.5 {
            // 返回稳定的参数错误，不让 adapter 猜测空 blur。
            return Err(Error::new(
                Errc::InvalidArgument,
                "RHI blur source, extent or radius is invalid",
            ));
        }
        // 读取共享几何 Component 已验证的原生裁剪边界。
        let (max_x, max_y) = extent
            // blur 不得在 Drawing 层饱和修正目标尺寸。
            .native_size_i32()
            // 理论上已由入口 is_valid 证明，仍保留稳定错误。
            .ok_or_else(|| super::rhi_invalid("RHI blur extent is outside native domain"))?;
        // 将请求区域裁到源纹理范围，避免 NDC 和 scissor 溢出。
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
        let mut weights = [0.0f32; BLUR_WEIGHT_COUNT];
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
            self.ensure_blur_resources(device)?;
        // 创建本次两个 pass 使用的临时颜色 render target。
        let scratch = device.create_texture(TextureDesc::new(extent, format))?;
        // 将临时 texture 映射为通用 render target 身份。
        let scratch_target = RenderTargetRef::Texture(scratch);
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
        let mut horizontal =
            RenderPassPlan::new(scratch_target, LoadAction::Clear(RhiColor::transparent()));
        // 设置水平 pass 的完整 viewport。
        horizontal.push(FramePlanCommand::SetViewport(viewport));
        // 限制水平 pass 只覆盖请求区域。
        horizontal.push(FramePlanCommand::SetScissor(Some(region)));
        // 上传当前区域的类型化 NDC 顶点。
        horizontal.push(FramePlanCommand::UploadVertex {
            buffer: vertex_buffer,
            data: vertices.clone(),
        });
        // 上传水平采样方向和类型化高斯常量。
        horizontal.push(FramePlanCommand::UploadUniform {
            buffer: uniform_buffer,
            // 共享 RHI 值对象拥有完整 Blur 字段排列。
            data: FrameUniformPayload::Blur(blur_uniform(
                extent,
                region,
                [1.0, 0.0],
                tap_radius as u32,
                &weights,
            )),
        });
        // 原子绑定原始 source texture 与共享 sampler。
        horizontal.push(FramePlanCommand::BindSampledTexture(
            // 固定 t0/s0 ABI 不向 blur lowering 暴露槽位。
            SampledTextureBinding::for_pipeline(
                // 传递水平 blur 的源纹理身份。
                source, // 传递共享 blur 采样器身份。
                sampler,
                // 水平 blur 绑定与后续 DrawPacket 使用同一个 pipeline。
                pipeline,
            ),
        ));
        // 追加固定六顶点 blur draw packet。
        horizontal.push(FramePlanCommand::Draw(DrawPacket::new(
            pipeline,
            DrawBufferBindings::new(vertex_buffer, uniform_buffer),
            // Blur quad 使用封闭的六顶点非索引范围。
            DrawRange::vertices(6),
        )));
        // 垂直 pass 读取 scratch texture，写回原始 target。
        let mut vertical = RenderPassPlan::new(RenderTargetRef::Texture(target), LoadAction::Load);
        // 设置垂直 pass 的完整 viewport。
        vertical.push(FramePlanCommand::SetViewport(viewport));
        // 限制垂直 pass 只覆盖请求区域。
        vertical.push(FramePlanCommand::SetScissor(Some(region)));
        // 复用相同的类型化区域顶点。
        vertical.push(FramePlanCommand::UploadVertex {
            buffer: vertex_buffer,
            data: vertices,
        });
        // 上传垂直采样方向和类型化高斯常量。
        vertical.push(FramePlanCommand::UploadUniform {
            buffer: uniform_buffer,
            // 垂直 pass 只改变方向，其余共享字段与高斯核保持相同。
            data: FrameUniformPayload::Blur(blur_uniform(
                extent,
                region,
                [0.0, 1.0],
                tap_radius as u32,
                &weights,
            )),
        });
        // 原子绑定水平 pass 生成的 scratch texture 与共享 sampler。
        vertical.push(FramePlanCommand::BindSampledTexture(
            // 垂直 pass 复用同一个完整采样绑定类型。
            SampledTextureBinding::for_pipeline(
                // 传递垂直 blur 的 scratch 纹理身份。
                scratch, // 传递共享 blur 采样器身份。
                sampler,
                // 垂直 blur 绑定与后续 DrawPacket 使用同一个 pipeline。
                pipeline,
            ),
        ));
        // 追加固定六顶点 blur draw packet。
        vertical.push(FramePlanCommand::Draw(DrawPacket::new(
            pipeline,
            DrawBufferBindings::new(vertex_buffer, uniform_buffer),
            // Blur quad 使用封闭的六顶点非索引范围。
            DrawRange::vertices(6),
        )));
        // 以严格顺序组装水平和垂直两个 pass。
        let mut plan = super::FramePlan::offscreen();
        // 保留 source → scratch 的先后关系。
        plan.push_pass(horizontal);
        // 保留 scratch → target 的先后关系。
        plan.push_pass(vertical);
        // 只执行离屏或 no-present surface segment，不提前交换主窗口。
        let execution = super::execute_plan_without_present(device, &plan);
        // 两个 pass 结束后立即检查式释放 scratch texture。
        let cleanup = device.destroy_texture(scratch);
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
