// 引入基础点与矩形几何。
use crate::core::{Point, Rect};
// 引入 draw System 公开的颜色、线帽、连接、圆角与描边契约。
use crate::draw::{Color, LineCap, LineJoin, Radius, StrokeOptions};
// 引入 draw System 公开绘制上下文。
use crate::draw::painting::PaintContext;
// 引入 UI System 自有的边框线型语义。
use crate::ui::theme::style::BorderStyle;

// 避免退化线段和浮点循环无法收敛。
const GEOMETRY_EPSILON: f32 = 1e-4;
// 单角最多 2048 段；以 0.25 的弦高预算为最终 0.5 轮廓误差保留浮点余量。
const CORNER_STEPS_MIN: usize = 4;
const CORNER_STEPS_MAX: usize = 2048;
const CORNER_SAGITTA: f64 = 0.25;
const MAX_PATTERN_STROKES: usize = 16_384;

fn corner_steps(radius: f32) -> Option<usize> {
    if !radius.is_finite() || radius < 0.0 {
        return None;
    }
    if radius as f64 <= CORNER_SAGITTA {
        return Some(CORNER_STEPS_MIN);
    }
    // r(1-cos(pi/(4n))) <= e；asin 形式避免大半径下 1-e/r 舍入到 1。
    let half_angle = (CORNER_SAGITTA / (2.0 * radius as f64)).sqrt().asin();
    let steps = (std::f64::consts::PI / (8.0 * half_angle)).ceil() as usize;
    (steps <= CORNER_STEPS_MAX).then_some(steps.max(CORNER_STEPS_MIN))
}

// 把 UI 边框线型映射为 draw System 已公开的描边能力。
pub(super) fn paint_border(
    // 接收 draw System 公开绘制上下文。
    ctx: &mut PaintContext,
    // 接收边框中心线所在矩形。
    rect: Rect,
    // 接收已经解析的最终颜色。
    color: Color,
    // 接收现有 Style 契约解析出的单一描边宽度。
    width: f32,
    // 接收四角半径一致的现有 UI 圆角值。
    radius: Option<Radius>,
    // 接收 UI System 自有线型。
    style: BorderStyle,
) {
    // 非有限或非正宽度不产生 draw 调用。
    if !width.is_finite() || width <= 0.0 {
        // 保持绘制失败语义为安全空操作。
        return;
    }
    // 按完整 CSS 线型集合选择确定实现。
    match style {
        // none 只关闭绘制，盒模型宽度已由布局单独消费。
        BorderStyle::None => {}
        // solid 复用现有矩形描边，保持 CPU/GPU 快路径。
        BorderStyle::Solid => ctx.stroke_rect(rect, color, width, radius),
        // dashed 沿完整圆角周长保持连续相位。
        BorderStyle::Dashed => paint_patterned_border(
            // 转交绘制上下文。
            ctx,
            // 转交边框几何。
            rect,
            // 转交边框颜色。
            color,
            // 转交描边宽度。
            width,
            // 圆角线型使用同一周长。
            radius,
            // 虚线单段长度采用三个线宽。
            width * 3.0,
            // 虚线间隔采用两个线宽。
            width * 2.0,
            // 虚线使用平直线帽。
            LineCap::Butt,
        ),
        // dotted 使用极短圆帽线段形成直径等于线宽的圆点。
        BorderStyle::Dotted => paint_patterned_border(
            // 转交绘制上下文。
            ctx,
            // 转交边框几何。
            rect,
            // 转交边框颜色。
            color,
            // 转交描边宽度。
            width,
            // 圆点同样沿圆角周长分布。
            radius,
            // 保留有限非零线段以让共享 stroker 生成圆帽。
            (width * 0.01).max(0.01),
            // 相邻圆点中心约隔两个线宽。
            width * 2.0,
            // 圆帽把短线段扩展为圆点。
            LineCap::Round,
        ),
        // double 使用两道各占约三分之一总宽度的同色描边。
        BorderStyle::Double => paint_double_border(ctx, rect, color, width, radius),
    }
}

