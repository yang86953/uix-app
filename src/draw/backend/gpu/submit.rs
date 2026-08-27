//! GPU-native 队列到通用 RHI 的提交实现 — gpu 子模块。

// 引入当前 canvas owner 与软资源闲置回收阈值。
use super::{NativeGpuCanvas2D, SOFT_FALLBACK_IDLE_PRESENT_GRACE};

use crate::core::{Errc, Error};
// 引入通用 RHI lowering 的 mesh 载荷和 renderer。
use crate::draw::backend::rhi_renderer::{
    RhiCoverageQuad, RhiRenderer, RhiRendererFrame, RhiShadow, RhiShapeRect, RhiSolidMesh,
    RhiTexturedQuad,
};
use crate::draw::geometry::types::BlendMode;
// 引入 Device-only lowering 所需的目标、extent、load 和 viewport 类型。
use crate::platform::presentation::rhi::{
    GraphicsDevice, LoadAction, RhiExtent, RhiScissor, RhiViewport, TextureHandle,
};

// 将保序混合 RHI lowering 拆到独立文件，避免继续膨胀提交模块。
#[path = "rhi_lowering.rs"]
mod lowering;

// 将渐变队列提交拆到独立文件，保持提交模块的单文件行数边界。
#[path = "rhi_gradient_submit.rs"]
mod gradient_submit;

// 由逻辑 canvas 尺寸和显式目标 extent 推导物理 lowering 比例。
pub(crate) fn rhi_physical_geometry(
    // 接收本次 Drawing 目标的物理范围，不取得 Surface 生命周期。
    extent: RhiExtent,
    // 接收 Drawing 语义中的逻辑宽度。
    logical_width: i32,
    // 接收 Drawing 语义中的逻辑高度。
    logical_height: i32,
) -> (RhiViewport, f32, f32) {
    // 计算 x/y 两个轴的逻辑到物理比例，覆盖 mixed-DPI 的非对称变化。
    let scale_x = extent.width as f32 / logical_width.max(1) as f32;
    let scale_y = extent.height as f32 / logical_height.max(1) as f32;
    // 返回 RHI 使用的物理 viewport 和两轴比例。
    (
        RhiViewport {
            width: extent.width as f32,
            height: extent.height as f32,
        },
        scale_x,
        scale_y,
    )
}

// 把逻辑 scissor 转成已经裁到 surface 范围内的物理 scissor。
fn rhi_physical_scissor(
    // 接收 Drawing 已经计算的逻辑裁剪。
    scissor: (i32, i32, i32, i32),
    // 接收当前目标的水平逻辑到物理比例。
    scale_x: f32,
    // 接收当前目标的垂直逻辑到物理比例。
    scale_y: f32,
    // 接收本次目标的物理范围，禁止从组合 Surface 隐式读取。
    extent: RhiExtent,
) -> Option<RhiScissor> {
    // 逻辑裁剪已经由 canvas 计算，此处只拒绝异常输入。
    if scissor.2 <= 0 || scissor.3 <= 0 || !scale_x.is_finite() || !scale_y.is_finite() {
        // 空裁剪不能被误译成“无裁剪”。
        return None;
    }
    // 用绝对远端换算并夹到 drawable，避免缩放后的宽高少一列。
    let x0 = (scissor.0 as f32 * scale_x)
        .floor()
        .max(0.0)
        .min(extent.width as f32);
    let y0 = (scissor.1 as f32 * scale_y)
        .floor()
        .max(0.0)
        .min(extent.height as f32);
    let x1 = (scissor.0.saturating_add(scissor.2) as f32 * scale_x)
        .ceil()
        .max(0.0)
        .min(extent.width as f32);
    let y1 = (scissor.1.saturating_add(scissor.3) as f32 * scale_y)
        .ceil()
        .max(0.0)
        .min(extent.height as f32);
    // 完全不可见的操作应回到兼容路径，由原有路径决定 no-op 语义。
    if x1 <= x0 || y1 <= y0 {
        // 不返回 full-surface 的 None，避免越界绘制。
        return None;
    }
    // 物理坐标已经夹到 u32 extent，转换为共享 RHI 可接受的 i32。
    // 防止异常超大 drawable 在窄的 native RECT 类型中回绕。
    let max_i32 = i32::MAX as f32;
    // 返回已经限制到共享原生整数坐标范围的裁剪。
    Some(RhiScissor {
        x: x0.min(max_i32) as i32,
        y: y0.min(max_i32) as i32,
        width: (x1 - x0).min(max_i32) as i32,
        height: (y1 - y0).min(max_i32) as i32,
    })
}

