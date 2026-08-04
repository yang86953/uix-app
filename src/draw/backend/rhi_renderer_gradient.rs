//! 通用 GPU Renderer 的线性/径向渐变 RHI lowering。

// 引入渐变常量编码返回的共享字节载荷。
use std::sync::Arc;

// 引入共享错误和最终 damage 类型。
use crate::core::error::Result;
// 引入当前帧的 damage 语义。
use crate::core::PresentDamage;
// 引入薄 RHI 的设备、命令和 pass 类型。
use crate::native::present::rhi::{DrawPacket, GraphicsContextRhi, LoadAction, RhiViewport};

// 引入父 renderer 的帧计划、target 和渐变载荷。
use super::{
    FramePlan, FramePlanCommand, RenderPassPlan, RenderTargetRef, RhiGradientRect, RhiRenderer,
};

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
        context: &mut dyn GraphicsContextRhi,
        damage: PresentDamage,
        viewport: RhiViewport,
        load: LoadAction,
        target: RenderTargetRef,
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
        let (pipeline, vertex_buffer, uniform_buffer) = self.ensure_gradient_resources(context)?;
        // 计划使用当前 context 的 surface 代际。
        let surface = context.token();
        // 创建 surface 或离屏 pass，并保留调用方的 load/clear 语义。
        let mut pass = RenderPassPlan::new(target, load);
        // 所有渐变共享同一个物理 viewport。
        pass.push(FramePlanCommand::SetViewport(viewport));
        // 每个渐变只更新常量并保留 painter order。
        for gradient in gradients {
            // 在 draw 前设置当前渐变的裁剪。
            pass.push(FramePlanCommand::SetScissor(gradient.scissor));
            // 上传完整 GradientConstants，保持 D3D11 16-byte 对齐布局。
            pass.push(FramePlanCommand::UpdateBuffer {
                buffer: uniform_buffer,
                offset: 0,
                data: Self::encode_gradient_constants(viewport, gradient),
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
        let mut plan = FramePlan::new(surface, damage);
        // 保留渐变 painter order。
        plan.push_pass(pass);
        // surface 计划最终 present，texture 计划只执行离屏 submit。
        super::execute_plan_for_target(context, &plan, target)?;
        // 资源由 renderer 跨帧复用，不能在这里销毁。
        Ok(())
    }

    // 编码 96 字节 GradientConstants：viewport、origin/edge_x、edge_y、两色与参数。
    pub(crate) fn encode_gradient_constants(
        viewport: RhiViewport,
        gradient: &RhiGradientRect,
    ) -> Arc<[u8]> {
        // 用 TL 角和两条边恢复任意 affine parallelogram。
        let origin = gradient.corners[0];
        let edge_x = [
            gradient.corners[1][0] - origin[0],
            gradient.corners[1][1] - origin[1],
        ];
        let edge_y = [
            gradient.corners[3][0] - origin[0],
            gradient.corners[3][1] - origin[1],
        ];
        // 线性对角方向仍按变换后两条边的长度保持旧 AABB 语义。
        let mut params = gradient.params;
        if params[0] < 0.5 {
            params[2] = (edge_x[0] * edge_x[0] + edge_x[1] * edge_x[1]).sqrt();
            params[3] = (edge_y[0] * edge_y[0] + edge_y[1] * edge_y[1]).sqrt();
        }
        // 归一化局部坐标让线性方向和径向圆距不受物理 DPR 影响。
        Self::encode_f32s(&[
            viewport.width,
            viewport.height,
            0.0,
            0.0,
            origin[0],
            origin[1],
            edge_x[0],
            edge_x[1],
            edge_y[0],
            edge_y[1],
            0.0,
            0.0,
            gradient.color_a[0],
            gradient.color_a[1],
            gradient.color_a[2],
            gradient.color_a[3],
            gradient.color_b[0],
            gradient.color_b[1],
            gradient.color_b[2],
            gradient.color_b[3],
            params[0],
            params[1],
            params[2],
            params[3],
        ])
    }
}
