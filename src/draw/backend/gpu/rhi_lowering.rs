//! NativeGpuCanvas2D 的保序混合 RHI lowering。

// 引入当前错误类型、几何和结果别名。
use crate::core::error::{Error, Result};
use crate::core::{Point, Rect};
// 引入统一的 FramePlan 离屏执行类型。
use crate::draw::backend::frame_plan::FramePlan;
// 引入通用 renderer 的混合载荷和执行器。
use crate::draw::backend::rhi_renderer::{
    RhiCoverageQuad, RhiGradientRect, RhiLineSegment, RhiMsdfQuad, RhiOp, RhiRenderer,
    RhiRendererFrame, RhiSector, RhiShadow, RhiShapeRect, RhiSolidMesh, RhiTexturedQuad,
};
// 引入 GPU native 队列的几何载荷和 TextureMove 资源类型。
use crate::platform::presentation::rhi::{
    GraphicsDevice, LoadAction, RhiExtent, RhiViewport, TextureHandle, TextureMove,
};

// 引入待决操作的完整枚举和 scroll boundary 载荷。
use super::super::pending::{PendingNativeOp, PendingNativeScroll};
// 引入待决 canvas 类型。
use super::NativeGpuCanvas2D;

// 检查一组颜色或常量是否可以进入 RHI ABI。
fn finite_values(values: &[f32]) -> bool {
    // 所有固定常量都必须是有限数。
    values.iter().all(|value| value.is_finite())
}

// 按事实能力选择普通或 Additive shape pipeline。
fn shape_rhi_op(shape: RhiShapeRect, additive: bool, additive_supported: bool) -> Option<RhiOp> {
    // adapter 未声明 Additive 时必须保持原子回退，不能偷换为 SrcOver。
    if additive && !additive_supported {
        // 返回 None 让上层保留 pending queue 与 typed fallback 边界。
        return None;
    }
    // 两种 blend 共享同一 shape 几何与常量，只切换固定 pipeline。
    Some(if additive {
        // 显式选择 Additive shape pipeline。
        RhiOp::AdditiveShape(shape)
    } else {
        // 普通矩形继续使用 premultiplied SrcOver pipeline。
        RhiOp::Shape(shape)
    })
}