// 把已经完成逻辑几何 lowering 的 xy 顶点转换为物理坐标。
fn scale_rhi_vertices(
    vertices: &[f32],
    scale_x: f32,
    scale_y: f32,
) -> Option<std::sync::Arc<[f32]>> {
    // xy 顶点必须成对出现，且每个坐标都能稳定缩放。
    if vertices.len() < 6 || !vertices.len().is_multiple_of(2) {
        // 将不完整的 mesh 留给兼容路径处理。
        return None;
    }
    // 分配与原 mesh 等长的物理顶点载荷。
    let mut scaled = Vec::with_capacity(vertices.len());
    // 逐对转换 x/y 坐标。
    for pair in vertices.chunks_exact(2) {
        // 拒绝 NaN/无穷，避免 adapter 产生未定义裁剪。
        if !pair[0].is_finite() || !pair[1].is_finite() {
            // 将异常 mesh 留给兼容路径错误处理。
            return None;
        }
        // 计算物理 x 坐标并检查乘法没有溢出。
        let x = pair[0] * scale_x;
        // 计算物理 y 坐标并检查乘法没有溢出。
        let y = pair[1] * scale_y;
        // 异常结果交回兼容路径。
        if !x.is_finite() || !y.is_finite() {
            // 不让 NaN 进入底层 viewport 变换。
            return None;
        }
        // 写入物理 x 坐标。
        scaled.push(x);
        // 写入物理 y 坐标。
        scaled.push(y);
    }
    // 返回不可变共享物理载荷。
    Some(std::sync::Arc::<[f32]>::from(scaled))
}

// 将任意设备四角统一转换到当前 RHI 的物理坐标。
fn scale_rhi_corners(corners: [[f32; 2]; 4], scale_x: f32, scale_y: f32) -> Option<[[f32; 2]; 4]> {
    // 逐角缩放并拒绝非有限坐标，避免 shader 收到未定义几何。
    let scaled = corners.map(|corner| [corner[0] * scale_x, corner[1] * scale_y]);
    if scaled.iter().flatten().any(|value| !value.is_finite()) {
        return None;
    }
    Some(scaled)
}

// 按旧 queue 的几何平均规则把逻辑圆角半径降低到物理 RHI 空间。
fn scale_rhi_shape_radius(radius: [f32; 4], scale_x: f32, scale_y: f32) -> Option<[f32; 4]> {
    // 各向异性缩放下沿用旧 SDF 的等效圆角半径定义。
    let scale = (scale_x.abs() * scale_y.abs()).sqrt();
    // 缩放因子必须是可表示的正有限值。
    if !scale.is_finite() || scale <= 0.0 {
        // 异常几何交回兼容路径。
        return None;
    }
    // 逐角缩放并检查乘法结果。
    let mut scaled = [0.0; 4];
    // 保留四个角的 painter 语义顺序。
    for (index, value) in radius.into_iter().enumerate() {
        // 负半径不属于 RECT_HLSL 的合法输入。
        if !value.is_finite() || value < 0.0 {
            // 让旧路径处理异常输入。
            return None;
        }
        // 计算物理半径。
        scaled[index] = value * scale;
        // 不允许溢出为无穷。
        if !scaled[index].is_finite() {
            // 让旧路径处理异常输入。
            return None;
        }
    }
    // 返回物理空间圆角半径。
    Some(scaled)
}

