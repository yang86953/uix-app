//! 通用 GPU Renderer 的仿射阴影 lowering。

// 引入 RHI 计划执行所需的资源描述与句柄。
use crate::platform::presentation::rhi::{
    BufferDesc, DrawBufferBindings, DrawPacket, DrawRange, DrawRasterState, DrawSamplingBinding,
    GraphicsDevice, LoadAction, PipelineBinding, PipelineDesc, PipelineKind, RhiShadowRasterParams,
    RhiViewport,
};

// 复用 renderer 主模块的无目标 pass 和 shape 单位 quad 资源。
use super::{BufferHandle, FramePlanCommand, FrameUniformPayload, RhiRenderer, RhiRendererPass};

// 保存一个已经完成几何 lowering 的仿射阴影。
#[derive(Debug, Clone, Copy)]
pub(crate) struct RhiShadow {
    // 保存阴影本体物理宽度。
    pub(crate) w: f32,
    // 保存阴影本体物理高度。
    pub(crate) h: f32,
    // 保存包含 blur 与 offset 的设备四边形，顺序为 TL/TR/BR/BL。
    pub(crate) corners: [[f32; 2]; 4],
    // 保存 x 方向 blur 物理量。
    pub(crate) blur_x: f32,
    // 保存 y 方向 blur 物理量。
    pub(crate) blur_y: f32,
    // 保存阴影直通颜色。
    pub(crate) rgba: [f32; 4],
    // 保存阴影本体圆角半径。
    pub(crate) radius: [f32; 4],
    // 保存是否使用 ambient 曲线。
    pub(crate) ambient: bool,
    // 保存当前阴影的物理裁剪矩形。
    pub(crate) scissor: Option<super::RhiScissor>,
}

// 把 renderer 的 Shadow 载荷映射为共享 RHI 像素契约值对象。
fn shadow_uniform(viewport: RhiViewport, shadow: &RhiShadow) -> RhiShadowRasterParams {
    // 由共享值对象唯一派生两条仿射边、环境标记和固定 ABI。
    RhiShadowRasterParams::new(
        // 传入当前物理视口。
        viewport,
        // 传入已经包含 offset 与 blur 的设备四角。
        shadow.corners,
        // 传入已经规整的直通颜色。
        shadow.rgba,
        // 传入固定四角顺序的本体圆角。
        shadow.radius,
        // 传入两个物理轴上的模糊量。
        [shadow.blur_x, shadow.blur_y],
        // 传入不含模糊扩展的本体尺寸。
        [shadow.w, shadow.h],
        // 传入覆盖曲线身份。
        shadow.ambient,
    )
}

// 校验阴影四边形是有限、非退化且保持凸顶点顺序的仿射四边形。
fn valid_shadow_corners(corners: &[[f32; 2]; 4]) -> bool {
    // 顶点坐标必须全部可表示，避免 shader 收到未定义值。
    if corners.iter().flatten().any(|value| !value.is_finite()) {
        // 非有限坐标不能进入 draw ABI。
        return false;
    }
    // 逐条相邻边计算叉积，要求四个角保持同一绕向。
    let mut winding = 0.0f32;
    // 四边形必须由四个连续的非零转角组成。
    for index in 0..4 {
        // 读取当前顶点及其后的两个顶点。
        let a = corners[index];
        let b = corners[(index + 1) % 4];
        let c = corners[(index + 2) % 4];
        // 计算连续两条边的二维叉积。
        let cross = (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]);
        // 退化或溢出的转角必须拒绝。
        if !cross.is_finite() || cross.abs() <= 1e-5 {
            // 将异常阴影交回兼容路径。
            return false;
        }
        // 第一个转角建立期望绕向，后续转角必须一致。
        if winding == 0.0 {
            winding = cross.signum();
        } else if cross.signum() != winding {
            // 自交或凹四边形不能由单位 quad 稳定表示。
            return false;
        }
    }
    // 通过全部几何门禁。
    true
}

