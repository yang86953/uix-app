//! 通用 GPU Renderer 的可分离 blur lowering。

// 引入稳定错误类型。
use crate::core::error::{Errc, Error, Result};
use crate::platform::presentation::rhi::{
    BLUR_WEIGHT_COUNT, BufferDesc, BufferHandle, DrawBufferBindings, DrawPacket, DrawRange,
    DrawRasterState, DrawSamplingBinding, GraphicsDevice, LoadAction, PipelineDesc, PipelineKind,
    RhiBlurDirection, RhiBlurPassGeometry, RhiBlurRasterParams, RhiColor, RhiExtent, RhiScissor,
    RhiTextureRegion, RhiViewport, SampledTextureBinding, SamplerDesc, SamplerHandle, TextureDesc,
    TextureFormat, TextureHandle,
};

// 引入父 renderer 已导出的目标无关命令与唯一 Frame owner。
use super::{
    FramePlanCommand, FrameUniformPayload, FrameVertexPayload, RhiRenderer, RhiRendererFrame,
};

// 把 Drawing 的 Blur 事实映射为共享 RHI 常量值对象。
fn blur_uniform(
    // 接收已经绑定 source/target extent 与两个区域的共享几何。
    geometry: RhiBlurPassGeometry,
    // 接收本次 pass 的封闭水平或垂直方向。
    direction: RhiBlurDirection,
    // 接收高斯核中心两侧的 tap 半径。
    tap_radius: u32,
    // 接收由 Drawing Blur Module 计算并归一化的完整权重槽。
    weights: &[f32; BLUR_WEIGHT_COUNT],
) -> RhiBlurRasterParams {
    // 由共享几何唯一排列源域边界、规范 texel step、tap 半径和权重。
    geometry.raster_params(direction, tap_radius, weights)
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
        crate::platform::presentation::rhi::PipelineBinding,
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
        // 区域 quad 固定使用六个 position-float2/uv-float2 顶点。
        let vertex_buffer = if let Some(buffer) = self.blur_vertex_buffer {
            // 复用已有区域 vertex buffer。
            buffer
        } else {
            // 创建动态 position/uv buffer，所有坐标只由共享 Blur 几何决定。
            let buffer = device.create_buffer(BufferDesc::vertex(
                // 保存六个 float4 顶点的精确容量。
                6 * 4 * std::mem::size_of::<f32>(),
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
            // 按所有 adapter 共用的 16-byte 常量缓冲 ABI 创建资源。
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
        // 把裁剪结果收敛为源与目标共用的非空物理区域。
        let physical_region = RhiTextureRegion::from_xy(
            x as u32,
            y as u32,
            RhiExtent::new(width as u32, height as u32),
        );
        // 唯一共享门禁绑定 source extent/region 与 target extent/region。
        let geometry = RhiBlurPassGeometry::new(extent, physical_region, extent, physical_region)?;
        // scissor 只从已验证目标区域机械投影。
        let region = geometry.destination_scissor();
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
        // 从 scratch 创建成功起由同一 Frame 持有计划，并在末尾统一检查式清理。
        let mut frame = RhiRendererFrame::offscreen(device, target);
        // 两个 pass 都使用完整 target viewport，scissor 限制实际改写区域。
        let viewport = RhiViewport {
            width: extent.width as f32,
            height: extent.height as f32,
        };
        // 共享几何一次生成最终 NDC position 与绝对 source UV，shader 不再解释 region。
        let vertices = FrameVertexPayload::position_uv_f32(geometry.vertex_values());
        // 水平命令包只描述 source 采样，不自行绑定 scratch target。
        let mut horizontal = frame.new_pass();
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
                geometry,
                RhiBlurDirection::Horizontal,
                tap_radius as u32,
                &weights,
            )),
        });
        // 原子绑定原始 source texture 与共享 sampler。
        // 追加固定六顶点 blur draw packet。
        horizontal.push(FramePlanCommand::Draw(DrawPacket::new(
            pipeline,
            DrawBufferBindings::new(vertex_buffer, uniform_buffer),
            // DrawPacket 直接拥有水平 blur 的采样绑定。
            DrawSamplingBinding::sampled(SampledTextureBinding::for_pipeline(
                source, sampler, pipeline,
            )),
            // Horizontal blur 固化完整 viewport 与区域 scissor。
            DrawRasterState::new(viewport, Some(region)),
            // Blur quad 使用封闭的六顶点非索引范围。
            DrawRange::vertices(6),
        )));
        // 垂直命令包只描述 scratch 采样，不自行绑定最终 target。
        let mut vertical = frame.new_pass();
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
                geometry,
                RhiBlurDirection::Vertical,
                tap_radius as u32,
                &weights,
            )),
        });
        // 原子绑定水平 pass 生成的 scratch texture 与共享 sampler。
        // 追加固定六顶点 blur draw packet。
        vertical.push(FramePlanCommand::Draw(DrawPacket::new(
            pipeline,
            DrawBufferBindings::new(vertex_buffer, uniform_buffer),
            // DrawPacket 直接拥有垂直 blur 的采样绑定。
            DrawSamplingBinding::sampled(SampledTextureBinding::for_pipeline(
                scratch, sampler, pipeline,
            )),
            // Vertical blur 固化完整 viewport 与区域 scissor。
            DrawRasterState::new(viewport, Some(region)),
            // Blur quad 使用封闭的六顶点非索引范围。
            DrawRange::vertices(6),
        )));
        // 先由 Offscreen Frame owner 绑定水平 pass 的 scratch 目标。
        let binding = frame.push_offscreen_pass(
            scratch,
            LoadAction::Clear(RhiColor::transparent()),
            horizontal,
        );
        // 绑定失败同样属于必须先保留、再执行 scratch 清理的主阶段错误。
        let execution = match binding {
            // 角色门禁成功后绑定最终目标并一次性执行完整双 pass 计划。
            Ok(()) => {
                // 垂直 pass 使用 Offscreen Frame 构造时冻结的最终目标。
                frame.push_pass(LoadAction::Load, vertical);
                // 唯一 Frame 门面执行一个计划、一次 submit 且不触发 present。
                frame.execute()
            }
            // 不使用提前返回，确保 scratch 仍进入 checked cleanup。
            Err(error) => Err(error),
        };
        // 两个 pass 结束后立即检查式释放 scratch texture。
        let cleanup = frame.device().destroy_texture(scratch);
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