impl NativeGpuCanvas2D {
    // 尝试把纯 solid mesh/无圆角 rect 队列交给通用 FramePlan RHI lowering。
    pub(crate) fn submit_rhi_solid(
        &self,
        renderer: &mut RhiRenderer,
        // 借用只允许资源、命令与 submit 的 Device 角色。
        device: &mut dyn GraphicsDevice,
        // 接收 retained texture 的物理范围。
        extent: RhiExtent,
        load: LoadAction,
        // 指定本次 solid 计划唯一允许写入的离屏纹理。
        target: TextureHandle,
    ) -> Result<bool, Error> {
        // soft 内容与 RHI native 几何不能在这条纵切中交错提交。
        if self.soft_has_content || self.pending_native.is_empty() {
            // 返回 false 让兼容路径保持原有 painter-order 语义。
            return Ok(false);
        }
        // 预先分配同一顺序的 RHI mesh 载荷。
        let mut meshes = Vec::with_capacity(self.pending_native.len());
        // 从显式纹理范围推导物理 viewport 和两轴缩放。
        let (viewport, scale_x, scale_y) =
            rhi_physical_geometry(extent, self.surface_w, self.surface_h);
        // 逐项检查当前队列是否属于已经迁移的几何子集。
        for operation in &self.pending_native {
            // 把现有逻辑 scissor 转成薄 RHI 的物理矩形。
            let Some(scissor) = rhi_physical_scissor(operation.scissor(), scale_x, scale_y, extent)
            else {
                // 返回 false 而不是伪造一帧成功。
                return Ok(false);
            };
            // 仅迁移已经完成三角 lowering 的 mesh，以及无圆角 rect。
            let mesh = match operation {
                // 直接复用通用 GPU queue 生成的设备空间三角形。
                super::pending::PendingNativeOp::SolidMesh(mesh) => {
                    // 缩放失败时回到兼容路径，避免在无 Result 的 Canvas 边界伪造成功。
                    let Some(vertices) =
                        scale_rhi_vertices(mesh.mesh.vertices.as_ref(), scale_x, scale_y)
                    else {
                        // 让旧路径保留原有错误与回退策略。
                        return Ok(false);
                    };
                    // 组装物理坐标 solid mesh。
                    RhiSolidMesh {
                        vertices,
                        rgba: mesh.mesh.rgba,
                        scissor: Some(scissor),
                    }
                }
                // 无圆角矩形可以在通用层稳定展开成两个三角形。
                super::pending::PendingNativeOp::SolidRect(rect)
                    if rect.rect.radius.iter().all(|radius| *radius == 0.0) =>
                {
                    let value = rect.rect;
                    RhiSolidMesh {
                        vertices: std::sync::Arc::<[f32]>::from(vec![
                            value.x * scale_x,
                            value.y * scale_y,
                            (value.x + value.w) * scale_x,
                            value.y * scale_y,
                            (value.x + value.w) * scale_x,
                            (value.y + value.h) * scale_y,
                            value.x * scale_x,
                            value.y * scale_y,
                            (value.x + value.w) * scale_x,
                            (value.y + value.h) * scale_y,
                            value.x * scale_x,
                            (value.y + value.h) * scale_y,
                        ]),
                        rgba: value.rgba,
                        scissor: Some(scissor),
                    }
                }
                // 圆角、描边、字形、渐变、图片等仍由兼容路径处理。
                _ => return Ok(false),
            };
            // 保持 pending queue 的 painter order。
            meshes.push(mesh);
        }
        // Device-only lowering 只能构造显式纹理 Offscreen 帧。
        let frame = RhiRendererFrame::offscreen(device, target);
        // 由通用 renderer 生成并执行离屏 FramePlan。
        renderer.execute_solid_meshes(frame, viewport, load, &meshes)?;
        // 告知调用方本次队列已经通过 RHI present 成功。
        Ok(true)
    }

