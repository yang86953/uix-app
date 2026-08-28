//! 通用 GPU Renderer 的圆角与描边矩形 lowering。

// 引入 RHI 计划执行所需的资源描述与句柄。
use crate::platform::presentation::rhi::{
    BufferDesc, BufferHandle, DrawBufferBindings, DrawPacket, DrawRange, DrawRasterState,
    DrawSamplingBinding, GraphicsDevice, LoadAction, PipelineBinding, PipelineDesc, PipelineKind,
    RhiShapeRasterParams, RhiViewport,
};

// 复用 renderer 主模块的无目标 pass 和 shape payload。
use super::{FramePlanCommand, FrameUniformPayload, RhiRenderer, RhiRendererPass, RhiShapeRect};

// 把 renderer 的 Shape 载荷映射为共享 RHI 像素契约值对象。
fn shape_uniform(viewport: RhiViewport, rect: &RhiShapeRect) -> RhiShapeRasterParams {
    // 由共享值对象唯一计算描边外扩、同心 SDF 原点和固定 ABI。
    RhiShapeRasterParams::new(
        // 传入当前物理视口。
        viewport,
        // 传入原始 Shape 左上角和尺寸。
        [rect.x, rect.y, rect.w, rect.h],
        // 传入已经规整的直通颜色。
        rect.rgba,
        // 传入固定四角顺序的圆角半径。
        rect.radius,
        // 传入描边半宽，零表示填充。
        rect.half_stroke,
    )
}

// 为 shape shader 创建或复用单位 quad、pipeline 和常量 buffer。
impl RhiRenderer {
    // 准备固定 position float2 ABI 的 shape 资源。
    pub(super) fn ensure_shape_resources(
        &mut self,
        device: &mut dyn GraphicsDevice,
    ) -> Result<(PipelineBinding, PipelineBinding, BufferHandle, BufferHandle), crate::core::Error>
    {
        // 首次使用时创建 SrcOver 与 Additive 两个固定 shape pipeline。
        let (pipeline, additive_pipeline) =
            if let Some((pipeline, additive_pipeline)) = self.shape_pipeline {
                // 复用已经登记的 shape pipeline。
                (pipeline, additive_pipeline)
            } else {
                // 只选择通用层定义的 Shape pipeline 语义。
                let pipeline = device.create_pipeline(PipelineDesc {
                    kind: PipelineKind::ShapeRect,
                })?;
                // 创建同 ABI 但使用 Additive blend 的 shape pipeline。
                let additive_pipeline = device.create_pipeline(PipelineDesc {
                    kind: PipelineKind::ShapeRectAdditive,
                })?;
                // 缓存两个 shape pipeline 句柄。
                self.shape_pipeline = Some((pipeline, additive_pipeline));
                (pipeline, additive_pipeline)
            };
        // 静态单位 quad 的内容由每个 FramePlan pass 显式上传。
        let unit_vertices = RhiRenderer::unit_quad_vertex_payload();
        // 首次使用时创建单位 quad buffer。
        let vertex_buffer = if let Some(buffer) = self.shape_vertex_buffer {
            // 复用已有 vertex buffer。
            buffer
        } else {
            // 创建位置 float2 ABI 的默认 vertex buffer。
            let buffer = device.create_buffer(BufferDesc::vertex(
                // 保存共享类型化 payload 的精确容量。
                unit_vertices.size_bytes(),
                // 步长只来自共享 Shape 顶点 ABI。
                PipelineKind::ShapeRect.contract().vertex.stride_bytes(),
            ))?;
            // 缓存尚未写入本帧内容的单位 quad 句柄。
            self.shape_vertex_buffer = Some(buffer);
            buffer
        };
        // 首次使用时创建只服从 Shape ABI 的固定 uniform buffer。
        let uniform_buffer = if let Some(uniform) = self.shape_uniform {
            // 复用已有 uniform buffer。
            uniform
        } else {
            // Shape 独立拥有六个 float4，禁止与同尺寸的其它语义管线暗中耦合。
            let uniform = device.create_buffer(BufferDesc::uniform(
                // 使用共享 RHI 常量，禁止 adapter 私自接受旧 Shape 布局。
                PipelineKind::ShapeRect.contract().uniform.size_bytes(),
            ))?;
            // 缓存 shape uniform 句柄。
            self.shape_uniform = Some(uniform);
            uniform
        };
        // 返回 SrcOver、Additive 及共享 buffer 资源。
        Ok((pipeline, additive_pipeline, vertex_buffer, uniform_buffer))
    }

