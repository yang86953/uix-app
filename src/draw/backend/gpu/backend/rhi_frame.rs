//! FrameEncoder 到通用 RHI 的整帧 lowering。

// 引入共享引用计数载荷，避免临时图片跨计划生命周期失效。
use std::sync::Arc;

// 引入统一错误和提交 damage 类型。
// 引入统一错误、几何和提交 damage 类型。
use crate::core::{Errc, Error, Point, PresentDamage, Rect};
// 引入绘制层颜色值。
use crate::draw::geometry::color::Color;
// 引入 FramePlan 的目标类型。
// 引入 FramePlan 的有序 pass、命令和目标类型。
use crate::draw::backend::frame_plan::{
    FramePlan, FramePlanCommand, RenderPassPlan, RenderTargetRef,
};
// 引入通用 RHI renderer 的固定语义载荷。
use crate::draw::backend::rhi_renderer::{
    RhiCoverageQuad, RhiOp, RhiShapeRect, RhiSolidMesh, RhiTexturedQuad,
};
// 引入 FrameEncoder 的有序命令和值类型。
use crate::draw::painting::{
    FrameCommand, FrameEncoder, FrameGlyphBlit, FrameImage, FrameRasterOp,
};
// 引入薄 RHI 的执行类型。
// 引入薄 RHI 的执行、目标和纹理搬移类型。
use crate::native::present::rhi::{
    GraphicsContextRhi, LoadAction, RenderTargetHandle, RhiColor, RhiExtent, RhiScissor,
    RhiViewport, TextureHandle, TextureMove,
};

// 引入当前 GPU backend。
use super::GpuBackend;

// 将 FrameEncoder RHI lowering 单元测试拆到尾部文件，保持实现文件不超过行数边界。
#[cfg(test)]
#[path = "rhi_frame_test_tail.rs"]
mod tests;

// 保存一条已经完成物理坐标 lowering 的 FrameEncoder 计划。
struct LoweredFrame {
    // 保存首条 Clear 命令映射出的 pass load action。
    clear: Option<RhiColor>,
    // 保存严格保持原始 painter order 的分段操作和纹理搬移。
    segments: Vec<LoweredFrameSegment>,
}

// 保存一个由相邻 scroll boundary 分隔的 RHI 操作片段。
struct LoweredFrameSegment {
    // 保存该片段之前必须执行的逻辑 scroll；首段为空。
    move_before: Option<FrameScrollCopy>,
    // 保存该片段内按原始顺序排列的 RHI draw 操作。
    operations: Vec<RhiOp>,
}

// 保存尚未结合当前 target extent 的逻辑 scroll 描述。
#[derive(Clone, Copy)]
struct FrameScrollCopy {
    // 保存逻辑 viewport。
    viewport: crate::draw::painting::FrameRect,
    // 保存 source 相对 viewport 的水平位移。
    dx: i32,
    // 保存 source 相对 viewport 的垂直位移。
    dy: i32,
}

// 将 8-bit straight Color 转为通用 shader 使用的直通 RGBA 浮点值。
fn color_rgba(color: Color) -> [f32; 4] {
    // RHI shader 会按 CPU 颜色规则量化并 premultiply。
    [
        color.r as f32 / 255.0,
        color.g as f32 / 255.0,
        color.b as f32 / 255.0,
        color.a as f32 / 255.0,
    ]
}

// 将清理颜色转换为 FramePlan 的浮点清理值。
fn clear_color(color: Color) -> RhiColor {
    // 保持与普通颜色 lowering 相同的 8-bit 通道边界。
    RhiColor(color_rgba(color))
}

