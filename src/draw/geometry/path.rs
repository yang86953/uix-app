//! Bezier 路径系统 — 路径构建、二次/三次贝塞尔曲线、填充规则。
//!
//! 参考 tiny-skia::PathBuilder 设计，保持接口简洁。

use crate::core::Point;

use super::types::Transform;

/// 填充规则。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillRule {
    /// 非零环绕规则（默认）。从点向外射射线，顺时针+1逆时针-1，非零则在内部。
    NonZero,
    /// 奇偶规则。射线穿过的路径边数为奇数则在内部。
    EvenOdd,
}

/// 线段端点样式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCap {
    /// 平头（默认）。在端点处垂直截断。
    Butt,
    /// 圆头。以端点为中心画半圆。
    Round,
    /// 方头。超出端点半个线宽后截断。
    Square,
}

/// 线段连接样式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineJoin {
    /// 尖角连接（默认）。延长外边缘直到相交，过长则回退到 Bevel。
    Miter,
    /// 圆角连接。
    Round,
    /// 平角连接。用直线连接外端点。
    Bevel,
}

/// 路径段类型。
#[derive(Debug, Clone, Copy)]
pub enum PathSegment {
    /// 将当前位置移动到指定点，并开始新的子路径。
    MoveTo(Point),
    /// 从当前位置绘制直线到指定点。
    LineTo(Point),
    /// 二次贝塞尔：(控制点, 终点)
    QuadTo(Point, Point),
    /// 三次贝塞尔：(控制点1, 控制点2, 终点)
    CubicTo(Point, Point, Point),
    /// 关闭子路径（直线回到起点）。
    Close,
}

/// 不可变路径，存储已构建的段序列。
#[derive(Debug, Clone)]
pub struct Path {
    pub(crate) segments: Vec<PathSegment>,
}

impl Path {
    /// 返回平移后的新路径（所有坐标 +dx, +dy）。
    pub fn translated(&self, dx: f32, dy: f32) -> Self {
        let segments = self
            .segments
            .iter()
            .map(|seg| match seg {
                PathSegment::MoveTo(p) => PathSegment::MoveTo(Point::new(p.x + dx, p.y + dy)),
                PathSegment::LineTo(p) => PathSegment::LineTo(Point::new(p.x + dx, p.y + dy)),
                PathSegment::QuadTo(c, e) => PathSegment::QuadTo(
                    Point::new(c.x + dx, c.y + dy),
                    Point::new(e.x + dx, e.y + dy),
                ),
                PathSegment::CubicTo(c1, c2, e) => PathSegment::CubicTo(
                    Point::new(c1.x + dx, c1.y + dy),
                    Point::new(c2.x + dx, c2.y + dy),
                    Point::new(e.x + dx, e.y + dy),
                ),
                PathSegment::Close => PathSegment::Close,
            })
            .collect();
        Path { segments }
    }

    /// 返回仿射变换后的新路径（控制点与端点同变换）。
    pub fn transformed(&self, transform: Transform) -> Self {
        let map = |p: Point| transform.transform_point(p);
        let segments = self
            .segments
            .iter()
            .map(|seg| match seg {
                PathSegment::MoveTo(p) => PathSegment::MoveTo(map(*p)),
                PathSegment::LineTo(p) => PathSegment::LineTo(map(*p)),
                PathSegment::QuadTo(c, e) => PathSegment::QuadTo(map(*c), map(*e)),
                PathSegment::CubicTo(c1, c2, e) => {
                    PathSegment::CubicTo(map(*c1), map(*c2), map(*e))
                }
                PathSegment::Close => PathSegment::Close,
            })
            .collect();
        Path { segments }
    }

    /// 遍历路径段。
    pub fn segments(&self) -> &[PathSegment] {
        &self.segments
    }