// 绘制双线边框并在两道线之间保留一个线宽三分之一的间隙。
fn paint_double_border(
    // 接收 draw System 绘制上下文。
    ctx: &mut PaintContext,
    // 接收外道边框矩形。
    rect: Rect,
    // 接收统一颜色。
    color: Color,
    // 接收总边框宽度。
    width: f32,
    // 接收可选圆角。
    radius: Option<Radius>,
) {
    // 每道线占总宽度三分之一。
    let band = width / 3.0;
    // 外道线保持现有边框中心线。
    ctx.stroke_rect(rect, color, band, radius);
    // 内道线中心向内移动两个 band，留下一个 band 间隙。
    let inset = band * 2.0;
    // 过小矩形无法容纳第二道线时退化为单道细线。
    let Some(inner) = inset_rect(rect, inset) else {
        // 外道线已经提供确定可见结果。
        return;
    };
    // 内圆角随矩形内缩并保持非负。
    let inner_radius = radius.map(|value| {
        // Style 当前只产生四角一致的半径，仍逐角安全收缩。
        Radius {
            // 收缩左上圆角。
            tl: (value.tl - inset).max(0.0),
            // 收缩右上圆角。
            tr: (value.tr - inset).max(0.0),
            // 收缩右下圆角。
            br: (value.br - inset).max(0.0),
            // 收缩左下圆角。
            bl: (value.bl - inset).max(0.0),
        }
    });
    // 绘制内道同色细线。
    ctx.stroke_rect(inner, color, band, inner_radius);
}

// 沿闭合圆角矩形周长绘制连续相位的线段模式。
#[allow(clippy::too_many_arguments)]
fn paint_patterned_border(
    // 接收 draw System 绘制上下文。
    ctx: &mut PaintContext,
    // 接收边框矩形。
    rect: Rect,
    // 接收最终颜色。
    color: Color,
    // 接收描边宽度。
    width: f32,
    // 接收可选圆角。
    radius: Option<Radius>,
    // 接收每次开启描边的周长距离。
    on_length: f32,
    // 接收每次关闭描边的周长距离。
    off_length: f32,
    // 接收虚线或圆点所需线帽。
    cap: LineCap,
) {
    // 生成包含闭合终点的圆角周长折线。
    let perimeter = rounded_rect_perimeter(rect, radius);
    // 无有效周长时安全返回。
    if perimeter.len() < 2 {
        // 不向 draw System提交空路径。
        return;
    }
    // 为每段开启区间生成可跨圆角采样点的开放折线。
    let strokes = patterned_subpaths(&perimeter, on_length, off_length);
    // 所有分段共享相同描边参数。
    let options = StrokeOptions {
        // 保持 Style 声明宽度。
        width,
        // 区分虚线平帽和圆点圆帽。
        cap,
        // 圆角采样点使用圆连接避免折线棱角。
        join: LineJoin::Round,
        // 圆连接不消费 miter 限制，仍使用稳定默认值。
        miter_limit: StrokeOptions::default().miter_limit,
    };
    // 每个开启区间作为独立开放路径提交。
    for stroke in strokes {
        // 共享 draw stroker 同时服务 CPU 与 GPU。
        ctx.stroke_polyline(&stroke, color, &options);
    }
}