    // 将一个 shape 的固定常量和 draw packet 追加到既有 pass。
    pub(super) fn append_shape_commands(
        pass: &mut RhiRendererPass,
        viewport: RhiViewport,
        rect: &RhiShapeRect,
        pipeline: PipelineBinding,
        vertex_buffer: BufferHandle,
        uniform_buffer: BufferHandle,
    ) {
        // 选择当前矩形的物理裁剪。
        // 上传由共享层完整类型化且保持 16-byte 对齐的 ShapeConstants。
        pass.push(FramePlanCommand::UploadUniform {
            buffer: uniform_buffer,
            // adapter 只接收冻结后的 Shape ABI，不再拥有描边外扩策略。
            data: FrameUniformPayload::Shape(shape_uniform(viewport, rect)),
        });
        // 追加当前矩形的单位 quad draw packet。
        pass.push(FramePlanCommand::Draw(DrawPacket::new(
            pipeline,
            // 绑定当前 draw 的顶点与 uniform 资源。
            DrawBufferBindings::new(vertex_buffer, uniform_buffer),
            // Shape draw 不使用采样资源。
            DrawSamplingBinding::none(),
            // Draw 自有当前 shape 的 viewport 与 scissor 栅格事实。
            DrawRasterState::new(viewport, rect.scissor),
            // 单位 quad 使用封闭的六顶点非索引范围。
            DrawRange::vertices(6),
        )));
    }

    // 执行一帧圆角/描边矩形 RHI 计划。
    pub(crate) fn execute_shape_rects(
        &mut self,
        mut frame: super::RhiRendererFrame<'_>,
        viewport: RhiViewport,
        load: LoadAction,
        rects: &[RhiShapeRect],
    ) -> Result<(), crate::core::Error> {
        // 空列表不应伪造一次 present。
        if rects.is_empty() {
            // 返回稳定的参数错误。
            return Err(super::rhi_invalid(
                "RhiRenderer cannot execute an empty shape list",
            ));
        }
        // 拒绝非有限 viewport，避免计划构造和 adapter 结果分叉。
        if !viewport.is_valid() {
            // 返回稳定的参数错误。
            return Err(super::rhi_invalid("RhiRenderer shape viewport is invalid"));
        }
        // 在创建资源前验证所有矩形的固定 shader 常量（唯一校验权威见 RhiShapeRect::validate）。
        for rect in rects {
            match rect.validate() {
                Ok(()) => {}
                Err(super::RhiShapeRectInvalid::Geometry) => {
                    // 返回稳定的参数错误。
                    return Err(super::rhi_invalid("RhiRenderer shape geometry is invalid"));
                }
                Err(super::RhiShapeRectInvalid::Constants) => {
                    // 不把负半径或负描边交给 shader。
                    return Err(super::rhi_invalid(
                        "RhiRenderer shape constants are invalid",
                    ));
                }
                Err(super::RhiShapeRectInvalid::Scissor) => {
                    // 返回稳定的参数错误。
                    return Err(super::rhi_invalid("RhiRenderer shape scissor is invalid"));
                }
            }
        }
        // 准备 shape pipeline、单位 quad 和常量 buffer。
        let (pipeline, _, vertex_buffer, uniform_buffer) =
            self.ensure_shape_resources(frame.device())?;
        // 创建由封闭帧绑定目标与 load 语义的无目标 pass。
        let mut pass = frame.new_pass();
        // 所有 shape 共享同一个物理 viewport。
        // 在第一个 Shape draw 前建立静态单位 quad 的类型化内容事实。
        pass.push(FramePlanCommand::UploadVertex {
            // 绑定本次 Shape 资源创建的静态 vertex buffer。
            buffer: vertex_buffer,
            // 使用共享 renderer 工厂提供固定 float2 payload。
            data: RhiRenderer::unit_quad_vertex_payload(),
        });
        // 每个矩形只更新常量并保留 painter order。
        for rect in rects {
            // 在 draw 前设置当前矩形的裁剪。
            // 上传共享类型化 ShapeConstants，保持两个 adapter 使用同一绘制边界。
            pass.push(FramePlanCommand::UploadUniform {
                buffer: uniform_buffer,
                // 复用唯一编码入口，避免独立提交与混合计划的 ABI 漂移。
                data: FrameUniformPayload::Shape(shape_uniform(viewport, rect)),
            });
            // 追加单位 quad 的非索引 shape draw packet。
            pass.push(FramePlanCommand::Draw(DrawPacket::new(
                pipeline,
                // 绑定当前 draw 的顶点与 uniform 资源。
                DrawBufferBindings::new(vertex_buffer, uniform_buffer),
                // Shape draw 不使用采样资源。
                DrawSamplingBinding::none(),
                // Draw 自有当前 shape 的 viewport 与 scissor 栅格事实。
                DrawRasterState::new(viewport, rect.scissor),
                // 单位 quad 使用封闭的六顶点非索引范围。
                DrawRange::vertices(6),
            )));
        }
        // 将 shape painter pass 写入封闭帧唯一拥有的计划。
        frame.push_pass(load, pass);
        // surface 计划最终 present，texture 计划只执行离屏 submit。
        frame.execute()?;
        // 资源由 renderer 跨帧复用，不能在这里销毁。
        Ok(())
    }
}