    // 尝试把轴对齐圆角/描边矩形队列交给通用 FramePlan lowering。
    pub(crate) fn submit_rhi_shapes(
        &self,
        renderer: &mut RhiRenderer,
        // 借用只允许资源、命令与 submit 的 Device 角色。
        device: &mut dyn GraphicsDevice,
        // 接收 retained texture 的物理范围。
        extent: RhiExtent,
        load: LoadAction,
        // 指定本次 shape 计划唯一允许写入的离屏纹理。
        target: TextureHandle,
    ) -> Result<bool, Error> {
        // soft 内容与 RHI shape 不能在这条纵切中交错提交。
        if self.soft_has_content || self.pending_native.is_empty() {
            // 返回 false 让兼容路径保持原有 painter-order 语义。
            return Ok(false);
        }
        // 从显式纹理范围推导物理 viewport 和两轴缩放。
        let (viewport, scale_x, scale_y) =
            rhi_physical_geometry(extent, self.surface_w, self.surface_h);
        // 预先分配同一顺序的 shape 载荷。
        let mut rects = Vec::with_capacity(self.pending_native.len());
        // 逐项确认当前队列只包含 shape rect/stroke rect。
        for operation in &self.pending_native {
            // 把逻辑裁剪转换为物理裁剪。
            let Some(scissor) = rhi_physical_scissor(operation.scissor(), scale_x, scale_y, extent)
            else {
                // 空裁剪保留原有 no-op 语义。
                return Ok(false);
            };
            // 只迁移轴对齐圆角填充和描边矩形。
            let rect = match operation {
                // 填充矩形沿用 queue 已规整的圆角与直通颜色。
                super::pending::PendingNativeOp::SolidRect(rect) => {
                    // Additive 必须由 mixed renderer 选择独立 pipeline，不能进入 SrcOver 快路。
                    if rect.additive {
                        // 返回 false 时尚未提交任何 shape，mixed/兼容边界仍可安全接管。
                        return Ok(false);
                    }
                    let value = rect.rect;
                    let Some(radius) = scale_rhi_shape_radius(value.radius, scale_x, scale_y)
                    else {
                        // 异常半径交回兼容路径。
                        return Ok(false);
                    };
                    RhiShapeRect {
                        x: value.x * scale_x,
                        y: value.y * scale_y,
                        w: value.w * scale_x,
                        h: value.h * scale_y,
                        rgba: value.rgba,
                        radius,
                        half_stroke: 0.0,
                        scissor: Some(scissor),
                    }
                }
                // 描边矩形把完整线宽转换为 shader 所需的半宽。
                super::pending::PendingNativeOp::StrokeRect(rect) => {
                    // Additive 必须由 mixed renderer 选择独立 pipeline，不能进入 SrcOver 快路。
                    if rect.additive {
                        // 尚未提交任何 shape，mixed/兼容边界仍可原子接管。
                        return Ok(false);
                    }
                    let value = rect.rect;
                    let Some(radius) = scale_rhi_shape_radius(value.radius, scale_x, scale_y)
                    else {
                        // 异常半径交回兼容路径。
                        return Ok(false);
                    };
                    let half_stroke =
                        value.line_width * 0.5 * (scale_x.abs() * scale_y.abs()).sqrt();
                    if !half_stroke.is_finite() || half_stroke < 0.0 {
                        // 异常线宽交回兼容路径。
                        return Ok(false);
                    }
                    RhiShapeRect {
                        x: value.x * scale_x,
                        y: value.y * scale_y,
                        w: value.w * scale_x,
                        h: value.h * scale_y,
                        rgba: value.rgba,
                        radius,
                        half_stroke,
                        scissor: Some(scissor),
                    }
                }
                // 其他 UI 语义不能被矩形 SDF shader 偷换。
                _ => return Ok(false),
            };
            // 保持 pending queue 的 painter order。
            rects.push(rect);
        }
        // Device-only lowering 只能构造显式纹理 Offscreen 帧。
        let frame = RhiRendererFrame::offscreen(device, target);
        // 由通用 renderer 生成并执行离屏 FramePlan。
        renderer.execute_shape_rects(frame, viewport, load, &rects)?;
        // 告知调用方本次队列已经通过 RHI present 成功。
        Ok(true)
    }