// 验证并缩放一个逻辑矩形的坐标和尺寸。
fn scaled_rect(
    rect: crate::draw::painting::FrameRect,
    scale_x: f32,
    scale_y: f32,
) -> Option<(f32, f32, f32, f32)> {
    // 空矩形由调用方作为 no-op 处理。
    if rect.width <= 0 || rect.height <= 0 {
        return None;
    }
    // 逐轴转换到当前 RHI target 的物理空间。
    let x = rect.x as f32 * scale_x;
    let y = rect.y as f32 * scale_y;
    let width = rect.width as f32 * scale_x;
    let height = rect.height as f32 * scale_y;
    // 不把溢出的几何交给 shader。
    if !x.is_finite() || !y.is_finite() || !width.is_finite() || !height.is_finite() {
        return None;
    }
    // 当前 FrameEncoder 只允许正向物理 scale。
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    Some((x, y, width, height))
}

// 按通用 shape shader 的等效圆角规则缩放四个角半径。
fn scaled_radius(
    radius: crate::draw::painting::FrameRadius,
    scale_x: f32,
    scale_y: f32,
) -> Option<[f32; 4]> {
    // 各向异性缩放沿用现有 native queue 的几何平均规则。
    let scale = (scale_x.abs() * scale_y.abs()).sqrt();
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    // 读取已经过 FrameEncoder 构造校验的半径。
    let value = radius.to_radius();
    let mut result = [0.0; 4];
    for (slot, input) in [value.tl, value.tr, value.br, value.bl]
        .into_iter()
        .enumerate()
    {
        // 再次检查浮点乘法，防止跨 ABI 溢出。
        result[slot] = input * scale;
        if !result[slot].is_finite() || result[slot] < 0.0 {
            return None;
        }
    }
    Some(result)
}

// 把逻辑裁剪转换为物理整数 scissor，并裁到当前 target 边界。
fn scaled_scissor(
    clip: crate::draw::painting::FrameRect,
    bounds: crate::draw::painting::FrameRect,
    viewport: RhiViewport,
    scale_x: f32,
    scale_y: f32,
) -> Option<RhiScissor> {
    // 先在逻辑空间裁剪，避免负坐标转换后产生错误宽度。
    let clip = clip.intersection(bounds)?;
    let x0 = (clip.x as f32 * scale_x)
        .floor()
        .max(0.0)
        .min(viewport.width);
    let y0 = (clip.y as f32 * scale_y)
        .floor()
        .max(0.0)
        .min(viewport.height);
    let x1 = ((clip.x + clip.width) as f32 * scale_x)
        .ceil()
        .max(0.0)
        .min(viewport.width);
    let y1 = ((clip.y + clip.height) as f32 * scale_y)
        .ceil()
        .max(0.0)
        .min(viewport.height);
    // 空物理裁剪表示当前操作完全不可见。
    if !x0.is_finite() || !y0.is_finite() || !x1.is_finite() || !y1.is_finite() {
        return None;
    }
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    // 限制到原生 scissor 的有符号整数边界。
    let max_i32 = i32::MAX as f32;
    Some(RhiScissor {
        x: x0.min(max_i32) as i32,
        y: y0.min(max_i32) as i32,
        width: (x1 - x0).min(max_i32) as i32,
        height: (y1 - y0).min(max_i32) as i32,
    })
}

// 构造轴对齐 quad 的设备空间四角。
fn axis_aligned_corners(x: f32, y: f32, width: f32, height: f32) -> [[f32; 2]; 4] {
    // RHI 统一采用左上、右上、右下、左下顺序。
    [
        [x, y],
        [x + width, y],
        [x + width, y + height],
        [x, y + height],
    ]
}