// 把单个待决操作降低为保序混合 RHI 载荷。
fn lower_operation(
    operation: &PendingNativeOp,
    // 借用只读 Device 能力，禁止 lowering 取得 Surface 生命周期。
    device: &dyn GraphicsDevice,
    // 接收当前显式纹理目标的物理范围。
    extent: RhiExtent,
    scale_x: f32,
    scale_y: f32,
) -> Option<RhiOp> {
    // 所有 native 操作共享一份经过物理缩放和边界裁剪的 scissor。
    let scissor = super::rhi_physical_scissor(operation.scissor(), scale_x, scale_y, extent)?;
    // 按原始 pending queue 顺序逐项选择对应的 RHI 语义。
    match operation {
        // 将已完成 tessellation 的 mesh 只做坐标缩放。
        PendingNativeOp::SolidMesh(mesh) => {
            // 拒绝无法表示为物理 xy 列表的 mesh。
            let vertices =
                super::scale_rhi_vertices(mesh.mesh.vertices.as_ref(), scale_x, scale_y)?;
            // 拒绝异常颜色，避免把无效浮点送入 shader。
            if !finite_values(&mesh.mesh.rgba) {
                // 异常载荷交回兼容路径。
                return None;
            }
            // 返回 solid mesh lowering 结果。
            Some(RhiOp::Solid(RhiSolidMesh {
                vertices,
                rgba: mesh.mesh.rgba,
                scissor: Some(scissor),
            }))
        }
        // 将设备空间中心线缩放为解析覆盖率线段，不再展开为硬边三角形。
        PendingNativeOp::Line(line) => {
            let value = line.line;
            let width = value.width * (scale_x.abs() * scale_y.abs()).sqrt();
            let start = [value.start[0] * scale_x, value.start[1] * scale_y];
            let end = [value.end[0] * scale_x, value.end[1] * scale_y];
            if !finite_values(&start)
                || !finite_values(&end)
                || !finite_values(&value.rgba)
                || !width.is_finite()
                || width <= 0.0
                || start == end
            {
                return None;
            }
            Some(RhiOp::Line(RhiLineSegment {
                start,
                end,
                width,
                rgba: value.rgba,
                scissor: Some(scissor),
            }))
        }
        // 将填充矩形统一降低为 shape SDF，半描边宽度为零。
        PendingNativeOp::SolidRect(rect) => {
            // 读取已经规整过颜色和圆角的矩形。
            let value = rect.rect;
            // 把逻辑圆角换算到物理空间并按物理矩形规范化。
            let radius = super::scale_rhi_shape_radius(
                value.radius,
                scale_x,
                scale_y,
                value.w * scale_x,
                value.h * scale_y,
            )?;
            // 拒绝异常几何或颜色。
            if value.w <= 0.0
                || value.h <= 0.0
                || !value.x.is_finite()
                || !value.y.is_finite()
                || !value.w.is_finite()
                || !value.h.is_finite()
                || !finite_values(&value.rgba)
            {
                // 异常载荷交回兼容路径。
                return None;
            }
            // 构造两种 blend 共用的填充 shape 载荷。
            let shape = RhiShapeRect {
                x: value.x * scale_x,
                y: value.y * scale_y,
                w: value.w * scale_x,
                h: value.h * scale_y,
                rgba: value.rgba,
                radius,
                half_stroke: 0.0,
                scissor: Some(scissor),
            };
            // 依据 pending 语义与当前 RHI 事实能力选择固定 pipeline。
            shape_rhi_op(
                shape,
                rect.additive,
                device.device_capabilities().additive_blend,
            )
        }
        // 将圆角或直角描边矩形降低为 shape SDF。
        PendingNativeOp::StrokeRect(rect) => {
            // 读取已经规整过颜色、半径和线宽的矩形。
            let value = rect.rect;
            // 把逻辑圆角换算到物理空间并按物理矩形规范化。
            let radius = super::scale_rhi_shape_radius(
                value.radius,
                scale_x,
                scale_y,
                value.w * scale_x,
                value.h * scale_y,
            )?;
            // 各向异性缩放沿用已有 shader 的等效半径规则。
            let half_stroke = value.line_width * 0.5 * (scale_x * scale_y).sqrt();
            // 拒绝异常几何、颜色或线宽。
            if value.w <= 0.0
                || value.h <= 0.0
                || !value.x.is_finite()
                || !value.y.is_finite()
                || !value.w.is_finite()
                || !value.h.is_finite()
                || !finite_values(&value.rgba)
                || !half_stroke.is_finite()
                || half_stroke < 0.0
            {
                // 异常载荷交回兼容路径。
                return None;
            }
            // 构造 SrcOver/Additive 共用的描边 shape 载荷。
            let shape = RhiShapeRect {
                x: value.x * scale_x,
                y: value.y * scale_y,
                w: value.w * scale_x,
                h: value.h * scale_y,
                rgba: value.rgba,
                radius,
                half_stroke,
                scissor: Some(scissor),
            };
            // 依据描边事实与当前 RHI 能力选择固定 pipeline。
            shape_rhi_op(
                shape,
                rect.additive,
                device.device_capabilities().additive_blend,
            )
        }
        // 将已完成仿射 lowering 的 box shadow 降低为保留 painter order 的 shadow draw。
        PendingNativeOp::BoxShadow(shadow) => {
            // 读取已展开的阴影参数和设备四边形。
            let value = shadow.shadow;
            let corners = super::scale_rhi_corners(value.corners, scale_x, scale_y)?;
            // 仿射 shadow shader 只接受有限、非退化、凸四边形。
            if !super::super::geometry::convex_quad_is_valid(&corners) {
                // 不把真实四边形错误压平为 AABB。
                return None;
            }
            // 把逻辑圆角换算到物理空间并按物理本体矩形规范化。
            let radius = super::scale_rhi_shape_radius(
                value.radius,
                scale_x,
                scale_y,
                value.w * scale_x,
                value.h * scale_y,
            )?;
            // 分别缩放 blur 和 offset，保留各向异性语义。
            let blur_x = value.blur_x * scale_x.abs();
            let blur_y = value.blur_y * scale_y.abs();
            let offset_x = value.offset_x * scale_x;
            let offset_y = value.offset_y * scale_y;
            // 拒绝异常阴影参数和几何。
            if value.w <= 0.0
                || value.h <= 0.0
                || !value.x.is_finite()
                || !value.y.is_finite()
                || !value.w.is_finite()
                || !value.h.is_finite()
                || !finite_values(&value.rgba)
                || !blur_x.is_finite()
                || !blur_y.is_finite()
                || !offset_x.is_finite()
                || !offset_y.is_finite()
                || blur_x < 0.0
                || blur_y < 0.0
            {
                // 异常载荷交回兼容路径。
                return None;
            }
            // 返回保留设备四角的 shadow lowering 结果。
            Some(RhiOp::Shadow(RhiShadow {
                // 保存物理本体宽度，原点已经冻结在扩展四角中。
                w: value.w * scale_x,
                // 保存物理本体高度。
                h: value.h * scale_y,
                // 保存已经包含 offset 与 blur 的设备四角。
                corners,
                // 保存物理 X 轴模糊量。
                blur_x,
                // 保存物理 Y 轴模糊量。
                blur_y,
                // 保存直通阴影颜色。
                rgba: value.rgba,
                // 保存物理四角半径。
                radius,
                // 保存覆盖曲线身份。
                ambient: value.ambient,
                // 保存物理裁剪。
                scissor: Some(scissor),
            }))
        }
        // 将无旋转的 R8 glyph coverage 降低为 sampled coverage quad。
        PendingNativeOp::Glyph(glyph) => {
            // 保存可选的本地轮廓边列表，MSDF 路径优先于面积 coverage。
            let outline_mesh = glyph.glyph.outline_mesh.as_ref();
            // 检查字形源纹理尺寸，R8 与 MSDF 共用同一 padded extent。
            let pixel_count = (glyph.glyph.cov_w as usize).checked_mul(glyph.glyph.cov_h as usize);
            if glyph.glyph.cov_w == 0
                || glyph.glyph.cov_h == 0
                || (outline_mesh.is_none() && pixel_count != Some(glyph.glyph.coverage.len()))
            {
                // 异常 coverage 交回兼容路径。
                return None;
            }
            // 换算字形目标矩形到物理空间。
            let x = glyph.glyph.x * scale_x;
            let y = glyph.glyph.y * scale_y;
            let w = glyph.glyph.w * scale_x;
            let h = glyph.glyph.h * scale_y;
            // 拒绝异常几何或颜色。
            if w <= 0.0
                || h <= 0.0
                || !x.is_finite()
                || !y.is_finite()
                || !w.is_finite()
                || !h.is_finite()
                || !finite_values(&glyph.glyph.rgba)
            {
                // 异常载荷交回兼容路径。
                return None;
            }
            // 有轮廓边列表时生成通用 RGBA8 MSDF quad，不回退到高层 glyph atlas。
            if let Some(edges) = outline_mesh {
                // 轮廓边列表必须满足 MSDF 生成器的固定边 ABI。
                if !crate::draw::resources::font::glyph_outline::is_outline_edges(edges.as_ref()) {
                    // 异常轮廓交回兼容路径。
                    return None;
                }
                // 返回保留仿射四角的 MSDF lowering 结果。
                return Some(RhiOp::Msdf(RhiMsdfQuad {
                    x,
                    y,
                    w,
                    h,
                    corners: glyph
                        .glyph
                        .corners
                        .map(|corner| [corner[0] * scale_x, corner[1] * scale_y]),
                    range: crate::draw::resources::font::glyph_outline::MSDF_RANGE,
                    edges: std::sync::Arc::clone(edges),
                    pixel_w: glyph.glyph.cov_w,
                    pixel_h: glyph.glyph.cov_h,
                    rgba: glyph.glyph.rgba,
                    scissor: Some(scissor),
                }));
            }
            // 返回 R8 coverage lowering 结果。
            Some(RhiOp::Coverage(RhiCoverageQuad {
                x,
                y,
                w,
                h,
                corners: glyph
                    .glyph
                    .corners
                    .map(|corner| [corner[0] * scale_x, corner[1] * scale_y]),
                rgba: glyph.glyph.rgba,
                coverage: glyph.glyph.coverage.clone(),
                pixel_w: glyph.glyph.cov_w,
                pixel_h: glyph.glyph.cov_h,
                scissor: Some(scissor),
            }))
        }
        // 将纯 SrcOver/Additive 图片 blit 降低为采样 quad。
        PendingNativeOp::ImageBlit(image) => {
            // Additive 必须由当前 adapter 明确声明支持。
            if image.blit.additive
                // Additive 能力只能从组合上下文的 Device 角色读取。
                && !device.device_capabilities().additive_blend
            {
                // 不伪造 Additive 的 blending 结果。
                return None;
            }
            // 检查图片尺寸和紧密 BGRA payload。
            let pixel_count =
                (image.blit.pixel_w as usize).checked_mul(image.blit.pixel_h as usize);
            if image.blit.pixel_w == 0
                || image.blit.pixel_h == 0
                || pixel_count != Some(image.blit.pixels.len())
            {
                // 异常图片交回兼容路径。
                return None;
            }
            // 换算图片目标矩形到物理空间。
            let x = image.blit.x * scale_x;
            let y = image.blit.y * scale_y;
            let w = image.blit.w * scale_x;
            let h = image.blit.h * scale_y;
            // 保持原有 premultiplied opacity 语义。
            let opacity = image.blit.opacity.clamp(0.0, 1.0);
            // 拒绝异常目标几何或透明度。
            if w <= 0.0
                || h <= 0.0
                || !x.is_finite()
                || !y.is_finite()
                || !w.is_finite()
                || !h.is_finite()
                || !opacity.is_finite()
            {
                // 异常图片交回兼容路径。
                return None;
            }
            // 返回颜色纹理 lowering 结果。
            Some(RhiOp::Textured(RhiTexturedQuad {
                x,
                y,
                w,
                h,
                corners: image
                    .blit
                    .corners
                    .map(|corner| [corner[0] * scale_x, corner[1] * scale_y]),
                rgba: [opacity; 4],
                additive: image.blit.additive,
                pixels: image.blit.pixels.clone(),
                pixel_w: image.blit.pixel_w,
                pixel_h: image.blit.pixel_h,
                scissor: Some(scissor),
            }))
        }
        // 将线性渐变 lowering 为统一渐变常量。
        PendingNativeOp::LinearGradient(gradient) => {
            // 读取已经计算好的逻辑渐变矩形。
            let value = gradient.rect;
            // 缩放真实四角，避免旋转/剪切再次压平成 AABB。
            let corners = super::scale_rhi_corners(value.corners, scale_x, scale_y)?;
            // 渐变 quad 必须保持固定顶点顺序和非退化面积。
            if !super::super::geometry::convex_quad_is_valid(&corners) {
                // 异常仿射几何交回兼容路径。
                return None;
            }
            // 计算 AABB 供通用载荷保留边界诊断信息。
            let (min_x, min_y, max_x, max_y) = super::super::geometry::quad_aabb(corners);
            // 拒绝异常几何和颜色。
            if value.w <= 0.0
                || value.h <= 0.0
                || !value.x.is_finite()
                || !value.y.is_finite()
                || !value.w.is_finite()
                || !value.h.is_finite()
                || !finite_values(&value.color_a)
                || !finite_values(&value.color_b)
            {
                // 异常渐变交回兼容路径。
                return None;
            }
            // 返回线性渐变 lowering 结果，mode 0 表示线性。
            Some(RhiOp::Gradient(RhiGradientRect {
                x: min_x,
                y: min_y,
                w: max_x - min_x,
                h: max_y - min_y,
                corners,
                color_a: value.color_a,
                color_b: value.color_b,
                // params[2]/[3] 携带渐变局部矩形宽高，shader 的对角插值与
                // CPU linear_gradient_t 在任意仿射下同源。
                params: [0.0, value.dir as f32, value.local_w, value.local_h],
                stops: value.stops,
                // 圆角掩码已在 queue 期按逻辑空间预计算。
                mask_radius: value.mask.map(|(radius, _)| radius).unwrap_or([0.0; 4]),
                mask: value.mask.map(|(_, mask)| mask).unwrap_or([
                    value.local_w,
                    value.local_h,
                    0.0,
                    0.0,
                    1.0,
                    1.0,
                ]),
                scissor: Some(scissor),
            }))
        }
        // 将任意 affine 变换的径向渐变 lowering 为局部圆盘常量。
        PendingNativeOp::RadialGradient(gradient) => {
            // 读取已经计算好的逻辑径向渐变参数。
            let value = gradient.grad;
            // 缩放真实四角；圆距在 shader 的归一化局部坐标中计算。
            let corners = super::scale_rhi_corners(value.corners, scale_x, scale_y)?;
            // 径向 quad 同样不能退化，否则局部圆盘没有可定义的映射。
            if !super::super::geometry::convex_quad_is_valid(&corners) {
                // 异常仿射几何交回兼容路径。
                return None;
            }
            // 保留逻辑半径比值，outer 在 shader 中固定为半径 0.5。
            let inner = value.inner_r;
            let outer = value.outer_r;
            let inner_ratio = inner / outer;
            // 计算实际 quad 的物理包围盒。
            let (min_x, min_y, max_x, max_y) = super::super::geometry::quad_aabb(corners);
            // 拒绝异常半径、几何或颜色。
            if !value.cx.is_finite()
                || !value.cy.is_finite()
                || !inner.is_finite()
                || !outer.is_finite()
                || inner < 0.0
                || outer <= 0.0
                || inner > outer
                || !inner_ratio.is_finite()
                || !finite_values(&value.color_inner)
                || !finite_values(&value.color_outer)
            {
                // 异常渐变交回兼容路径。
                return None;
            }
            // 返回径向渐变 lowering 结果，mode 1 表示径向。
            Some(RhiOp::Gradient(RhiGradientRect {
                x: min_x,
                y: min_y,
                w: max_x - min_x,
                h: max_y - min_y,
                corners,
                color_a: value.color_inner,
                color_b: value.color_outer,
                params: [1.0, inner_ratio * 0.5, 0.5, 0.0],
                stops: None,
                // 圆角掩码已在 queue 期按逻辑空间预计算。
                mask_radius: value.mask.map(|(radius, _)| radius).unwrap_or([0.0; 4]),
                mask: value.mask.map(|(_, mask)| mask).unwrap_or([
                    outer * 2.0,
                    outer * 2.0,
                    0.0,
                    0.0,
                    1.0,
                    1.0,
                ]),
                scissor: Some(scissor),
            }))
        }
        // 将轴对齐原生扇形降低为物理外接椭圆和统一 SectorConstants。
        PendingNativeOp::Sector(sector) => {
            // 读取 queue 阶段已经规整过的扇形参数。
            let value = sector.sector;
            // mixed-DPI 可在两个轴使用不同物理缩放，因此保留椭圆外接盒。
            let radius_x = value.radius * scale_x.abs();
            let radius_y = value.radius * scale_y.abs();
            let x = value.cx * scale_x - radius_x;
            let y = value.cy * scale_y - radius_y;
            // 只有合法角度和正尺寸才能进入固定 shader ABI。
            if !x.is_finite()
                || !y.is_finite()
                || !radius_x.is_finite()
                || !radius_y.is_finite()
                || radius_x <= 0.0
                || radius_y <= 0.0
                || !value.start_angle.is_finite()
                || !value.sweep_angle.is_finite()
                || value.start_angle < 0.0
                || value.start_angle >= std::f32::consts::TAU
                || value.sweep_angle <= 0.0
                || value.sweep_angle > std::f32::consts::TAU
                || !finite_values(&value.rgba)
            {
                // 异常载荷交回兼容路径。
                return None;
            }
            // 返回保留 painter order 的 RHI sector 操作。
            Some(RhiOp::Sector(RhiSector {
                x,
                y,
                w: radius_x * 2.0,
                h: radius_y * 2.0,
                start_angle: value.start_angle,
                sweep_angle: value.sweep_angle,
                rgba: value.rgba,
                scissor: Some(scissor),
            }))
        }
        // scroll 已在上层切成 TextureMove boundary，不能伪装成 sampled draw。
        PendingNativeOp::ScrollCopy(_) => None,
    }
}