    // 尝试把仿射阴影队列交给通用 FramePlan lowering。
    pub(crate) fn submit_rhi_shadows(
        &self,
        renderer: &mut RhiRenderer,
        // 借用只允许资源、命令与 submit 的 Device 角色。
        device: &mut dyn GraphicsDevice,
        // 接收 retained texture 的物理范围。
        extent: RhiExtent,
        load: LoadAction,
        // 指定本次 shadow 计划唯一允许写入的离屏纹理。
        target: TextureHandle,
    ) -> Result<bool, Error> {
        // soft 内容与 RHI shadow 不能在这条纵切中交错提交。
        if self.soft_has_content || self.pending_native.is_empty() {
            // 返回 false 让兼容路径保持原有 painter-order 语义。
            return Ok(false);
        }
        // 从显式纹理范围推导物理 viewport 和两轴缩放。
        let (viewport, scale_x, scale_y) =
            rhi_physical_geometry(extent, self.surface_w, self.surface_h);
        // 预先分配同一顺序的 shadow 载荷。
        let mut shadows = Vec::with_capacity(self.pending_native.len());
        // 逐项确认当前队列只包含 box shadow。
        for operation in &self.pending_native {
            // 只有 box shadow 操作可以进入 shadow SDF pipeline。
            let super::pending::PendingNativeOp::BoxShadow(shadow) = operation else {
                // 其他高层操作不能被阴影 shader 偷换。
                return Ok(false);
            };
            // 旋转/剪切四边形保留为真实设备四角。
            let corners = match scale_rhi_corners(shadow.shadow.corners, scale_x, scale_y) {
                // 继续校验缩放后的四边形。
                Some(corners) if super::geometry::convex_quad_is_valid(&corners) => corners,
                // 不把真实仿射几何错误地压平为 AABB。
                _ => {
                    // 交回原有兼容路径。
                    return Ok(false);
                }
            };
            // 把逻辑裁剪转换为物理裁剪。
            let Some(scissor) = rhi_physical_scissor(shadow.scissor, scale_x, scale_y, extent)
            else {
                // 空裁剪保留原有 no-op 语义。
                return Ok(false);
            };
            // 把旧 queue 已完成的 body/offset/blur/radius 统一转换为物理空间。
            let value = shadow.shadow;
            let Some(radius) = scale_rhi_shape_radius(value.radius, scale_x, scale_y) else {
                // 异常半径交回兼容路径。
                return Ok(false);
            };
            let blur_x = value.blur_x * scale_x.abs();
            let blur_y = value.blur_y * scale_y.abs();
            let offset_x = value.offset_x * scale_x;
            let offset_y = value.offset_y * scale_y;
            // 拒绝缩放后产生的异常阴影参数。
            if !blur_x.is_finite()
                || !blur_y.is_finite()
                || !offset_x.is_finite()
                || !offset_y.is_finite()
            {
                // 让兼容路径保留原有 typed error 或回退语义。
                return Ok(false);
            }
            // 组装保留仿射四角的物理 shadow。
            shadows.push(RhiShadow {
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
            });
        }
        // Device-only lowering 只能构造显式纹理 Offscreen 帧。
        let frame = RhiRendererFrame::offscreen(device, target);
        // 由通用 renderer 生成并执行离屏 FramePlan。
        renderer.execute_shadows(frame, viewport, load, &shadows)?;
        // 告知调用方本次队列已经通过 RHI present 成功。
        Ok(true)
    }