// 生成矩形中心线的闭合圆角周长折线（逐角半径，S4）。
//
// 公开契约：整条折线（包括线段中点）距真实圆角轮廓不超过 0.5 逻辑像素；内部采样段数
// 自适应于半径，不构成公开语义。半径先按相邻和规则归一化。
fn rounded_rect_perimeter(rect: Rect, radius: Option<Radius>) -> Vec<Point> {
    // 拒绝非有限或无面积矩形。
    if !rect.x.is_finite()
        // 验证垂直起点。
        || !rect.y.is_finite()
        // 验证宽度。
        || !rect.w.is_finite()
        // 验证高度。
        || !rect.h.is_finite()
        // 宽度必须为正。
        || rect.w <= 0.0
        // 高度必须为正。
        || rect.h <= 0.0
    {
        // 无效输入不产生几何。
        return Vec::new();
    }
    // 逐角半径先做相邻和归一化，保持与其他绘制入口同一轮廓。
    let corner = radius
        .map(|value| value.normalized(rect.w, rect.h))
        .unwrap_or_else(Radius::zero);
    if [corner.tl, corner.tr, corner.br, corner.bl].into_iter().any(|r| corner_steps(r).is_none()) {
        tracing::warn!(target: "uix_app::drawing", "patterned border exceeds the rounded contour segment budget; layer omitted");
        return Vec::new();
    }
    // 计算右侧坐标。
    let right = rect.x + rect.w;
    // 计算底部坐标。
    let bottom = rect.y + rect.h;
    // 全直角矩形使用最小五点闭合周长。
    if corner.tl <= GEOMETRY_EPSILON
        && corner.tr <= GEOMETRY_EPSILON
        && corner.br <= GEOMETRY_EPSILON
        && corner.bl <= GEOMETRY_EPSILON
    {
        // 按顺时针顺序返回并重复起点闭合。
        return vec![
            // 左上角。
            Point::new(rect.x, rect.y),
            // 右上角。
            Point::new(right, rect.y),
            // 右下角。
            Point::new(right, bottom),
            // 左下角。
            Point::new(rect.x, bottom),
            // 回到左上角。
            Point::new(rect.x, rect.y),
        ];
    }
    // 从顶部左圆角终点开始，避免闭合缝落在圆角。
    let mut points = vec![Point::new(rect.x + corner.tl, rect.y)];
    // 追加顶部直线终点。
    points.push(Point::new(right - corner.tr, rect.y));
    // 追加右上圆角。
    append_corner(
        // 转交输出点列。
        &mut points,
        // 右上圆心 X。
        right - corner.tr,
        // 右上圆心 Y。
        rect.y + corner.tr,
        // 右上角半径。
        corner.tr,
        // 从顶部方向开始。
        -std::f64::consts::FRAC_PI_2,
        // 到右侧方向结束。
        0.0,
    );
    // 追加右侧直线终点。
    points.push(Point::new(right, bottom - corner.br));
    // 追加右下圆角。
    append_corner(
        &mut points,
        // 右下圆心 X。
        right - corner.br,
        // 右下圆心 Y。
        bottom - corner.br,
        // 右下角半径。
        corner.br,
        // 从右侧方向开始。
        0.0,
        // 到底部方向结束。
        std::f64::consts::FRAC_PI_2,
    );
    // 追加底部直线终点。
    points.push(Point::new(rect.x + corner.bl, bottom));
    // 追加左下圆角。
    append_corner(
        &mut points,
        // 左下圆心 X。
        rect.x + corner.bl,
        // 左下圆心 Y。
        bottom - corner.bl,
        // 左下角半径。
        corner.bl,
        // 从底部方向开始。
        std::f64::consts::FRAC_PI_2,
        // 到左侧方向结束。
        std::f64::consts::PI,
    );
    // 追加左侧直线终点。
    points.push(Point::new(rect.x, rect.y + corner.tl));
    // 追加左上圆角并回到起点。
    append_corner(
        &mut points,
        // 左上圆心 X。
        rect.x + corner.tl,
        // 左上圆心 Y。
        rect.y + corner.tl,
        // 左上角半径。
        corner.tl,
        // 从左侧方向开始。
        std::f64::consts::PI,
        // 到顶部方向结束。
        std::f64::consts::PI * 1.5,
    );
    // 返回完整闭合周长。
    points
}

// 追加一个顺时针九十度圆角的采样点；段数随半径自适应。
fn append_corner(
    // 接收目标点列。
    points: &mut Vec<Point>,
    // 接收圆心 X。
    cx: f32,
    // 接收圆心 Y。
    cy: f32,
    // 接收半径。
    radius: f32,
    // 接收起始角。
    start: f64,
    // 接收结束角。
    end: f64,
) {
    // 按半径选择本段弧的采样段数。
    let Some(steps) = corner_steps(radius) else { return; };
    // 跳过起点以避免与前一条直线终点重复。
    for step in 1..=steps {
        // 计算当前圆角归一化进度。
        let progress = step as f64 / steps as f64;
        // 线性插值角度。
        let angle = start + (end - start) * progress;
        // 追加圆周采样点。
        points.push(Point::new(
            // 计算 X 坐标。
            (cx as f64 + radius as f64 * angle.cos()) as f32,
            // 计算 Y 坐标。
            (cy as f64 + radius as f64 * angle.sin()) as f32,
        ));
    }
}