// 把原生 Canvas2D scroll 转成与 DrawSurface 相同的逻辑 copy 记录。
fn native_scroll_to_pending(
    scroll: PendingNativeScroll,
) -> crate::draw::backend::gpu::backend::surface::PendingScrollCopy {
    // source 是按位移移动后的视口，destination 保持原视口原点。
    crate::draw::backend::gpu::backend::surface::PendingScrollCopy {
        source: Rect::new(
            scroll.viewport.x + scroll.dx as f32,
            scroll.viewport.y + scroll.dy as f32,
            scroll.viewport.w,
            scroll.viewport.h,
        ),
        destination: Point::new(scroll.viewport.x, scroll.viewport.y),
    }
}

// 把原生 Canvas2D scroll 降低为当前 retained target 的 TextureMove。
fn lower_native_scroll_move(
    scroll: PendingNativeScroll,
    target: TextureHandle,
    // 接收显式 retained texture 的物理范围。
    extent: RhiExtent,
    logical_width: i32,
    logical_height: i32,
    scale_x: f32,
    scale_y: f32,
) -> Result<Option<TextureMove>> {
    // 复用 surface scroll 的裁剪、DPR 整数化和 memmove 方向语义。
    crate::draw::backend::gpu::backend::rhi_surface_scroll::lower_scroll_copy(
        native_scroll_to_pending(scroll),
        target,
        logical_width,
        logical_height,
        scale_x,
        scale_y,
        extent,
    )
}

