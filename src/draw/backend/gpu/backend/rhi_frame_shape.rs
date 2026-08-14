//! FrameEncoder 轴对齐 shape 到通用 RHI 的亚像素 lowering。

// 引入共享矩形和颜色值。
use crate::core::Rect;
// 引入 FrameEncoder 的整数、亚像素和样式值类型。
use crate::draw::painting::{FrameRadius, FrameRect, FrameSampledRect, FrameStrokeWidth};

// 引入父模块已经统一定义的颜色、裁剪与 RHI 载荷。
use super::{color_rgba, scaled_radius, scaled_scissor, Color, RhiOp, RhiShapeRect, RhiViewport};

// 把整数 FrameRect 降低为共享 shape 队列。
pub(super) fn append_shape(
    // 接收保持 painter order 的 RHI 队列。
    operations: &mut Vec<RhiOp>,
    // 接收整数矩形。
    rect: FrameRect,
    // 接收直通颜色。
    color: Color,
    // 接收圆角半径。
    radius: FrameRadius,
    // 接收半描边宽度。
    half_stroke: f32,
    // 选择普通或 Additive blend。
    additive: bool,
    // 接收可选整数裁剪。
    clip: Option<FrameRect>,
    // 接收逻辑目标边界。
    bounds: FrameRect,
    // 接收物理视口。
    viewport: RhiViewport,
    // 接收水平缩放。
    scale_x: f32,
    // 接收垂直缩放。
    scale_y: f32,
) -> bool {
    // 空整数矩形保持既有 no-op 语义。
    if rect.is_empty() {
        // 报告命令已经安全处理。
        return true;
    }
    // 把整数几何转换为共享亚像素表示。
    let rect = Rect::new(
        // 转换左边界。
        rect.x as f32,
        // 转换上边界。
        rect.y as f32,
        // 转换宽度。
        rect.width as f32,
        // 转换高度。
        rect.height as f32,
    );
    // 委托统一的亚像素 shape lowering。
    append_rect_shape(
        operations,
        rect,
        color,
        radius,
        half_stroke,
        additive,
        clip,
        bounds,
        viewport,
        scale_x,
        scale_y,
    )
}

// 把亚像素 FrameEncoder shape 降低为共享 shape 队列。
pub(super) fn append_subpixel_shape(
    // 接收保持 painter order 的 RHI 队列。
    operations: &mut Vec<RhiOp>,
    // 接收已经验证的亚像素矩形。
    rect: FrameSampledRect,
    // 接收直通颜色。
    color: Color,
    // 接收圆角半径。
    radius: FrameRadius,
    // 接收可选描边宽度。
    line_width: Option<FrameStrokeWidth>,
    // 接收整数裁剪。
    clip: FrameRect,
    // 接收逻辑目标边界。
    bounds: FrameRect,
    // 接收物理视口。
    viewport: RhiViewport,
    // 接收水平缩放。
    scale_x: f32,
    // 接收垂直缩放。
    scale_y: f32,
) -> bool {
    // 由可选线宽计算 shape shader 使用的半描边宽度。
    let half_stroke = line_width.map_or(0.0, |width| width.value() * 0.5);
    // 委托统一的亚像素 shape lowering，并固定普通 SrcOver blend。
    append_rect_shape(
        operations,
        rect.to_rect(),
        color,
        radius,
        half_stroke,
        false,
        Some(clip),
        bounds,
        viewport,
        scale_x,
        scale_y,
    )
}

// 追加一条已验证的轴对齐 shape 到混合 RHI 队列。
fn append_rect_shape(
    // 接收保持 painter order 的 RHI 队列。
    operations: &mut Vec<RhiOp>,
    // 接收真实亚像素矩形。
    rect: Rect,
    // 接收直通颜色。
    color: Color,
    // 接收圆角半径。
    radius: FrameRadius,
    // 接收半描边宽度。
    half_stroke: f32,
    // 选择普通或 Additive blend。
    additive: bool,
    // 接收可选整数裁剪。
    clip: Option<FrameRect>,
    // 接收逻辑目标边界。
    bounds: FrameRect,
    // 接收物理视口。
    viewport: RhiViewport,
    // 接收水平缩放。
    scale_x: f32,
    // 接收垂直缩放。
    scale_y: f32,
) -> bool {
    // 缩放真实矩形并拒绝无效物理几何。
    let Some((x, y, width, height)) = scaled_sampled_rect(rect, scale_x, scale_y) else {
        // 无法保真降低时交回上层处理。
        return false;
    };
    // 缩放四角半径并拒绝无效值。
    let Some(radius) = scaled_radius(radius, scale_x, scale_y) else {
        // 无法保真降低时交回上层处理。
        return false;
    };
    // 把可选逻辑裁剪转换为物理 scissor。
    let scissor = match clip {
        // 有裁剪时必须限制到当前目标。
        Some(clip) => match scaled_scissor(clip, bounds, viewport, scale_x, scale_y) {
            // 保存有效物理裁剪。
            Some(scissor) => Some(scissor),
            // 完全被裁掉时安全视为 no-op。
            None => return true,
        },
        // 无裁剪时由目标边界负责限制。
        None => None,
    };
    // 描边宽度必须稳定进入 shape 常量。
    if !half_stroke.is_finite() || half_stroke < 0.0 {
        // 无效宽度不能进入 GPU。
        return false;
    }
    // 组装共享 shape shader 的完整载荷。
    let shape = RhiShapeRect {
        // 保存物理左边界。
        x,
        // 保存物理上边界。
        y,
        // 保存物理宽度。
        w: width,
        // 保存物理高度。
        h: height,
        // 保存规范化直通颜色。
        rgba: color_rgba(color),
        // 保存缩放后的四角半径。
        radius,
        // 保存缩放后的半描边宽度。
        half_stroke: half_stroke * (scale_x.abs() * scale_y.abs()).sqrt(),
        // 保存可选物理裁剪。
        scissor,
    };
    // 根据命令事实选择目标 blend 队列。
    operations.push(if additive {
        // Additive shape 使用独立饱和加法 pipeline。
        RhiOp::AdditiveShape(shape)
    } else {
        // 普通 shape 使用 SrcOver pipeline。
        RhiOp::Shape(shape)
    });
    // 报告命令已经安全降低。
    true
}

// 缩放一个逻辑亚像素矩形到物理目标。
fn scaled_sampled_rect(rect: Rect, scale_x: f32, scale_y: f32) -> Option<(f32, f32, f32, f32)> {
    // 分别缩放位置和尺寸。
    let (x, y, width, height) = (
        // 缩放左边界。
        rect.x * scale_x,
        // 缩放上边界。
        rect.y * scale_y,
        // 缩放宽度。
        rect.w * scale_x,
        // 缩放高度。
        rect.h * scale_y,
    );
    // 拒绝溢出或反向的物理几何。
    if !x.is_finite()
        || !y.is_finite()
        || !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
    {
        // 返回无法降低的明确结果。
        return None;
    }
    // 返回完整物理矩形。
    Some((x, y, width, height))
}
