//! Bezier 路径系统 — 路径构建、二次/三次贝塞尔曲线、填充规则。
//!
//! 参考 tiny-skia::PathBuilder 设计，保持接口简洁。

use crate::native::Point;

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
    MoveTo(Point),
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
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
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

use crate::native::Rect;

/// 路径构建器。调用 move_to / line_to / quad_to / cubic_to / close 构建路径。
///
/// # 示例
/// ```ignore
/// let mut pb = PathBuilder::new();
/// pb.move_to(10.0, 10.0);
/// pb.line_to(100.0, 10.0);
/// pb.line_to(100.0, 100.0);
/// pb.close();
/// let path = pb.build();
/// ```
#[derive(Debug, Clone)]
pub struct PathBuilder {
    segments: Vec<PathSegment>,
    /// 当前子路径起点（用于 Close 回到起点）。
    start_point: Option<Point>,
    /// 当前位置（上一个段结束点）。
    current_point: Option<Point>,
}

impl PathBuilder {
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