// 在不触发 swapchain present 的前提下执行一条纹理搬移 boundary。
fn execute_texture_move(device: &mut dyn GraphicsDevice, movement: TextureMove) -> Result<()> {
    // 纹理搬移只属于 device，不依赖 swapchain generation。
    let mut plan = FramePlan::offscreen();
    // 按 lowering 顺序追加重叠安全的 TextureMove。
    plan.push_move(movement);
    // 只提交离屏命令，不获取或呈现 swapchain image。
    plan.execute_offscreen_on_device(device)?;
    // 丢弃只用于提交追踪的 handle，保留 typed Result 语义。
    Ok(())
}

// 构造清空后无其它绘制时使用的透明 dummy mesh。
fn empty_mixed_draw(viewport: RhiViewport) -> RhiOp {
    // 透明 SrcOver draw 不改变已经由 load action 初始化的目标。
    let vertices = std::sync::Arc::<[f32]>::from(vec![
        0.0,
        0.0,
        viewport.width,
        0.0,
        viewport.width,
        viewport.height,
        0.0,
        0.0,
        viewport.width,
        viewport.height,
        0.0,
        viewport.height,
    ]);
    RhiOp::Solid(RhiSolidMesh {
        vertices,
        rgba: [0.0; 4],
        scissor: None,
    })
}