    // 尝试把轴对齐 R8 glyph coverage 队列交给通用 FramePlan lowering。
    pub(crate) fn submit_rhi_glyphs(
        &self,
        renderer: &mut RhiRenderer,
        // 借用只允许资源、命令与 submit 的 Device 角色。
        device: &mut dyn GraphicsDevice,
        // 接收 retained texture 的物理范围。
        extent: RhiExtent,
        load: LoadAction,
        // 指定本次 glyph 计划唯一允许写入的离屏纹理。
        target: TextureHandle,
    ) -> Result<bool, Error> {
        // soft 内容与 RHI coverage 不能在这条纵切中交错提交。
        if self.soft_has_content || self.pending_native.is_empty() {
            // 返回 false 让兼容路径保持原有 painter-order 语义。
            return Ok(false);
        }
        // 从显式纹理范围推导物理 viewport 和两轴缩放。
        let (viewport, scale_x, scale_y) =
            rhi_physical_geometry(extent, self.surface_w, self.surface_h);
        // 预先分配同一顺序的 coverage quad 载荷。
        let mut quads = Vec::with_capacity(self.pending_native.len());
        // 逐项确认当前队列是轴对齐 R8 coverage 子集。
        for operation in &self.pending_native {
            // 只有 glyph 操作可以进入 coverage pipeline。
            let super::pending::PendingNativeOp::Glyph(glyph) = operation else {
                // 其他高层操作不能被 glyph shader 偷换。
                return Ok(false);
            };
            // 轮廓 mesh/MSDF 仍由后续 MSDF pipeline 处理，R8 coverage 可保留真实四角。
            if glyph.glyph.outline_mesh.is_some() {
                // 不把 MSDF 轮廓错误地当作 R8 coverage。
                return Ok(false);
            }
            // 把逻辑裁剪转换为物理裁剪。
            let Some(scissor) = rhi_physical_scissor(glyph.scissor, scale_x, scale_y, extent)
            else {
                // 空裁剪保留原有 no-op 语义。
                return Ok(false);
            };
            // 只接受紧密 R8 coverage，避免旧 atlas 的额外尾部字节被误上传。
            let pixel_count = (glyph.glyph.cov_w as usize).checked_mul(glyph.glyph.cov_h as usize);
            if glyph.glyph.cov_w == 0
                || glyph.glyph.cov_h == 0
                || pixel_count != Some(glyph.glyph.coverage.len())
            {
                // 交回兼容路径，由原有 atlas 逻辑处理异常载荷。
                return Ok(false);
            }
            // 计算物理目标矩形并拒绝异常几何。
            let x = glyph.glyph.x * scale_x;
            let y = glyph.glyph.y * scale_y;
            let w = glyph.glyph.w * scale_x;
            let h = glyph.glyph.h * scale_y;
            if !x.is_finite()
                || !y.is_finite()
                || !w.is_finite()
                || !h.is_finite()
                || w <= 0.0
                || h <= 0.0
            {
                // 让兼容路径保留原有 typed error 或 no-op 语义。
                return Ok(false);
            }
            // 组装物理坐标 coverage quad。
            quads.push(RhiCoverageQuad {
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
            });
        }
        // Device-only lowering 只能构造显式纹理 Offscreen 帧。
        let frame = RhiRendererFrame::offscreen(device, target);
        // 由通用 renderer 生成并执行离屏 FramePlan。
        renderer.execute_coverage_quads(frame, viewport, load, &quads)?;
        // 告知调用方本次队列已经通过 RHI present 成功。
        Ok(true)
    }

    // 尝试把纯图片 blit 队列交给通用 FramePlan sampled-quad lowering。
    pub(crate) fn submit_rhi_textured(
        &self,
        renderer: &mut RhiRenderer,
        // 借用只允许资源、命令与 submit 的 Device 角色。
        device: &mut dyn GraphicsDevice,
        // 接收 retained texture 的物理范围。
        extent: RhiExtent,
        load: LoadAction,
        // 指定本次图片计划唯一允许写入的离屏纹理。
        target: TextureHandle,
    ) -> Result<bool, Error> {
        // soft 内容与 RHI texture 不能在这条纵切中交错提交。
        if self.soft_has_content || self.pending_native.is_empty() {
            // 返回 false 让兼容路径保持原有 painter-order 语义。
            return Ok(false);
        }
        // 从显式纹理范围推导物理 viewport 和两轴缩放。
        let (viewport, scale_x, scale_y) =
            rhi_physical_geometry(extent, self.surface_w, self.surface_h);
        // 预先分配同一顺序的 sampled quad 载荷。
        let mut quads = Vec::with_capacity(self.pending_native.len());
        // 逐项确认当前队列是可迁移的 SrcOver image blit 子集。
        for operation in &self.pending_native {
            // 其他高层操作不能被纹理 shader 偷换。
            let super::pending::PendingNativeOp::ImageBlit(image) = operation else {
                return Ok(false);
            };
            // 缺少可选 Additive 能力时回到兼容路径，不能伪造 SrcOver 结果。
            if image.blit.additive
                // Additive 能力只能从显式 Device 角色读取。
                && !device.device_capabilities().additive_blend
            {
                // 保持旧 adapter 的能力分流和 painter-order 语义。
                return Ok(false);
            }
            // 把逻辑裁剪转换为物理裁剪。
            let Some(scissor) = rhi_physical_scissor(image.scissor, scale_x, scale_y, extent)
            else {
                // 空裁剪保留原有 no-op 语义。
                return Ok(false);
            };
            // 检查图片载荷尺寸，避免 texture 创建后才发现不一致。
            let pixel_count =
                (image.blit.pixel_w as usize).checked_mul(image.blit.pixel_h as usize);
            if image.blit.pixel_w == 0
                || image.blit.pixel_h == 0
                || pixel_count != Some(image.blit.pixels.len())
            {
                // 交回兼容路径，由原有 typed image 语义处理异常输入。
                return Ok(false);
            }
            // 保持 premultiplied 源 RGB 和 alpha 同比例降低 opacity。
            let opacity = image.blit.opacity.clamp(0.0, 1.0);
            // 将目标几何和裁剪一起转换到物理 RHI 坐标。
            quads.push(RhiTexturedQuad {
                x: image.blit.x * scale_x,
                y: image.blit.y * scale_y,
                w: image.blit.w * scale_x,
                h: image.blit.h * scale_y,
                corners: scale_rhi_corners(image.blit.corners, scale_x, scale_y).ok_or_else(
                    || {
                        Error::new(
                            Errc::InvalidArgument,
                            "RHI textured image affine corners are not finite",
                        )
                    },
                )?,
                rgba: [opacity; 4],
                additive: image.blit.additive,
                pixels: image.blit.pixels.clone(),
                pixel_w: image.blit.pixel_w,
                pixel_h: image.blit.pixel_h,
                scissor: Some(scissor),
            });
        }
        // Device-only lowering 只能构造显式纹理 Offscreen 帧。
        let frame = RhiRendererFrame::offscreen(device, target);
        // 由通用 renderer 生成并执行离屏 FramePlan。
        renderer.execute_textured_quads(frame, viewport, load, &quads)?;
        // 告知调用方本次队列已经通过 RHI present 成功。
        Ok(true)
    }