// 把闭合周长按开启和关闭距离切分为开放折线集合。
fn patterned_subpaths(points: &[Point], on_length: f32, off_length: f32) -> Vec<Vec<Point>> {
    // 非法 pattern 不产生绘制几何。
    if !on_length.is_finite()
        // 关闭距离也必须有限。
        || !off_length.is_finite()
        // 开启距离必须为正。
        || on_length <= GEOMETRY_EPSILON
        // 关闭距离必须为正。
        || off_length <= GEOMETRY_EPSILON
    {
        // 返回空分段。
        return Vec::new();
    }
    let perimeter: f64 = points.windows(2).map(|pair| {
        (pair[1].x as f64 - pair[0].x as f64).hypot(pair[1].y as f64 - pair[0].y as f64)
    }).sum();
    if !perimeter.is_finite() || perimeter / (on_length as f64 + off_length as f64) > MAX_PATTERN_STROKES as f64 - 1.0 {
        tracing::warn!(target: "uix_app::drawing", "patterned border exceeds the dash segment budget; layer omitted");
        return Vec::new();
    }
    // 保存已经完成的开启区间。
    let mut strokes = Vec::new();
    // 保存当前可能跨越多个周长采样段的开启折线。
    let mut current = Vec::new();
    // pattern 从开启区间开始。
    let mut drawing = true;
    // 保存当前 pattern 区间剩余距离。
    let mut pattern_remaining = on_length;
    // 逐段消费完整闭合周长。
    for pair in points.windows(2) {
        // 当前待消费位置从采样段起点开始。
        let mut start = pair[0];
        // 当前采样段固定终点。
        let target = pair[1];
        // 计算采样段总长度。
        let mut segment_remaining = distance(start, target);
        // 退化采样段不参与 pattern 相位。
        if segment_remaining <= GEOMETRY_EPSILON {
            // 继续下一采样段。
            continue;
        }
        // 一个采样段可能跨越多个开启和关闭区间。
        while segment_remaining > GEOMETRY_EPSILON {
            // 本步最多消费当前 pattern 区间剩余距离。
            let step = segment_remaining.min(pattern_remaining);
            // 按当前剩余线段比例取得本步终点。
            let end = interpolate(start, target, step / segment_remaining);
            // 开启区间把几何追加到当前折线。
            if drawing {
                // 新开启区间先记录起点。
                if current.is_empty() {
                    // 保留 pattern 边界的精确起点。
                    current.push(start);
                }
                // 追加本步终点，允许折线跨越圆角采样点。
                current.push(end);
            }
            // 推进当前采样段起点。
            start = end;
            // 扣除已消费几何距离。
            segment_remaining -= step;
            // 扣除已消费 pattern 距离。
            pattern_remaining -= step;
            // 当前 pattern 区间耗尽时切换状态。
            if pattern_remaining <= GEOMETRY_EPSILON {
                // 完成的开启折线至少需要两个点。
                if drawing && current.len() >= 2 {
                    // 转移折线所有权并清空当前缓存。
                    strokes.push(std::mem::take(&mut current));
                }
                // 开启与关闭状态交替。
                drawing = !drawing;
                // 为新状态重置目标距离。
                pattern_remaining = if drawing { on_length } else { off_length };
            }
        }
    }
    // 周长结束时保留尚未达到完整 on_length 的尾段。
    if current.len() >= 2 {
        // 尾段仍是有效可见折线。
        strokes.push(current);
    }
    // 返回全部开启区间。
    strokes
}

// 对矩形四边执行安全内缩。
fn inset_rect(rect: Rect, inset: f32) -> Option<Rect> {
    // 计算内缩后宽度。
    let width = rect.w - inset * 2.0;
    // 计算内缩后高度。
    let height = rect.h - inset * 2.0;
    // 无面积矩形不能承载第二道边框。
    if width <= GEOMETRY_EPSILON || height <= GEOMETRY_EPSILON {
        // 明确返回无内矩形。
        return None;
    }
    // 返回内缩后的确定矩形。
    Some(Rect::new(
        // 左边向内移动。
        rect.x + inset,
        // 顶边向内移动。
        rect.y + inset,
        // 使用剩余宽度。
        width,
        // 使用剩余高度。
        height,
    ))
}

// 计算两点欧氏距离。
fn distance(start: Point, end: Point) -> f32 {
    // hypot 提供稳定二维长度。
    (end.x - start.x).hypot(end.y - start.y)
}

// 在线段上按零到一比例插值。
fn interpolate(start: Point, end: Point, progress: f32) -> Point {
    // 分别插值两个坐标轴。
    Point::new(
        // 插值 X 坐标。
        start.x + (end.x - start.x) * progress,
        // 插值 Y 坐标。
        start.y + (end.y - start.y) * progress,
    )
}