// 为 NativeGpuCanvas2D 提供保序混合 RHI 提交入口。
impl NativeGpuCanvas2D {
    // 为没有绘制命令的新 retained surface 提交一次透明初始化 pass。
    pub(crate) fn submit_rhi_clear_only(
        // 借用通用 renderer 的固定资源与计划执行能力。
        &self,
        // 接收通用 renderer cache。
        renderer: &mut RhiRenderer,
        // 接收只允许资源、命令与 submit 的 Device 角色。
        device: &mut dyn GraphicsDevice,
        // 接收 retained texture 的物理范围。
        extent: RhiExtent,
        // 接收必须为 Clear 的首段 load action。
        load: LoadAction,
        // 接收唯一 retained texture target。
        target: TextureHandle,
        // 返回资源、pass 或 submit 的真实 typed 结果。
    ) -> Result<(), Error> {
        // 使用显式纹理范围和逻辑画布尺寸计算物理 viewport。
        let (viewport, _, _) = super::rhi_physical_geometry(extent, self.surface_w, self.surface_h);
        // 透明 SrcOver dummy 只触发 load clear，不改变初始化后的颜色。
        let operations = [empty_mixed_draw(viewport)];
        // retained 初始化只把 Device 与显式纹理交给 Renderer。
        let frame = RhiRendererFrame::offscreen(device, target);
        // 复用通用混合计划执行器，禁止为清空重新引入 adapter 高层入口。
        renderer.execute_ops(
            // 传入无法携带 present damage 的 Offscreen 帧。
            frame,
            // 使用当前 drawable 的物理 viewport。
            viewport,
            // 由调用方固定透明清空颜色。
            load,
            // 透明 dummy 保证计划具有一个合法 draw packet。
            &operations,
        )
    }