// 追加一个 FrameEncoder 圆角/描边矩形到混合 RHI 队列。
fn append_shape(
    operations: &mut Vec<RhiOp>,
    rect: crate::draw::painting::FrameRect,
    color: Color,
    radius: crate::draw::painting::FrameRadius,
    half_stroke: f32,
    additive: bool,
    clip: Option<crate::draw::painting::FrameRect>,
    bounds: crate::draw::painting::FrameRect,
    viewport: RhiViewport,
    scale_x: f32,
    scale_y: f32,
) -> bool {
    // 空几何保持旧 FrameEncoder 的 no-op 语义。
    if rect.is_empty() {
        return true;
    }
    // 计算物理矩形和圆角。
    let Some((x, y, width, height)) = scaled_rect(rect, scale_x, scale_y) else {
        return false;
    };
    let Some(radius) = scaled_radius(radius, scale_x, scale_y) else {
        return false;
    };
    // 指定裁剪完全在 target 外时，当前操作不产生像素。
    let scissor = match clip {
        Some(clip) => match scaled_scissor(clip, bounds, viewport, scale_x, scale_y) {
            Some(scissor) => Some(scissor),
            None => return true,
        },
        None => None,
    };
    // 描边宽度必须能稳定进入 shape 常量。
    if !half_stroke.is_finite() || half_stroke < 0.0 {
        return false;
    }
    // 保留通用 shape shader 的单一几何实现，只由 pipeline 选择 blend 语义。
    let shape = RhiShapeRect {
        x,
        y,
        w: width,
        h: height,
        rgba: color_rgba(color),
        radius,
        half_stroke: half_stroke * (scale_x.abs() * scale_y.abs()).sqrt(),
        scissor,
    };
    // Additive 和 SrcOver 共享同一组圆角常量与裁剪规则。
    operations.push(if additive {
        RhiOp::AdditiveShape(shape)
    } else {
        RhiOp::Shape(shape)
    });
    true
}

// 从 FrameGlyphBlit 复制精确尺寸的 coverage payload。
fn copy_glyph_coverage(glyph: &FrameGlyphBlit) -> Result<Option<Arc<[u8]>>, Error> {
    // 保护宽高乘法并要求 payload 至少覆盖描述的区域。
    let count = (glyph.width() as usize)
        .checked_mul(glyph.height() as usize)
        .ok_or_else(|| {
            Error::new(
                Errc::GraphicsOutOfMemory,
                "FrameEncoder glyph extent overflows",
            )
        })?;
    if count == 0 || glyph.coverage().len() < count {
        return Ok(None);
    }
    // 完整 payload 直接共享，避免多余复制。
    if glyph.coverage().len() == count {
        return Ok(Some(Arc::clone(glyph.coverage())));
    }
    // FrameEncoder 允许尾部容量，RHI texture ABI 则要求紧密长度。
    let mut retained = Vec::new();
    retained.try_reserve_exact(count).map_err(|error| {
        Error::new(
            Errc::GraphicsOutOfMemory,
            format!("FrameEncoder glyph coverage allocation failed: {error}"),
        )
    })?;
    retained.extend_from_slice(&glyph.coverage()[..count]);
    Ok(Some(Arc::from(retained)))
}

// 复制一个 FrameImage 的整数 source crop，并保持 BGRA packed 像素布局。
fn copy_image_crop(
    image: &FrameImage,
    source: crate::draw::painting::FrameRect,
) -> Result<Option<Arc<[u32]>>, Error> {
    // 非法 source 不应让 RHI 猜测边界，交回完整兼容执行器。
    if !source.is_within(image.width(), image.height()) {
        return Ok(None);
    }
    let count = (source.width as usize)
        .checked_mul(source.height as usize)
        .ok_or_else(|| {
            Error::new(
                Errc::GraphicsOutOfMemory,
                "FrameEncoder image extent overflows",
            )
        })?;
    let mut retained = Vec::new();
    retained.try_reserve_exact(count).map_err(|error| {
        Error::new(
            Errc::GraphicsOutOfMemory,
            format!("FrameEncoder image crop allocation failed: {error}"),
        )
    })?;
    let stride = image.width() as usize;
    for row in 0..source.height as usize {
        let start = (source.y as usize + row) * stride + source.x as usize;
        retained.extend_from_slice(&image.pixels()[start..start + source.width as usize]);
    }
    Ok(Some(Arc::from(retained)))
}