// 为阴影 shader 创建或复用独立 pipeline、顶点与常量资源。
impl RhiRenderer {
    // 准备 ShadowConstants 所需的独立 pipeline、单位 quad 和 uniform。
    pub(super) fn ensure_shadow_resources(
        &mut self,
        device: &mut dyn GraphicsDevice,
    ) -> Result<(PipelineBinding, BufferHandle, BufferHandle), crate::core::Error> {
        // 首次使用时创建固定的阴影 pipeline。
        let pipeline = if let Some(pipeline) = self.shadow_pipeline {
            // 复用已经登记的阴影 pipeline。
            pipeline
        } else {
            // 选择只由 RHI 契约定义的阴影 pipeline 语义。
            let pipeline = device.create_pipeline(PipelineDesc {
                kind: PipelineKind::BoxShadow,
            })?;
            // 缓存阴影 pipeline 句柄。
            self.shadow_pipeline = Some(pipeline);
            pipeline
        };
        // 静态单位 quad 的内容由每个 FramePlan pass 显式上传。
        let unit_vertices = RhiRenderer::unit_quad_vertex_payload();
        // 首次使用时创建 Shadow 自己的单位 quad buffer。
        let vertex_buffer = if let Some(buffer) = self.shadow_vertex_buffer {
            // 复用已经登记的 Shadow vertex buffer。
            buffer
        } else {
            // 按共享 pipeline 顶点布局创建精确容量。
            let buffer = device.create_buffer(BufferDesc::vertex(
                // 保存共享类型化 payload 的精确容量。
                unit_vertices.size_bytes(),
                // 使用 BoxShadow 契约声明的唯一顶点步长。
                PipelineKind::BoxShadow.contract().vertex.stride_bytes(),
            ))?;
            // 缓存尚未写入本帧内容的 Shadow vertex buffer 句柄。
            self.shadow_vertex_buffer = Some(buffer);
            // 返回刚创建的顶点资源。
            buffer
        };
        // 首次使用时创建只服从 Shadow ABI 的 uniform buffer。
        let uniform_buffer = if let Some(uniform) = self.shadow_uniform {
            // 复用已经登记的 Shadow uniform buffer。
            uniform
        } else {
            // 使用 BoxShadow 契约声明的精确常量容量。
            let uniform = device.create_buffer(BufferDesc::uniform(
                // 禁止借用 Shape 的同尺寸常量资源形成隐式耦合。
                PipelineKind::BoxShadow.contract().uniform.size_bytes(),
            ))?;
            // 缓存 Shadow uniform buffer 句柄。
            self.shadow_uniform = Some(uniform);
            // 返回刚创建的常量资源。
            uniform
        };
        // 返回阴影 draw 所需的固定资源。
        Ok((pipeline, vertex_buffer, uniform_buffer))
    }

    // 将一个 Shadow 的固定常量和 draw packet 追加到既有 pass。
    pub(super) fn append_shadow_commands(
        pass: &mut RhiRendererPass,
        viewport: RhiViewport,
        shadow: &RhiShadow,
        pipeline: PipelineBinding,
        vertex_buffer: BufferHandle,
        uniform_buffer: BufferHandle,
    ) {
        // 选择当前阴影的物理裁剪。
        // 上传由共享层完整类型化且保持 16-byte 对齐的 ShadowConstants。
        pass.push(FramePlanCommand::UploadUniform {
            // 选择 Shadow 自己的 uniform 资源。
            buffer: uniform_buffer,
            // Adapter 只接收冻结 ABI，不再拥有仿射边和环境标记公式。
            data: FrameUniformPayload::Shadow(shadow_uniform(viewport, shadow)),
        });
        // 追加当前阴影的单位 quad draw packet。
        pass.push(FramePlanCommand::Draw(DrawPacket::new(
            // 选择固定 BoxShadow pipeline。
            pipeline,
            // 绑定 Shadow 自己的单位 quad 与当前常量资源。
            DrawBufferBindings::new(vertex_buffer, uniform_buffer),
            // Shadow quad 不使用采样纹理。
            DrawSamplingBinding::none(),
            // Draw 自有当前 shadow 的 viewport 与 scissor 栅格事实。
            DrawRasterState::new(viewport, shadow.scissor),
            // 单位 quad 使用封闭的六顶点非索引范围。
            DrawRange::vertices(6),
        )));
    }