    // 尝试把一整个无 soft 内容队列降低为单个 painter-order FramePlan。
    pub(crate) fn submit_rhi_mixed(
        &self,
        renderer: &mut RhiRenderer,
        // 借用只允许资源、命令与 submit 的 Device 角色。
        device: &mut dyn GraphicsDevice,
        // 接收 retained texture 的物理范围。
        extent: RhiExtent,
        load: LoadAction,
        // 接收唯一 retained texture target。
        target: TextureHandle,
    ) -> Result<bool, Error> {
        // retained texture 使用显式 extent 计算物理几何。
        let (viewport, scale_x, scale_y) =
            super::rhi_physical_geometry(extent, self.surface_w, self.surface_h);
        // 复用显式 geometry 入口完成真正的保序 lowering。
        self.submit_rhi_mixed_for_geometry(
            renderer, device, extent, load, target, viewport, scale_x, scale_y, false,
        )
    }

    // 允许主 retained surface 先提交 native 前缀，随后由 owner 合成 soft tile。
    pub(crate) fn submit_rhi_native_prefix(
        &self,
        renderer: &mut RhiRenderer,
        // 借用只允许资源、命令与 submit 的 Device 角色。
        device: &mut dyn GraphicsDevice,
        // 接收 retained texture 的物理范围。
        extent: RhiExtent,
        load: LoadAction,
        // 接收唯一 retained texture target。
        target: TextureHandle,
    ) -> Result<bool, Error> {
        // retained texture 与普通 Picture 共用同一套物理 lowering 几何。
        let (viewport, scale_x, scale_y) =
            super::rhi_physical_geometry(extent, self.surface_w, self.surface_h);
        // 只放宽 soft 前缀检查，不放宽 scroll/未验证操作的原子回退。
        self.submit_rhi_mixed_for_geometry(
            renderer, device, extent, load, target, viewport, scale_x, scale_y, true,
        )
    }

