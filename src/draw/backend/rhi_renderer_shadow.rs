//! 通用 GPU Renderer 的仿射阴影 lowering。

// 引入最终 damage。
use crate::core::PresentDamage;
// 引入 RHI 计划执行所需的资源描述与句柄。
use crate::native::present::rhi::{
    pipeline_keys, DrawPacket, GraphicsContextRhi, LoadAction, PipelineDesc, PipelineHandle,
    RhiViewport,
};

// 复用 renderer 主模块的计划类型和 shape 单位 quad 资源。
use super::{
    BufferHandle, FramePlan, FramePlanCommand, RenderPassPlan, RenderTargetRef, RhiRenderer,
};

// 保存一个已经完成几何 lowering 的仿射阴影。
#[derive(Debug, Clone, Copy)]
pub(crate) struct RhiShadow {
    // 保存阴影本体左上角物理坐标。
    pub(crate) x: f32,
    // 保存阴影本体左上角物理坐标。
    pub(crate) y: f32,
    // 保存阴影本体物理宽度。
    pub(crate) w: f32,
    // 保存阴影本体物理高度。
    pub(crate) h: f32,
    // 保存包含 blur 与 offset 的设备四边形，顺序为 TL/TR/BR/BL。
    pub(crate) corners: [[f32; 2]; 4],
    // 保存阴影偏移物理量。
    pub(crate) offset_x: f32,
    // 保存阴影偏移物理量。
    pub(crate) offset_y: f32,
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

// 为阴影 shader 创建或复用 pipeline 与 shape ABI 资源。
impl RhiRenderer {
    // 准备 ShadowConstants 所需的 pipeline、单位 quad 和 uniform。
    pub(super) fn ensure_shadow_resources(
        &mut self,
        context: &mut dyn GraphicsContextRhi,
    ) -> Result<(PipelineHandle, BufferHandle, BufferHandle), crate::core::Error> {
        // 复用 shape 的 float2 单位 quad 和 96 字节 uniform 资源。
        let (_, _, vertex_buffer, uniform_buffer) = self.ensure_shape_resources(context)?;
        // 首次使用时创建固定的阴影 pipeline。
        let pipeline = if let Some(pipeline) = self.shadow_pipeline {
            // 复用已经登记的阴影 pipeline。
            pipeline
        } else {
            // 选择只由 RHI 契约定义的阴影 pipeline key。
            let pipeline = context.create_pipeline(PipelineDesc {
                key: pipeline_keys::BOX_SHADOW,
            })?;
            // 缓存阴影 pipeline 句柄。
            self.shadow_pipeline = Some(pipeline);
            pipeline
        };
        // 返回阴影 draw 所需的固定资源。
        Ok((pipeline, vertex_buffer, uniform_buffer))
    }

    // 执行一帧仿射阴影 RHI 计划。
    pub(crate) fn execute_shadows(
        &mut self,
        context: &mut dyn GraphicsContextRhi,
        damage: PresentDamage,
        viewport: RhiViewport,
        load: LoadAction,
        target: RenderTargetRef,
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
            // 阴影本体几何必须是有限正值。
            if !shadow.x.is_finite()
                || !shadow.y.is_finite()
                || !shadow.w.is_finite()
                || !shadow.h.is_finite()
                || shadow.w <= 0.0
                || shadow.h <= 0.0
            {
                // 返回稳定的参数错误。
                return Err(super::rhi_invalid("RhiRenderer shadow geometry is invalid"));
            }
            // 阴影四边形、偏移、blur、颜色和圆角必须有限，blur/圆角不能为负。
            if !shadow.offset_x.is_finite()
                || !shadow.offset_y.is_finite()
                || !shadow.blur_x.is_finite()
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
        let (pipeline, vertex_buffer, uniform_buffer) = self.ensure_shadow_resources(context)?;
        // 计划使用当前 context 的 surface 代际。
        let surface = context.token();
        // 创建 surface pass，并保留调用方的 load/clear 语义。
        let mut pass = RenderPassPlan::new(target, load);
        // 所有阴影共享同一个物理 viewport。
        pass.push(FramePlanCommand::SetViewport(viewport));
        // 每个阴影只更新常量并保留 painter order。
        for shadow in shadows {
            // 阴影 shader 使用两个方向 blur 的物理值，并以较大值控制软边曲线。
            // 在 draw 前设置当前阴影的裁剪。
            pass.push(FramePlanCommand::SetScissor(shadow.scissor));
            // 上传完整 AffineShadowConstants，保持 D3D11 16-byte 对齐布局。
            pass.push(FramePlanCommand::UpdateBuffer {
                buffer: uniform_buffer,
                offset: 0,
                data: RhiRenderer::encode_f32s(&[
                    viewport.width,
                    viewport.height,
                    0.0,
                    0.0,
                    shadow.corners[0][0],
                    shadow.corners[0][1],
                    shadow.corners[1][0] - shadow.corners[0][0],
                    shadow.corners[1][1] - shadow.corners[0][1],
                    shadow.rgba[0],
                    shadow.rgba[1],
                    shadow.rgba[2],
                    shadow.rgba[3],
                    shadow.radius[0],
                    shadow.radius[1],
                    shadow.radius[2],
                    shadow.radius[3],
                    shadow.corners[3][0] - shadow.corners[0][0],
                    shadow.corners[3][1] - shadow.corners[0][1],
                    shadow.blur_x,
                    shadow.blur_y,
                    shadow.w,
                    shadow.h,
                    if shadow.ambient { 1.0 } else { 0.0 },
                    0.0,
                ]),
            });
            // 追加单位 quad 的非索引阴影 draw packet。
            pass.push(FramePlanCommand::Draw(DrawPacket {
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
        }
        // 创建计划并追加唯一 surface pass。
        let mut plan = FramePlan::new(surface, damage);
        // 保留阴影 painter order。
        plan.push_pass(pass);
        // surface 计划最终 present，texture 计划只执行离屏 submit。
        super::execute_plan_for_target(context, &plan, target)?;
        // 资源由 renderer 跨帧复用，不能在这里销毁。
        Ok(())
    }
}

// 为仿射 shadow 几何门禁保留最小单元覆盖。
#[cfg(test)]
mod tests {
    // 引入当前模块的几何校验辅助。
    use super::valid_shadow_corners;

    // 旋转后的平行四边形必须进入 RHI shadow lowering。
    #[test]
    fn accepts_affine_parallelogram() {
        // 构造一个带旋转/剪切的四角载荷。
        let corners = [[10.0, 20.0], [90.0, 60.0], [70.0, 140.0], [-10.0, 100.0]];
        // 确认单位 quad 可以稳定覆盖该四边形。
        assert!(valid_shadow_corners(&corners));
    }

    // 自交或凹四边形必须被门禁拒绝。
    #[test]
    fn rejects_non_convex_shadow_quad() {
        // 构造一个凹四边形，避免 shader 产生不确定覆盖。
        let corners = [[10.0, 20.0], [90.0, 20.0], [40.0, 40.0], [10.0, 80.0]];
        // 确认异常几何不会进入 RHI draw。
        assert!(!valid_shadow_corners(&corners));
    }
}
