//! 混合 painter-order RHI 操作的固定 ABI 约束检查。

// 引入统一结果类型。
use crate::core::error::Result;
// 引入父混合模块定义的操作枚举。
use super::RhiOp;

// 为混合操作执行一次完整约束检查，避免 adapter 看到未定义 ABI。
pub(super) fn check_op(operation: &RhiOp) -> Result<()> {
    // 按操作类型验证其固定 payload。
    match operation {
        // 校验实心 mesh 的顶点和颜色。
        RhiOp::Solid(mesh) => {
            // 顶点必须是至少一个完整三角形的 xy 列表。
            if mesh.vertices.len() < 6
                || !mesh.vertices.len().is_multiple_of(2)
                || !(mesh.vertices.len() / 2).is_multiple_of(3)
                || mesh.vertices.iter().any(|value| !value.is_finite())
                || mesh.rgba.iter().any(|value| !value.is_finite())
            {
                // 返回稳定的参数错误。
                return Err(super::super::rhi_invalid(
                    "RhiRenderer mixed solid ABI is invalid",
                ));
            }
            // 校验显式裁剪。
            if mesh.scissor.is_some_and(|scissor| !scissor.is_valid()) {
                // 返回稳定的参数错误。
                return Err(super::super::rhi_invalid(
                    "RhiRenderer mixed solid scissor is invalid",
                ));
            }
        }
        // 校验解析线段常量。
        RhiOp::Line(line) => {
            if line
                .start
                .iter()
                .chain(line.end.iter())
                .chain(line.rgba.iter())
                .any(|value| !value.is_finite())
                || !line.width.is_finite()
                || line.width <= 0.0
                || line.start == line.end
            {
                return Err(super::super::rhi_invalid(
                    "RhiRenderer mixed line constants are invalid",
                ));
            }
            if line.scissor.is_some_and(|scissor| !scissor.is_valid()) {
                return Err(super::super::rhi_invalid(
                    "RhiRenderer mixed line scissor is invalid",
                ));
            }
        }
        // 校验颜色纹理 quad 的几何和像素 payload。
        RhiOp::Textured(quad) => {
            // 目标矩形和颜色 tint 必须是有限正值。
            if !quad.x.is_finite()
                || !quad.y.is_finite()
                || !quad.w.is_finite()
                || !quad.h.is_finite()
                || quad.w <= 0.0
                || quad.h <= 0.0
                || quad
                    .corners
                    .iter()
                    .flatten()
                    .any(|value| !value.is_finite())
                || quad.rgba.iter().any(|value| !value.is_finite())
            {
                // 返回稳定的参数错误。
                return Err(super::super::rhi_invalid(
                    "RhiRenderer mixed textured geometry is invalid",
                ));
            }
            // 纹理尺寸必须严格为正。
            if quad.pixel_w == 0 || quad.pixel_h == 0 {
                // 返回稳定的参数错误。
                return Err(super::super::rhi_invalid(
                    "RhiRenderer mixed texture extent is empty",
                ));
            }
            // 检查像素数量与纹理描述一致。
            let count = (quad.pixel_w as usize)
                .checked_mul(quad.pixel_h as usize)
                .ok_or_else(|| {
                    super::super::rhi_invalid("RhiRenderer mixed texture extent overflows")
                })?;
            // 拒绝 payload 长度不一致的颜色纹理。
            if quad.pixels.len() != count {
                // 返回稳定的参数错误。
                return Err(super::super::rhi_invalid(
                    "RhiRenderer mixed texture payload is invalid",
                ));
            }
            // 校验显式裁剪。
            if quad.scissor.is_some_and(|scissor| !scissor.is_valid()) {
                // 返回稳定的参数错误。
                return Err(super::super::rhi_invalid(
                    "RhiRenderer mixed textured scissor is invalid",
                ));
            }
        }
        // 校验已经存在的 sampled texture quad。
        RhiOp::Sampled(quad) => {
            // 目标矩形、颜色、纹理和 UV 必须有效。
            if !quad.x.is_finite()
                || !quad.y.is_finite()
                || !quad.w.is_finite()
                || !quad.h.is_finite()
                || quad.w <= 0.0
                || quad.h <= 0.0
                || quad
                    .corners
                    .iter()
                    .flatten()
                    .any(|value| !value.is_finite())
                || quad.rgba.iter().any(|value| !value.is_finite())
                || quad.texture.raw() == 0
                || [quad.u0, quad.v0, quad.u1, quad.v1]
                    .iter()
                    .any(|value| !value.is_finite() || *value < 0.0 || *value > 1.0)
                || quad.u1 <= quad.u0
                || quad.v1 <= quad.v0
            {
                // 返回稳定的参数错误。
                return Err(super::super::rhi_invalid(
                    "RhiRenderer mixed sampled texture geometry is invalid",
                ));
            }
            // 校验显式裁剪。
            if quad.scissor.is_some_and(|scissor| !scissor.is_valid()) {
                // 返回稳定的参数错误。
                return Err(super::super::rhi_invalid(
                    "RhiRenderer mixed sampled texture scissor is invalid",
                ));
            }
        }
        // 校验 R8 glyph coverage quad 的几何和覆盖率 payload。
        RhiOp::Coverage(quad) => {
            // 目标矩形和颜色必须是有限正值。
            if !quad.x.is_finite()
                || !quad.y.is_finite()
                || !quad.w.is_finite()
                || !quad.h.is_finite()
                || quad.w <= 0.0
                || quad.h <= 0.0
                || quad
                    .corners
                    .iter()
                    .flatten()
                    .any(|value| !value.is_finite())
                || quad.rgba.iter().any(|value| !value.is_finite())
            {
                // 返回稳定的参数错误。
                return Err(super::super::rhi_invalid(
                    "RhiRenderer mixed coverage geometry is invalid",
                ));
            }
            // 覆盖率纹理尺寸必须严格为正。
            if quad.pixel_w == 0 || quad.pixel_h == 0 {
                // 返回稳定的参数错误。
                return Err(super::super::rhi_invalid(
                    "RhiRenderer mixed coverage extent is empty",
                ));
            }
            // 检查 R8 payload 长度。
            let count = (quad.pixel_w as usize)
                .checked_mul(quad.pixel_h as usize)
                .ok_or_else(|| {
                    super::super::rhi_invalid("RhiRenderer mixed coverage extent overflows")
                })?;
            // 拒绝 payload 长度不一致的 coverage 纹理。
            if quad.coverage.len() != count {
                // 返回稳定的参数错误。
                return Err(super::super::rhi_invalid(
                    "RhiRenderer mixed coverage payload is invalid",
                ));
            }
            // 校验显式裁剪。
            if quad.scissor.is_some_and(|scissor| !scissor.is_valid()) {
                // 返回稳定的参数错误。
                return Err(super::super::rhi_invalid(
                    "RhiRenderer mixed coverage scissor is invalid",
                ));
            }
        }
        // 校验 RGBA8 MSDF 字形 quad 的几何、边列表和距离范围。
        RhiOp::Msdf(quad) => {
            // AABB、四角、颜色和距离范围必须是有限值。
            if !quad.x.is_finite()
                || !quad.y.is_finite()
                || !quad.w.is_finite()
                || !quad.h.is_finite()
                || quad.w <= 0.0
                || quad.h <= 0.0
                || quad
                    .corners
                    .iter()
                    .flatten()
                    .any(|value| !value.is_finite())
                || !quad.range.is_finite()
                || quad.range <= 0.0
                || quad.rgba.iter().any(|value| !value.is_finite())
            {
                // 返回稳定的参数错误。
                return Err(super::super::rhi_invalid(
                    "RhiRenderer mixed MSDF geometry is invalid",
                ));
            }
            // MSDF 源纹理尺寸必须严格为正。
            if quad.pixel_w == 0 || quad.pixel_h == 0 {
                // 返回稳定的参数错误。
                return Err(super::super::rhi_invalid(
                    "RhiRenderer mixed MSDF extent is empty",
                ));
            }
            // 边列表必须是有限的四浮点一条边，并受上限保护。
            if quad.edges.len() < 4
                || !quad.edges.len().is_multiple_of(4)
                || quad.edges.len() / 4 > 4096
                || quad.edges.iter().any(|value| !value.is_finite())
            {
                // 返回稳定的参数错误。
                return Err(super::super::rhi_invalid(
                    "RhiRenderer mixed MSDF edge list is invalid",
                ));
            }
            // 显式裁剪必须已经完成物理坐标 lowering。
            if quad.scissor.is_some_and(|scissor| !scissor.is_valid()) {
                // 返回稳定的参数错误。
                return Err(super::super::rhi_invalid(
                    "RhiRenderer mixed MSDF scissor is invalid",
                ));
            }
        }
        // 校验渐变常量。
        RhiOp::Gradient(gradient) => {
            // 渐变目标必须是有限正矩形。
            if !gradient.x.is_finite()
                || !gradient.y.is_finite()
                || !gradient.w.is_finite()
                || !gradient.h.is_finite()
                || gradient.w <= 0.0
                || gradient.h <= 0.0
                || gradient
                    .color_a
                    .iter()
                    .chain(gradient.color_b.iter())
                    .chain(gradient.params.iter())
                    .any(|value| !value.is_finite())
                || !super::super::gradient::valid_gradient_corners(&gradient.corners)
                || (gradient.params[0] != 0.0 && gradient.params[0] != 1.0)
            {
                // 返回稳定的参数错误。
                return Err(super::super::rhi_invalid(
                    "RhiRenderer mixed gradient constants are invalid",
                ));
            }
            // 校验显式裁剪。
            if gradient.scissor.is_some_and(|scissor| !scissor.is_valid()) {
                // 返回稳定的参数错误。
                return Err(super::super::rhi_invalid(
                    "RhiRenderer mixed gradient scissor is invalid",
                ));
            }
        }
        // 校验圆角/描边矩形常量（唯一校验权威见 RhiShapeRect::validate）。
        RhiOp::Shape(rect) | RhiOp::AdditiveShape(rect) => {
            match rect.validate() {
                Ok(()) => {}
                // 几何与常量违反在 mixed 提交路径共享同一稳定文案。
                Err(super::super::RhiShapeRectInvalid::Geometry)
                | Err(super::super::RhiShapeRectInvalid::Constants) => {
                    // 返回稳定的参数错误。
                    return Err(super::super::rhi_invalid(
                        "RhiRenderer mixed shape constants are invalid",
                    ));
                }
                Err(super::super::RhiShapeRectInvalid::Scissor) => {
                    // 返回稳定的参数错误。
                    return Err(super::super::rhi_invalid(
                        "RhiRenderer mixed shape scissor is invalid",
                    ));
                }
            }
        }
        // 校验原生扇形常量。
        RhiOp::Sector(sector) => {
            // 扇形外接椭圆、角度和颜色必须是有限正值。
            if !sector.x.is_finite()
                || !sector.y.is_finite()
                || !sector.w.is_finite()
                || !sector.h.is_finite()
                || sector.w <= 0.0
                || sector.h <= 0.0
                || !sector.start_angle.is_finite()
                || !sector.sweep_angle.is_finite()
                || sector.start_angle < 0.0
                || sector.start_angle >= std::f32::consts::TAU
                || sector.sweep_angle <= 0.0
                || sector.sweep_angle > std::f32::consts::TAU
                || sector.rgba.iter().any(|value| !value.is_finite())
            {
                // 返回稳定的参数错误。
                return Err(super::super::rhi_invalid(
                    "RhiRenderer mixed sector constants are invalid",
                ));
            }
            // 校验显式裁剪。
            if sector.scissor.is_some_and(|scissor| !scissor.is_valid()) {
                // 返回稳定的参数错误。
                return Err(super::super::rhi_invalid(
                    "RhiRenderer mixed sector scissor is invalid",
                ));
            }
        }
        // 校验仿射阴影常量。
        RhiOp::Shadow(shadow) => {
            // 阴影本体尺寸和 blur 必须是有限值。
            if !shadow.w.is_finite()
                || !shadow.h.is_finite()
                || shadow.w <= 0.0
                || shadow.h <= 0.0
                || !shadow.blur_x.is_finite()
                || !shadow.blur_y.is_finite()
                || shadow.blur_x < 0.0
                || shadow.blur_y < 0.0
                || shadow.rgba.iter().any(|value| !value.is_finite())
                || shadow
                    .radius
                    .iter()
                    .any(|value| !value.is_finite() || *value < 0.0)
                || shadow
                    .corners
                    .iter()
                    .flatten()
                    .any(|value| !value.is_finite())
            {
                // 返回稳定的参数错误。
                return Err(super::super::rhi_invalid(
                    "RhiRenderer mixed shadow constants are invalid",
                ));
            }
            // 校验显式裁剪。
            if shadow.scissor.is_some_and(|scissor| !scissor.is_valid()) {
                // 返回稳定的参数错误。
                return Err(super::super::rhi_invalid(
                    "RhiRenderer mixed shadow scissor is invalid",
                ));
            }
        }
    }
    // 当前操作满足统一约束。
    Ok(())
}