    // 在调用方已经知道 target extent 时执行保序混合 RHI lowering。
    pub(crate) fn submit_rhi_mixed_for_geometry(
        &self,
        renderer: &mut RhiRenderer,
        // 借用只允许资源、命令与 submit 的 Device 角色。
        device: &mut dyn GraphicsDevice,
        // 接收当前显式纹理目标的物理范围。
        extent: RhiExtent,
        load: LoadAction,
        // 接收唯一离屏纹理目标。
        target: TextureHandle,
        viewport: crate::platform::presentation::rhi::RhiViewport,
        scale_x: f32,
        scale_y: f32,
        allow_soft_prefix: bool,
    ) -> Result<bool, Error> {
        // 没有 native 队列时不能由该入口伪造一次空绘制。
        if self.pending_native.is_empty() {
            // 保持旧兼容路径的顺序和回退行为。
            return Ok(false);
        }
        // 普通 mixed 入口不能在消费 native 后丢失尚未合成的 soft 内容。
        if self.soft_has_content && !allow_soft_prefix {
            // 主 retained owner 使用专用 prefix 入口完成后续 soft 合成。
            return Ok(false);
        }
        // scroll 出现在 soft 内容之后时，当前 staging 没有记录可重排的 CPU 边界。
        if self.soft_has_content
            && self
                .pending_native
                .iter()
                .any(|operation| matches!(operation, PendingNativeOp::ScrollCopy(_)))
        {
            // 保留 typed fallback，禁止把 scroll 错放到 soft 前面。
            return Ok(false);
        }
        // 按 scroll boundary 把 pending queue 切成多个连续 draw segment。
        let mut segments: Vec<(Option<PendingNativeScroll>, Vec<RhiOp>)> = vec![(None, Vec::new())];
        // 逐项 lowering，任何未覆盖语义都回到兼容 presenter。
        for operation in &self.pending_native {
            // 目标相关 scroll 不能进入普通 draw operation，必须切开 painter order。
            if let PendingNativeOp::ScrollCopy(scroll) = operation {
                segments.push((Some(*scroll), Vec::new()));
                continue;
            }
            // 空 scissor 表示裁剪区域完全不可见（clip 为空），该操作是安全 no-op：
            // 兼容路径同样不绘制，RHI 路径必须跳过而不是原子拒绝整帧。
            let (_, _, scissor_w, scissor_h) = operation.scissor();
            if scissor_w <= 0 || scissor_h <= 0 {
                continue;
            }
            // 保留原始 pending queue 的 painter order。
            let Some(operation) = lower_operation(operation, device, extent, scale_x, scale_y)
            else {
                // 不在未验证的混合 pass 中伪造成功。
                return Ok(false);
            };
            // 追加当前连续 segment 的已验证 RHI 操作。
            let Some((_, ops)) = segments.last_mut() else {
                // 缺少段说明内部状态被破坏，附带当前段数便于定位。
                panic!(
                    "mixed RHI lowering always has one segment (segments={})",
                    segments.len()
                );
            };
            ops.push(operation);
        }
        // 预先降低所有 scroll，任何一条失败都不触碰后续 draw submit。
        let mut movements = Vec::with_capacity(segments.len());
        for (scroll, _) in &segments {
            let Some(scroll) = scroll else {
                movements.push(None);
                continue;
            };
            // submit 契约已经直接提供显式纹理句柄。
            let target_texture = target;
            match lower_native_scroll_move(
                *scroll,
                target_texture,
                extent,
                self.surface_w,
                self.surface_h,
                scale_x,
                scale_y,
            ) {
                // 记录有效搬移；空 viewport 是安全 no-op。
                Ok(movement) => movements.push(movement),
                // 不能无损表达时整条 native queue 原子回退。
                Err(error) if error.code() == crate::core::Errc::NotImplemented => {
                    return Ok(false);
                }
                // 其它几何或资源错误保持 typed error。
                Err(error) => return Err(error),
            }
        }
        // 新 target 首条是 Clear 且 scroll 之前没有 draw 时，补透明 dummy 初始化。
        if matches!(load, LoadAction::Clear(_))
            && segments
                .first()
                .is_some_and(|(_, operations)| operations.is_empty())
        {
            segments[0].1.push(empty_mixed_draw(viewport));
        }
        // 依次执行 segment draw 与前置 TextureMove，严格保留原始 painter order。
        let mut target_initialized = false;
        for ((_, operations), movement) in segments.iter().zip(movements.iter()) {
            // scroll boundary 必须发生在后续 segment draw 之前。
            if let Some(movement) = movement {
                // scroll path 只使用显式 Device 与 retained texture。
                execute_texture_move(device, *movement)?;
                // move 本身已经建立了后续 pass 的 Load 基线。
                target_initialized = true;
            }
            // 空 segment 不需要制造 draw；move boundary 已经独立提交。
            if operations.is_empty() {
                continue;
            }
            // 首个实际 draw 使用调用方的 load，其余 segment 保留旧颜色。
            let segment_load = if target_initialized {
                LoadAction::Load
            } else {
                load
            };
            // mixed lowering 只能构造显式纹理 Offscreen 帧。
            let frame = RhiRendererFrame::offscreen(device, target);
            // 当前 segment 仍由通用 renderer 负责 resource/pass/draw ABI。
            renderer.execute_ops(frame, viewport, segment_load, operations)?;
            // 后续 draw 只能 Load，不能再次清除 retained target。
            target_initialized = true;
        }
        // 纯 no-op scroll 不需要 GPU 命令，但仍应被完整消费而不是错误回退。
        Ok(target_initialized || segments.iter().all(|(_, operations)| operations.is_empty()))
    }
}
