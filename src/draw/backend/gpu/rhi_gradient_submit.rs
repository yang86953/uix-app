//! GPU-native 渐变队列的 RHI 提交 lowering。

// 引入统一错误。
use crate::core::Error;
// 引入通用渐变 payload、renderer 与封闭帧角色。
use crate::draw::backend::rhi_renderer::{RhiGradientRect, RhiRenderer, RhiRendererFrame};
// 引入 Device-only lowering 所需的目标、extent 与 pass load 语义。
use crate::platform::presentation::rhi::{GraphicsDevice, LoadAction, RhiExtent, TextureHandle};

// 引入 pending 操作和 submit 模块的物理 lowering helper。
use super::super::pending::PendingNativeOp;
use super::{rhi_physical_geometry, rhi_physical_scissor};
// 引入 GPU canvas 类型。
use super::super::NativeGpuCanvas2D;

// 为 GPU canvas 提供渐变队列的通用 RHI 提交入口。
impl NativeGpuCanvas2D {
    // 尝试把纯线性/径向渐变队列交给通用 FramePlan gradient lowering。
    pub(crate) fn submit_rhi_gradients(
        &self,
        renderer: &mut RhiRenderer,
        // 借用只允许资源、命令与 submit 的 Device 角色。
        device: &mut dyn GraphicsDevice,
        // 接收 retained texture 的物理范围。
        extent: RhiExtent,
        load: LoadAction,
        // 指定本次 gradient 计划唯一允许写入的离屏纹理。
        target: TextureHandle,
    ) -> Result<bool, Error> {
        // soft 内容与 RHI gradient 不能在这条纵切中交错提交。
        if self.soft_has_content || self.pending_native.is_empty() {
            // 返回 false 让兼容路径保持原有 painter-order 语义。
            return Ok(false);
        }
        // 从显式纹理范围推导物理 viewport 和两轴缩放。
        let (viewport, scale_x, scale_y) =
            rhi_physical_geometry(extent, self.surface_w, self.surface_h);
        // 预先分配同一顺序的渐变载荷。
        let mut gradients = Vec::with_capacity(self.pending_native.len());
        // 逐项确认当前队列是可迁移的渐变子集。
        for operation in &self.pending_native {
            // 把逻辑裁剪转换为物理裁剪。
            let Some(scissor) = rhi_physical_scissor(operation.scissor(), scale_x, scale_y, extent)
            else {
                // 空裁剪保留原有 no-op 语义。
                return Ok(false);
            };
            // 线性和径向渐变共用一个有限 shader pipeline。
            let gradient = match operation {
                // 线性渐变缩放真实四角，保留旋转/剪切后的局部坐标。
                PendingNativeOp::LinearGradient(gradient) => {
                    let value = gradient.rect;
                    let Some(corners) = super::scale_rhi_corners(value.corners, scale_x, scale_y)
                    else {
                        // 异常物理坐标交回兼容路径。
                        return Ok(false);
                    };
                    if !super::super::geometry::convex_quad_is_valid(&corners) {
                        // 退化 quad 不能进入单位渐变 shader。
                        return Ok(false);
                    }
                    let (min_x, min_y, max_x, max_y) = super::super::geometry::quad_aabb(corners);
                    RhiGradientRect {
                        x: min_x,
                        y: min_y,
                        w: max_x - min_x,
                        h: max_y - min_y,
                        corners,
                        color_a: value.color_a,
                        color_b: value.color_b,
                        // params[2]/[3] 携带渐变局部矩形宽高，shader 的对角
                        // 插值与 CPU linear_gradient_t 在任意仿射下同源。
                        params: [0.0, value.dir as f32, value.local_w, value.local_h],
                        scissor: Some(scissor),
                    }
                }
                // 径向 shader 在归一化局部坐标中保持圆语义，物理空间可为椭圆。
                PendingNativeOp::RadialGradient(gradient) => {
                    let value = gradient.grad;
                    let Some(corners) = super::scale_rhi_corners(value.corners, scale_x, scale_y)
                    else {
                        // 异常物理坐标交回兼容路径。
                        return Ok(false);
                    };
                    if !super::super::geometry::convex_quad_is_valid(&corners) {
                        // 退化 quad 不能进入单位渐变 shader。
                        return Ok(false);
                    }
                    // 半径比值在 affine 变换前后保持不变。
                    let inner = value.inner_r;
                    let outer = value.outer_r;
                    let inner_ratio = inner / outer;
                    if !value.cx.is_finite()
                        || !value.cy.is_finite()
                        || !inner.is_finite()
                        || !outer.is_finite()
                        || inner < 0.0
                        || outer <= 0.0
                        || inner > outer
                        || !inner_ratio.is_finite()
                    {
                        // 异常半径交回原有路径。
                        return Ok(false);
                    }
                    let (min_x, min_y, max_x, max_y) = super::super::geometry::quad_aabb(corners);
                    // 构造通用渐变矩形 payload。
                    RhiGradientRect {
                        x: min_x,
                        y: min_y,
                        w: max_x - min_x,
                        h: max_y - min_y,
                        corners,
                        color_a: value.color_inner,
                        color_b: value.color_outer,
                        params: [1.0, inner_ratio * 0.5, 0.5, 0.0],
                        scissor: Some(scissor),
                    }
                }
                // 圆角、描边、字形、图片等仍由兼容路径处理。
                _ => return Ok(false),
            };
            // 保持 pending queue 的 painter order。
            gradients.push(gradient);
        }
        // Device-only lowering 只能构造显式纹理 Offscreen 帧。
        let frame = RhiRendererFrame::offscreen(device, target);
        // 由通用 renderer 生成并执行离屏 FramePlan。
        renderer.execute_gradients(frame, viewport, load, &gradients)?;
        // 告知调用方本次队列已经通过 RHI present 成功。
        Ok(true)
    }
}