    /// 路径是否为空。
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// 路径的边界框。
    pub fn bounds(&self) -> Option<Rect> {
        if self.segments.is_empty() {
            return None;
        }
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        // 负坐标路径也必须从负无穷初始化最大值，不能使用 f32::MIN。
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for seg in &self.segments {
            let pts = seg.all_points();
            for p in &pts {
                min_x = min_x.min(p.x);
                min_y = min_y.min(p.y);
                max_x = max_x.max(p.x);
                max_y = max_y.max(p.y);
            }
        }
        if min_x == f32::MAX {
            None
        } else {
            Some(Rect::new(min_x, min_y, max_x - min_x, max_y - min_y))
        }
    }
}

impl PathSegment {
    /// 返回该段涉及的所有端点（不含控制点，曲线请用 all_points）。
    pub fn points(&self) -> &[Point] {
        match self {
            PathSegment::MoveTo(p) => std::slice::from_ref(p),
            PathSegment::LineTo(p) => std::slice::from_ref(p),
            PathSegment::QuadTo(_, _) => &[],
            PathSegment::CubicTo(_, _, _) => &[],
            PathSegment::Close => &[],
        }
    }

    /// 返回该段的所有点（含控制点）。
    pub fn all_points(&self) -> Vec<Point> {
        match self {
            PathSegment::MoveTo(p) => vec![*p],
            PathSegment::LineTo(p) => vec![*p],
            PathSegment::QuadTo(c, e) => vec![*c, *e],
            PathSegment::CubicTo(c1, c2, e) => vec![*c1, *c2, *e],
            PathSegment::Close => vec![],
        }
    }
}

use crate::core::Rect;

/// 路径构建器。调用 move_to / line_to / quad_to / cubic_to / close 构建路径。
///
/// # 示例
#[derive(Debug, Clone)]
pub struct PathBuilder {
    segments: Vec<PathSegment>,
    /// 当前子路径起点（用于 Close 回到起点）。
    start_point: Option<Point>,
    /// 当前位置（上一个段结束点）。
    current_point: Option<Point>,
}