    pub(crate) fn current_blend_mode(&self) -> BlendMode {
        self.blend_mode
    }

    pub(crate) fn commit_presented_frame(&mut self) {
        self.pending_native.clear();
        if self.soft_has_content {
            self.ensure_soft().surface_mut().clear_all();
            self.soft_has_content = false;
        }
        // 成功边界已经消费当前可变 soft 段。
        self.soft_current_has_content = false;
        // 成功边界已经按 painter order 消费所有已封口段。
        self.pending_soft_segments.clear();
        // 下一次 soft 操作重新建立与当前公开 blend 状态匹配的段。
        self.soft_segment_blend = None;
        // 成功后不再让 legacy fallback 看到上一段的 Additive 标记。
        self.soft_uses_destination_blend = false;
    }

    /// Completes one successful swapchain present, then ages this canvas's idle
    /// full-size soft allocation.
    pub(crate) fn finish_presented_frame(&mut self) -> bool {
        self.commit_presented_frame();
        self.age_soft_fallback_after_present()
    }

    /// Ages an offscreen canvas at the final swapchain success boundary without
    /// committing any unflushed Picture commands.
    pub(crate) fn age_soft_fallback_after_present(&mut self) -> bool {
        if self.soft_used_since_present {
            self.soft_used_since_present = false;
            self.soft_idle_presents = 0;
            return true;
        }
        if self.soft_fallback.is_none() || self.soft_has_content || self.deferred_error.is_some() {
            return false;
        }
        self.soft_idle_presents = self.soft_idle_presents.saturating_add(1);
        if self.soft_idle_presents >= SOFT_FALLBACK_IDLE_PRESENT_GRACE {
            self.soft_fallback = None;
            self.soft_idle_presents = 0;
        }
        false
    }

    pub(super) fn release_idle_soft_fallback(&mut self) {
        if self.soft_has_content || self.soft_used_since_present || self.deferred_error.is_some() {
            return;
        }
        self.soft_fallback = None;
        self.soft_idle_presents = 0;
    }

    /// A successful Picture flush has copied every soft pixel into its durable
    /// GPU render target. Unlike the swapchain canvas, an eligible Picture is
    /// static by policy, so keeping a second full-size CPU surface for a future
    /// repaint is not worth the resident memory.
    pub(super) fn release_committed_picture_staging(&mut self) {
        if self.soft_has_content || !self.pending_native.is_empty() || self.deferred_error.is_some()
        {
            return;
        }
        self.soft_fallback = None;
        self.soft_used_since_present = false;
        self.soft_idle_presents = 0;
        self.soft_uses_destination_blend = false;
    }
}