    // 执行一帧仿射阴影 RHI 计划。
    pub(crate) fn execute_shadows(
        &mut self,
        mut frame: super::RhiRendererFrame<'_>,
        viewport: RhiViewport,
        load: LoadAction,
        shadows: &[RhiShadow],
    ) -> Result<(), crate::core::Error> {
        // 空列表不应伪造一次 present。
        if shadows.is_empty() {
            // 返回稳定的参数错误。
            return Err(super::rhi_invalid(
                "RhiRenderer cannot execute an empty shadow list",
            ));
        }
        // 拒绝非有限 viewport，避免计划构造和 adapter 结果分叉。
        if !viewport.is_valid() {
            // 返回稳定的参数错误。
            return Err(super::rhi_invalid("RhiRenderer shadow viewport is invalid"));
        }
        // 在创建资源前验证所有阴影的固定 shader 常量。
        for shadow in shadows {
            // 阴影本体尺寸必须是有限正值。
            if !shadow.w.is_finite() || !shadow.h.is_finite() || shadow.w <= 0.0 || shadow.h <= 0.0
            {
                // 返回稳定的参数错误。
                return Err(super::rhi_invalid("RhiRenderer shadow geometry is invalid"));
            }
            // 阴影四边形、blur、颜色和圆角必须有限，blur/圆角不能为负。
            if !shadow.blur_x.is_finite()
                || !shadow.blur_y.is_finite()
                || shadow.blur_x < 0.0
                || shadow.blur_y < 0.0
                || shadow.rgba.iter().any(|value| !value.is_finite())
                || shadow
                    .radius
                    .iter()
                    .any(|value| !value.is_finite() || *value < 0.0)
                || !valid_shadow_corners(&shadow.corners)
            {
                // 返回稳定的参数错误。
                return Err(super::rhi_invalid(
                    "RhiRenderer shadow constants are invalid",
                ));
            }
            // 显式 scissor 必须已经完成物理坐标 lowering。
            if shadow.scissor.is_some_and(|scissor| !scissor.is_valid()) {
                // 返回稳定的参数错误。
                return Err(super::rhi_invalid("RhiRenderer shadow scissor is invalid"));
            }
        }
        // 准备阴影 pipeline、单位 quad 和常量 buffer。
        let (pipeline, vertex_buffer, uniform_buffer) =
            self.ensure_shadow_resources(frame.device())?;
        // 创建由封闭帧绑定目标与 load 语义的无目标 pass。
        let mut pass = frame.new_pass();
        // 所有阴影共享同一个物理 viewport。
        // 在第一个 Shadow draw 前建立静态单位 quad 的类型化内容事实。
        pass.push(FramePlanCommand::UploadVertex {
            // 绑定本次 Shadow 资源创建的静态 vertex buffer。
            buffer: vertex_buffer,
            // 使用共享 renderer 工厂提供固定 float2 payload。
            data: RhiRenderer::unit_quad_vertex_payload(),
        });
        // 每个阴影只更新常量并保留 painter order。
        for shadow in shadows {
            // 复用独立与混合路径共享的唯一 Shadow command lowering。
            RhiRenderer::append_shadow_commands(
                // 追加到当前 surface 或 retained texture pass。
                &mut pass,
                // 传入当前物理视口。
                viewport,
                // 传入已经验证的 Shadow 载荷。
                shadow,
                // 传入固定 BoxShadow pipeline。
                pipeline,
                // 传入 Shadow 自己的单位 quad。
                vertex_buffer,
                // 传入 Shadow 自己的常量资源。
                uniform_buffer,
            );
        }
        // 将阴影 painter pass 写入封闭帧唯一拥有的计划。
        frame.push_pass(load, pass);
        // surface 计划最终 present，texture 计划只执行离屏 submit。
        frame.execute()?;
        // 资源由 renderer 跨帧复用，不能在这里销毁。
        Ok(())
    }
}