// 追加一条已经验证的 FrameRasterOp，并返回是否可以保持无损 RHI 语义。
fn append_native_operation(
    operations: &mut Vec<RhiOp>,
    operation: &FrameRasterOp,
    bounds: crate::draw::painting::FrameRect,
    viewport: RhiViewport,
    scale_x: f32,
    scale_y: f32,
) -> Result<bool, Error> {
    match operation {
        // 普通矩形统一进入 shape pipeline。
        FrameRasterOp::FillRect { rect, color } => Ok(append_shape(
            operations,
            *rect,
            *color,
            crate::draw::painting::FrameRadius::zero(),
            0.0,
            false,
            None,
            bounds,
            viewport,
            scale_x,
            scale_y,
        )),
        // 圆角填充复用 shape SDF。
        FrameRasterOp::FillRoundedRect {
            rect,
            color,
            radius,
        } => Ok(append_shape(
            operations, *rect, *color, *radius, 0.0, false, None, bounds, viewport, scale_x,
            scale_y,
        )),
        // 带裁剪的圆角填充保持原始矩形几何，只改变 scissor。
        FrameRasterOp::FillRoundedRectClipped {
            rect,
            color,
            radius,
            clip,
        } => Ok(append_shape(
            operations,
            *rect,
            *color,
            *radius,
            0.0,
            false,
            Some(*clip),
            bounds,
            viewport,
            scale_x,
            scale_y,
        )),
        // 每个 glyph 保留原始顺序，覆盖率纹理由通用 RHI 管理。
        FrameRasterOp::BlitGlyphs { glyphs, clip } => {
            let Some(scissor) = scaled_scissor(*clip, bounds, viewport, scale_x, scale_y) else {
                return Ok(true);
            };
            for glyph in glyphs {
                let Some(coverage) = copy_glyph_coverage(glyph)? else {
                    return Ok(false);
                };
                let width = glyph.width() as f32 * scale_x;
                let height = glyph.height() as f32 * scale_y;
                let x = glyph.x() as f32 * scale_x;
                let y = glyph.y() as f32 * scale_y;
                if !x.is_finite()
                    || !y.is_finite()
                    || !width.is_finite()
                    || !height.is_finite()
                    || width <= 0.0
                    || height <= 0.0
                {
                    return Ok(false);
                }
                operations.push(RhiOp::Coverage(RhiCoverageQuad {
                    x,
                    y,
                    w: width,
                    h: height,
                    corners: axis_aligned_corners(x, y, width, height),
                    rgba: color_rgba(glyph.color()),
                    coverage,
                    pixel_w: glyph.width(),
                    pixel_h: glyph.height(),
                    scissor: Some(scissor),
                }));
            }
            Ok(true)
        }
        // 描边矩形与普通圆角填充共享 shape ABI。
        FrameRasterOp::StrokeRoundedRects { strokes, clip } => {
            let clip = clip.intersection(bounds);
            if clip.is_none() {
                return Ok(true);
            }
            for stroke in strokes {
                let value = stroke.rect();
                if !append_shape(
                    operations,
                    value,
                    stroke.color(),
                    stroke.radius(),
                    stroke.line_width().value() * 0.5,
                    false,
                    clip,
                    bounds,
                    viewport,
                    scale_x,
                    scale_y,
                ) {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        // Additive 矩形复用 shape SDF 与独立的加法 blend pipeline。
        FrameRasterOp::FillRectAdditive { rect, color } => Ok(append_shape(
            operations,
            *rect,
            *color,
            crate::draw::painting::FrameRadius::zero(),
            0.0,
            true,
            None,
            bounds,
            viewport,
            scale_x,
            scale_y,
        )),
        // Additive 圆角矩形保持与普通圆角相同的 SDF lowering。
        FrameRasterOp::FillRoundedRectAdditive {
            rect,
            color,
            radius,
        } => Ok(append_shape(
            operations, *rect, *color, *radius, 0.0, true, None, bounds, viewport, scale_x, scale_y,
        )),
        // ScrollCopy 仍需 surface image 级 copy，不能伪装成普通 draw。
        FrameRasterOp::ScrollCopy { .. } => Ok(false),
    }
}

// 把一条 FrameEncoder 命令流降低为单一保序 RHI operation list。
fn lower_frame_encoder(
    encoder: &FrameEncoder,
    viewport: RhiViewport,
    scale_x: f32,
    scale_y: f32,
) -> Result<Option<LoweredFrame>, Error> {
    // 编码器尺寸就是逻辑 target 边界。
    let bounds = crate::draw::painting::FrameRect::new(0, 0, encoder.width(), encoder.height());
    let mut clear = None;
    // 当前连续片段暂存尚未跨过 scroll boundary 的 draw 操作。
    let mut operations = Vec::new();
    // 保存已经完成 lowering 的连续片段。
    let mut segments = Vec::new();
    // 保存下一个片段开始前必须执行的 scroll。
    let mut pending_move = None;
    for (index, command) in encoder.commands().iter().enumerate() {
        match command {
            // FramePlan load action 只表达帧首 clear；中途 clear 必须整体回退。
            FrameCommand::Clear { color } => {
                if index != 0 || clear.is_some() {
                    return Ok(None);
                }
                clear = Some(clear_color(*color));
            }
            // native geometry 直接降低为通用 RHI operation。
            FrameCommand::Native { operation } => {
                if let FrameRasterOp::ScrollCopy { viewport, dx, dy } = operation {
                    // 先封存 scroll 之前的操作，保持 destination-dependent 顺序。
                    segments.push(LoweredFrameSegment {
                        move_before: pending_move.take(),
                        operations,
                    });
                    // 为 scroll 之后的操作创建新的连续片段。
                    operations = Vec::new();
                    // 保存逻辑 scroll，物理坐标在 target 代际已确定后再计算。
                    pending_move = Some(FrameScrollCopy {
                        viewport: *viewport,
                        dx: *dx,
                        dy: *dy,
                    });
                } else if !append_native_operation(
                    &mut operations,
                    operation,
                    bounds,
                    viewport,
                    scale_x,
                    scale_y,
                )? {
                    // 任一普通操作不能无损表达时整条 encoder 原子回退。
                    return Ok(None);
                }
            }
            // CPU segment 作为一条 premultiplied texture quad 上传。
            FrameCommand::CpuSegment { image, src, dst } => {
                let Some(pixels) = copy_image_crop(image, *src)? else {
                    return Ok(None);
                };
                if let Some((x, y, width, height)) = scaled_rect(*dst, scale_x, scale_y) {
                    operations.push(RhiOp::Textured(RhiTexturedQuad {
                        x,
                        y,
                        w: width,
                        h: height,
                        corners: axis_aligned_corners(x, y, width, height),
                        rgba: [1.0; 4],
                        additive: false,
                        pixels,
                        pixel_w: src.width as u32,
                        pixel_h: src.height as u32,
                        scissor: None,
                    }));
                } else if !dst.is_empty() {
                    return Ok(None);
                }
            }
            // Picture blit 使用相同纹理 pipeline，并保留 opacity/additive 事实。
            FrameCommand::PictureBlit {
                image,
                src,
                dst,
                opacity,
                additive,
            } => {
                if opacity.is_transparent() {
                    continue;
                }
                let Some(pixels) = copy_image_crop(image, *src)? else {
                    return Ok(None);
                };
                let x = dst.x() * scale_x;
                let y = dst.y() * scale_y;
                let width = dst.width() * scale_x;
                let height = dst.height() * scale_y;
                if !x.is_finite()
                    || !y.is_finite()
                    || !width.is_finite()
                    || !height.is_finite()
                    || width <= 0.0
                    || height <= 0.0
                {
                    return Ok(None);
                }
                operations.push(RhiOp::Textured(RhiTexturedQuad {
                    x,
                    y,
                    w: width,
                    h: height,
                    corners: axis_aligned_corners(x, y, width, height),
                    rgba: [opacity.value(); 4],
                    additive: *additive,
                    pixels,
                    pixel_w: src.width as u32,
                    pixel_h: src.height as u32,
                    scissor: None,
                }));
            }
        }
    }
    // 封存最后一个连续片段及其前置 scroll。
    segments.push(LoweredFrameSegment {
        move_before: pending_move,
        operations,
    });
    // 空命令流仍需可执行的计划；调用方会为片段补透明 dummy draw。
    Ok(Some(LoweredFrame { clear, segments }))
}

// 构造清空后无其它绘制时使用的透明 dummy mesh。
fn empty_frame_draw(viewport: RhiViewport) -> RhiOp {
    // 透明 SrcOver draw 不改变已经由 load action 初始化的目标。
    let vertices = Arc::<[f32]>::from(vec![
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

// 把 FrameEncoder 的 viewport scroll 转成共用 surface copy 记录。
fn frame_scroll_to_pending(scroll: FrameScrollCopy) -> super::surface::PendingScrollCopy {
    // 复用 CPU 参考执行器的 source = viewport + delta 规则。
    let source_x = scroll.viewport.x.saturating_add(scroll.dx);
    // 复用 CPU 参考执行器的 source 顶部规则。
    let source_y = scroll.viewport.y.saturating_add(scroll.dy);
    // 把整数 FrameRect 变成通用逻辑几何。
    let source = Rect::new(
        source_x as f32,
        source_y as f32,
        scroll.viewport.width as f32,
        scroll.viewport.height as f32,
    );
    // 目标保持 viewport 原点，不把 copy 方向误写成反向平移。
    let destination = Point::new(scroll.viewport.x as f32, scroll.viewport.y as f32);
    // 返回与 NativeGpuDrawSurface 相同的 memmove 描述。
    super::surface::PendingScrollCopy {
        source,
        destination,
    }
}

// 把一个逻辑 FrameEncoder scroll 降低为目标纹理上的物理 TextureMove。
fn lower_frame_scroll_move(
    scroll: FrameScrollCopy,
    target: TextureHandle,
    logical_width: i32,
    logical_height: i32,
    scale_x: f32,
    scale_y: f32,
    extent: RhiExtent,
) -> Result<Option<TextureMove>, Error> {
    // 让 surface pending scroll 与 FrameEncoder scroll 共享同一套裁剪规则。
    super::rhi_surface_scroll::lower_scroll_copy(
        frame_scroll_to_pending(scroll),
        target,
        logical_width,
        logical_height,
        scale_x,
        scale_y,
        extent,
    )
}

// 在不触发 swapchain present 的前提下执行一条纹理搬移 boundary。
fn execute_frame_texture_move(
    context: &mut dyn GraphicsContextRhi,
    target: RenderTargetHandle,
    viewport: RhiViewport,
    movement: TextureMove,
) -> Result<(), Error> {
    // move 后的空 pass 保留 target 状态并满足 FramePlan 的非空 pass 契约。
    let mut pass = RenderPassPlan::new(RenderTargetRef::Texture(target), LoadAction::Load);
    // 明确设置当前目标的物理 viewport，避免 adapter 沿用上一 pass 状态。
    pass.push(FramePlanCommand::SetViewport(viewport));
    // 计划使用当前 owner context 的 surface generation。
    let mut plan = FramePlan::new(context.token(), PresentDamage::Full);
    // 先执行重叠安全的 TextureMove，再进入空 load pass。
    plan.push_move(movement);
    // 追加 move boundary 的 target pass。
    plan.push_pass(pass);
    // 只提交离屏命令，不获取或呈现 swapchain image。
    plan.execute_offscreen_on_context(context)?;
    // 返回已完成的 move boundary。
    Ok(())
}

// 为 GpuBackend 提供 FrameEncoder 的 RHI 片段执行入口。
impl GpuBackend {
    // 尝试整条无损 lowering；任何目标相关或不完整操作都返回 false。
    pub(super) fn try_execute_frame_encoder_rhi(
        &mut self,
        encoder: &FrameEncoder,
        target: RenderTargetRef,
        load: LoadAction,
        present: bool,
    ) -> Result<bool, Error> {
        // 没有通用 renderer 时不能只迁移其中一部分命令。
        if self.rhi_renderer.is_none() || self.gpu_ctx.rhi_context().is_none() {
            return Ok(false);
        }
        // 保存调用方的主 surface 身份，后续把它改写为 retained texture target。
        let target_is_surface = matches!(target, RenderTargetRef::Surface);
        // 主 surface 使用 drawable extent，Picture texture 使用自身逻辑尺寸。
        let (viewport, scale_x, scale_y) = if target_is_surface {
            let Some(context) = self.gpu_ctx.rhi_context() else {
                return Ok(false);
            };
            super::super::submit::rhi_physical_geometry(context, encoder.width(), encoder.height())
        } else {
            (
                RhiViewport {
                    width: encoder.width().max(1) as f32,
                    height: encoder.height().max(1) as f32,
                },
                1.0,
                1.0,
            )
        };
        // 主 surface 的 FrameEncoder 必须先写入跨帧 retained texture。
        let target = if target_is_surface {
            // 在当前 owner-thread context 上确保本代际 retained texture 存在。
            let texture = self.ensure_rhi_surface_texture()?;
            // 通过通用 render target handle 把 texture 交给离屏 FramePlan。
            RenderTargetRef::Texture(RenderTargetHandle::from_raw(texture.raw()))
        } else {
            // Picture target 已经由调用方提供其专属 RHI texture。
            target
        };
        // 主 surface 的前置 scroll 必须先于本段 FrameEncoder 保持 painter order。
        if target_is_surface && !self.surface.pending_scroll_copies.is_empty() {
            // 读取刚刚确保存在的同代际 retained texture。
            let retained_texture = self.rhi_surface_texture.ok_or_else(|| {
                // 资源状态不完整时拒绝继续 lowering。
                Error::new(
                    Errc::InvalidState,
                    "surface scroll lowering lost its retained texture",
                )
            })?;
            // 不可表达的搬移交回整条 FrameEncoder 兼容回退。
            if !self.try_apply_rhi_surface_scroll_copies(retained_texture)? {
                // 不消费未成功 lower 的逻辑搬移记录。
                return Ok(false);
            }
        }
        // 先完成所有命令的 lowering，再创建任意 native resource。
        let Some(mut lowered) = lower_frame_encoder(encoder, viewport, scale_x, scale_y)? else {
            return Ok(false);
        };
        // 纹理 target 必须同时提供 render target 和 move source 的不透明句柄。
        let RenderTargetRef::Texture(target_handle) = target else {
            // 主 surface 已在上方改写为 retained texture；其它目标同样必须是纹理。
            return Err(Error::new(
                Errc::InvalidState,
                "FrameEncoder RHI lowering requires a texture target",
            ));
        };
        // 将 render target 身份转换为 TextureMove 所需的纹理身份。
        let target_texture = TextureHandle::from_raw(target_handle.raw());
        // 读取 scroll 边界校验所需的物理纹理 extent。
        let target_extent = if target_is_surface {
            // 主 surface 的 extent 来自当前组合 context 代际。
            let Some(context) = self.gpu_ctx.rhi_context() else {
                // context 丢失时交回整条兼容执行器。
                return Ok(false);
            };
            context.token().extent
        } else {
            // Picture texture 在进入 encoder 前已由调用方验证尺寸匹配。
            RhiExtent::new(encoder.width().max(1) as u32, encoder.height().max(1) as u32)
        };
        // 预先降低每个 scroll boundary，任何一条不安全都不触碰 GPU submit。
        let mut moves = Vec::with_capacity(lowered.segments.len());
        for segment in &lowered.segments {
            // 没有 scroll 的首段不产生 TextureMove。
            let Some(scroll) = segment.move_before else {
                moves.push(None);
                continue;
            };
            // 复用 surface 与 FrameEncoder 共用的整数裁剪和 DPR 规则。
            match lower_frame_scroll_move(
                scroll,
                target_texture,
                encoder.width(),
                encoder.height(),
                scale_x,
                scale_y,
                target_extent,
            ) {
                // 记录有效搬移；空 viewport 是安全 no-op。
                Ok(movement) => moves.push(movement),
                // 不能无损表达时整条 encoder 原子回退。
                Err(error) if error.code() == Errc::NotImplemented => return Ok(false),
                // 其它几何或资源错误保持 typed error。
                Err(error) => return Err(error),
            }
        }
        // Additive 是可选 RHI 能力，缺失时保持整条 FrameEncoder 原子回退。
        if lowered.segments.iter().flat_map(|segment| &segment.operations).any(|operation| {
            matches!(operation, RhiOp::Textured(quad) if quad.additive)
                || matches!(operation, RhiOp::AdditiveShape(_))
        }) {
            let Some(context) = self.gpu_ctx.rhi_context() else {
                return Ok(false);
            };
            if !context.capabilities().additive_blend {
                return Ok(false);
            }
        }
        // 每个 scroll boundary 两侧都必须形成可提交的 retained pass。
        for segment in &mut lowered.segments {
            // 空片段以透明 dummy draw 占位，不改变 load 后的颜色。
            if segment.operations.is_empty() {
                segment.operations.push(empty_frame_draw(viewport));
            }
        }
        // 首条 Clear 优先于调用方提供的 retained/load 状态。
        let effective_load = lowered.clear.map(LoadAction::Clear).unwrap_or(load);
        // 主 surface segment 在本次提交前清除旧的待 present 标记。
        if target_is_surface {
            // 只有 execute_ops 成功后才重新标记等待最终合成。
            self.rhi_surface_frame_pending_present = false;
        }
        // 在 owner-thread context 上按片段执行多个有序 RHI boundary。
        let (gpu_ctx, rhi_renderer) = (&mut self.gpu_ctx, &mut self.rhi_renderer);
        let Some(renderer) = rhi_renderer.as_mut() else {
            return Ok(false);
        };
        let Some(context) = gpu_ctx.rhi_context() else {
            return Ok(false);
        };
        // 记录 FrameEncoder 已经实际进入通用 RHI 的调试信息，区分旧路径回退。
        tracing::debug!(
            "Graphics RHI FrameEncoder submit: target={target:?}, surface_retained={}, operations={}, present={present}",
            target_is_surface,
            lowered
                .segments
                .iter()
                .map(|segment| segment.operations.len())
                .sum::<usize>()
        );
        // 依次执行 segment pass 与其前置 TextureMove，严格保留 painter order。
        for (index, (segment, movement)) in lowered
            .segments
            .iter()
            .zip(moves.iter())
            .enumerate()
        {
            // scroll 必须发生在后续 segment pass 之前。
            if let Some(movement) = movement {
                // move boundary 只提交 retained texture，不获取或呈现 swapchain。
                execute_frame_texture_move(context, target_handle, viewport, *movement)?;
            }
            // 首个 segment 使用 encoder clear/load，之后的 segment 保留旧颜色。
            let segment_load = if index == 0 {
                effective_load
            } else {
                LoadAction::Load
            };
            // 最终 present 语义只传给最后一个 segment；主 surface 当前仍由外层 present。
            let segment_present = present && index + 1 == lowered.segments.len();
            // 提交当前连续 RHI 操作并保持其内部顺序。
            renderer.execute_ops(
                context,
                PresentDamage::Full,
                viewport,
                segment_load,
                target,
                &segment.operations,
                segment_present,
            )?;
        }
        // FrameEncoder 已落入 retained texture，最终 swapchain 合成交由 present 边界完成。
        if target_is_surface {
            // 标记只在整条 ordered FramePlan 成功后建立。
            self.rhi_surface_frame_pending_present = true;
        }
        Ok(true)
    }
}
