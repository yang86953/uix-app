//! 通用 GPU Renderer 的线性/径向渐变 RHI lowering。

// 引入共享错误结果类型。
use crate::core::error::Result;
// 引入薄 RHI 的设备、命令和 pass 类型。
use crate::native::present::rhi::{DrawPacket, LoadAction, RhiGradientRasterParams, RhiViewport};

// 引入父 renderer 的帧计划、target 和渐变载荷。
use super::{FramePlanCommand, FrameUniformPayload, RenderPassPlan, RhiGradientRect, RhiRenderer};

// 检查渐变 quad 是否保持有限、凸且非退化。
pub(super) fn valid_gradient_corners(corners: &[[f32; 2]; 4]) -> bool {
    // 顶点必须可表示，避免插值阶段得到未定义坐标。
    if corners.iter().flatten().any(|value| !value.is_finite()) {
        return false;
    }
    // 固定 TL/TR/BR/BL 顺序，四个转角必须保持同一绕向。
    let mut winding = 0.0f32;
    for index in 0..4 {
        // 读取连续的三个角点。
        let a = corners[index];
        let b = corners[(index + 1) % 4];
        let c = corners[(index + 2) % 4];
        // 计算连续边叉积。
        let cross = (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]);
        // 退化或混绕 quad 不能稳定执行。
        if !cross.is_finite() || cross.abs() <= 1e-5 {
            return false;
        }
        // 保存第一个转角的方向，后续必须一致。
        if winding == 0.0 {
            winding = cross.signum();
        } else if cross.signum() != winding {
            return false;
        }
    }
    true
}

// 为 RhiRenderer 提供统一的渐变计划执行入口。
impl RhiRenderer {
    // 构造并执行一帧线性/径向渐变 RHI 计划。
    pub(crate) fn execute_gradients(
        &mut self,
        mut frame: super::RhiRendererFrame<'_>,
        viewport: RhiViewport,
        load: LoadAction,
        gradients: &[RhiGradientRect],
    ) -> Result<()> {
        // 空列表不应伪造一次 present。
        if gradients.is_empty() {
            // 返回稳定的参数错误。
            return Err(super::rhi_invalid(
                "RhiRenderer cannot execute an empty gradient list",
            ));
        }
        // 拒绝非有限 viewport，避免计划构造和 adapter 结果分叉。
        if !viewport.is_valid() {
            // 返回稳定的参数错误。
            return Err(super::rhi_invalid(
                "RhiRenderer gradient viewport is invalid",
            ));
        }
        // 先验证所有渐变的有限性和固定 shader 参数范围。
        for gradient in gradients {
            // 渐变 AABB 必须是有限正矩形。
            if !gradient.x.is_finite()
                || !gradient.y.is_finite()
                || !gradient.w.is_finite()
                || !gradient.h.is_finite()
                || gradient.w <= 0.0
                || gradient.h <= 0.0
            {
                // 返回稳定的参数错误。
                return Err(super::rhi_invalid(
                    "RhiRenderer gradient geometry is invalid",
                ));
            }
            // 两端颜色和 shader 参数必须保持有限。
            if gradient
                .color_a
                .iter()
                .chain(gradient.color_b.iter())
                .chain(gradient.params.iter())
                .any(|value| !value.is_finite())
            {
                // 返回稳定的参数错误。
                return Err(super::rhi_invalid(
                    "RhiRenderer gradient constants are invalid",
                ));
            }
            // 仿射目标四角必须与单位 quad 的局部插值保持一致。
            if !valid_gradient_corners(&gradient.corners) {
                // 返回稳定的参数错误。
                return Err(super::rhi_invalid("RhiRenderer gradient quad is invalid"));
            }
            // mode 只允许固定 shader ABI 的 linear/radial 两种值。
            if gradient.params[0] != 0.0 && gradient.params[0] != 1.0 {
                // 返回稳定的参数错误。
                return Err(super::rhi_invalid("RhiRenderer gradient mode is invalid"));
            }
            // 显式 scissor 必须已经完成物理坐标 lowering。
            if gradient.scissor.is_some_and(|scissor| !scissor.is_valid()) {
                // 返回稳定的参数错误。
                return Err(super::rhi_invalid(
                    "RhiRenderer gradient scissor is invalid",
                ));
            }
        }
        // 准备渐变的固定 RHI 资源。
        let (pipeline, vertex_buffer, uniform_buffer) =
            self.ensure_gradient_resources(frame.device())?;
        // 从封闭帧作用域取得唯一计划目标。
        let target = frame.render_target();
        // 创建 surface 或离屏 pass，并保留调用方的 load/clear 语义。
        let mut pass = RenderPassPlan::new(target, load);
        // 所有渐变共享同一个物理 viewport。
        pass.push(FramePlanCommand::SetViewport(viewport));
        // 每个渐变只更新常量并保留 painter order。
        for gradient in gradients {
            // 在 draw 前设置当前渐变的裁剪。
            pass.push(FramePlanCommand::SetScissor(gradient.scissor));
            // 上传类型化 GradientConstants，保持 D3D11 16-byte 对齐布局。
            pass.push(FramePlanCommand::UploadUniform {
                buffer: uniform_buffer,
                data: FrameUniformPayload::Gradient(Self::gradient_uniform(viewport, gradient)),
            });
            // 追加单位 quad 的非索引渐变 draw packet。
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
        // 创建计划并追加唯一 pass。
        let mut plan = frame.plan();
        // 保留渐变 painter order。
        plan.push_pass(pass);
        // surface 计划最终 present，texture 计划只执行离屏 submit。
        frame.execute(&plan)?;
        // 资源由 renderer 跨帧复用，不能在这里销毁。
        Ok(())
    }

    // 构造 GradientConstants：viewport、origin/edge_x、edge_y、两色与参数。
    pub(crate) fn gradient_uniform(
        viewport: RhiViewport,
        gradient: &RhiGradientRect,
    ) -> RhiGradientRasterParams {
        // 共享 RHI 值对象唯一拥有仿射边、线性长度与字段顺序。
        RhiGradientRasterParams::new(
            // 保留物理视口。
            viewport,
            // 保留固定 TL、TR、BR、BL 四角。
            gradient.corners,
            // 保留起始 straight-alpha 颜色。
            gradient.color_a,
            // 保留结束 straight-alpha 颜色。
            gradient.color_b,
            // 保留模式、方向或半径参数。
            gradient.params,
        )
    }
}