impl PathBuilder {
    /// 创建一个不含任何路径段的构建器。
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
            start_point: None,
            current_point: None,
        }
    }

    /// 移动到新位置（开始新子路径）。
    pub fn move_to(&mut self, x: f32, y: f32) -> &mut Self {
        let p = Point::new(x, y);
        self.segments.push(PathSegment::MoveTo(p));
        self.start_point = Some(p);
        self.current_point = Some(p);
        self
    }

    /// 线段到目标点。
    pub fn line_to(&mut self, x: f32, y: f32) -> &mut Self {
        let p = Point::new(x, y);
        self.segments.push(PathSegment::LineTo(p));
        self.current_point = Some(p);
        self
    }

    /// 二次贝塞尔曲线到目标点，cx/cy 为控制点。
    pub fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) -> &mut Self {
        let c = Point::new(cx, cy);
        let e = Point::new(x, y);
        self.segments.push(PathSegment::QuadTo(c, e));
        self.current_point = Some(e);
        self
    }

    /// 三次贝塞尔曲线到目标点，c1/c2 为控制点。
    pub fn cubic_to(
        &mut self,
        c1x: f32,
        c1y: f32,
        c2x: f32,
        c2y: f32,
        x: f32,
        y: f32,
    ) -> &mut Self {
        let c1 = Point::new(c1x, c1y);
        let c2 = Point::new(c2x, c2y);
        let e = Point::new(x, y);
        self.segments.push(PathSegment::CubicTo(c1, c2, e));
        self.current_point = Some(e);
        self
    }

    /// 以三次 Bézier 段追加圆弧；角度使用弧度，单次最多绘制一周。
    pub fn arc(
        &mut self,
        cx: f32,
        cy: f32,
        radius: f32,
        start_angle: f32,
        end_angle: f32,
    ) -> &mut Self {
        if !cx.is_finite()
            || !cy.is_finite()
            || !radius.is_finite()
            || !start_angle.is_finite()
            || !end_angle.is_finite()
            || radius <= 0.0
        {
            return self;
        }

        let sweep = (end_angle - start_angle).clamp(-std::f32::consts::TAU, std::f32::consts::TAU);
        if sweep.abs() <= f32::EPSILON {
            return self;
        }
        let segment_count = (sweep.abs() / std::f32::consts::FRAC_PI_2).ceil().max(1.0) as usize;
        let segment_sweep = sweep / segment_count as f32;
        self.move_to(
            cx + radius * start_angle.cos(),
            cy + radius * start_angle.sin(),
        );

        for index in 0..segment_count {
            let angle_a = start_angle + segment_sweep * index as f32;
            let angle_b = angle_a + segment_sweep;
            let tangent_scale = 4.0 / 3.0 * (segment_sweep * 0.25).tan() * radius;
            let (sin_a, cos_a) = angle_a.sin_cos();
            let (sin_b, cos_b) = angle_b.sin_cos();
            let end_x = cx + radius * cos_b;
            let end_y = cy + radius * sin_b;
            self.cubic_to(
                cx + radius * cos_a - tangent_scale * sin_a,
                cy + radius * sin_a + tangent_scale * cos_a,
                end_x + tangent_scale * sin_b,
                end_y - tangent_scale * cos_b,
                end_x,
                end_y,
            );
        }
        if (sweep.abs() - std::f32::consts::TAU).abs() <= f32::EPSILON * 8.0 {
            self.close();
        }
        self
    }

    /// 追加一条开放折线。少于两个有效点时不产生几何。
    pub fn polyline(&mut self, points: &[Point]) -> &mut Self {
        if points.len() < 2
            || points
                .iter()
                .any(|point| !point.x.is_finite() || !point.y.is_finite())
        {
            return self;
        }
        self.move_to(points[0].x, points[0].y);
        for point in &points[1..] {
            self.line_to(point.x, point.y);
        }
        self
    }

    /// 追加一个闭合多边形。少于三个有效点时不产生几何。
    pub fn polygon(&mut self, points: &[Point]) -> &mut Self {
        if points.len() < 3
            || points
                .iter()
                .any(|point| !point.x.is_finite() || !point.y.is_finite())
        {
            return self;
        }
        self.polyline(points);
        self.close()
    }

    /// 以四段三次 Bézier 追加由 `bounds` 定义的闭合椭圆。
    pub fn ellipse(&mut self, bounds: Rect) -> &mut Self {
        if !bounds.x.is_finite()
            || !bounds.y.is_finite()
            || !bounds.w.is_finite()
            || !bounds.h.is_finite()
            || bounds.w <= 0.0
            || bounds.h <= 0.0
        {
            return self;
        }

        const KAPPA: f32 = 0.552_284_8;
        let cx = bounds.x + bounds.w * 0.5;
        let cy = bounds.y + bounds.h * 0.5;
        let rx = bounds.w * 0.5;
        let ry = bounds.h * 0.5;
        let ox = rx * KAPPA;
        let oy = ry * KAPPA;

        self.move_to(cx + rx, cy)
            .cubic_to(cx + rx, cy + oy, cx + ox, cy + ry, cx, cy + ry)
            .cubic_to(cx - ox, cy + ry, cx - rx, cy + oy, cx - rx, cy)
            .cubic_to(cx - rx, cy - oy, cx - ox, cy - ry, cx, cy - ry)
            .cubic_to(cx + ox, cy - ry, cx + rx, cy - oy, cx + rx, cy)
            .close()
    }

    /// 关闭当前子路径（回到 move_to 起点）。
    pub fn close(&mut self) -> &mut Self {
        self.segments.push(PathSegment::Close);
        self.current_point = self.start_point;
        self
    }

    /// 构建不可变 Path。
    pub fn build(&self) -> Path {
        Path {
            segments: self.segments.clone(),
        }
    }

    /// 当前是否在构建中（有至少一个 move_to）。
    pub fn has_segments(&self) -> bool {
        !self.segments.is_empty()
    }
}

impl Default for PathBuilder {
    fn default() -> Self {
        Self::new()
    }
}
