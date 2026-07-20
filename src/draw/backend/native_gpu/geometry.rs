//! 变换与设备几何辅助（从 `native_gpu` 主模块拆出，控制文件体积）。

use std::sync::Arc;

use crate::core::{Point, Rect};
use crate::draw::primitives::stroker::StrokeOptions;
use crate::draw::primitives::types::{Radius, Transform};
use crate::native::traits::present::GpuSolidMesh;

#[derive(Clone, Copy)]
pub(super) struct IntegerFrame {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) width: i32,
    pub(super) height: i32,
}

pub(super) fn rect_to_integer_frame(rect: Rect) -> Option<IntegerFrame> {
    if !rect.x.is_finite()
        || !rect.y.is_finite()
        || !rect.w.is_finite()
        || !rect.h.is_finite()
        || rect.x.fract() != 0.0
        || rect.y.fract() != 0.0
        || rect.w.fract() != 0.0
        || rect.h.fract() != 0.0
        || rect.w <= 0.0
        || rect.h <= 0.0
    {
        return None;
    }
    Some(IntegerFrame {
        x: rect.x as i32,
        y: rect.y as i32,
        width: rect.w as i32,
        height: rect.h as i32,
    })
}

pub(super) fn frame_within(frame: IntegerFrame, width: i32, height: i32) -> bool {
    frame.x >= 0
        && frame.y >= 0
        && frame.width > 0
        && frame.height > 0
        && frame.x.saturating_add(frame.width) <= width
        && frame.y.saturating_add(frame.height) <= height
}

pub(super) fn scales_are_uniform(scale: (f32, f32)) -> bool {
    (scale.0.abs() - scale.1.abs()).abs()
        <= f32::EPSILON * scale.0.abs().max(scale.1.abs()).max(1.0)
}

pub(super) fn scaled_corner_radii(radius: Option<Radius>, scale: (f32, f32)) -> [f32; 4] {
    // 各向同性用 |sx|；各向异性用几何平均近似椭圆角在圆形 SDF 下的等效半径。
    let s = if scales_are_uniform(scale) {
        scale.0.abs()
    } else {
        (scale.0.abs() * scale.1.abs()).sqrt()
    };
    match radius {
        Some(rad) => [rad.tl * s, rad.tr * s, rad.br * s, rad.bl * s],
        None => [0.0; 4],
    }
}

pub(super) fn uniform_transform_scale(transform: Transform) -> Option<f32> {
    let [a, b, _, c, d, _] = transform.m;
    if b != 0.0 || c != 0.0 || !a.is_finite() || !d.is_finite() {
        return None;
    }
    if !scales_are_uniform((a, d)) {
        return None;
    }
    Some(a.abs())
}

pub(super) fn stroke_options_for_transform(
    opts: &StrokeOptions,
    transform: Transform,
) -> StrokeOptions {
    let mut scaled = *opts;
    if let Some(scale) = uniform_transform_scale(transform) {
        scaled.width = opts.width * scale;
    }
    scaled
}

pub(super) fn solid_mesh_from_affine_rect(
    rect: Rect,
    transform: Transform,
    offset_x: f32,
    offset_y: f32,
    rgba: [f32; 4],
) -> GpuSolidMesh {
    let map = |x: f32, y: f32| {
        let point = transform.transform_point(Point::new(x + offset_x, y + offset_y));
        (point.x, point.y)
    };
    let (x0, y0) = map(rect.x, rect.y);
    let (x1, y1) = map(rect.x + rect.w, rect.y);
    let (x2, y2) = map(rect.x + rect.w, rect.y + rect.h);
    let (x3, y3) = map(rect.x, rect.y + rect.h);
    GpuSolidMesh {
        vertices: Arc::<[f32]>::from(vec![x0, y0, x1, y1, x2, y2, x0, y0, x2, y2, x3, y3]),
        rgba,
    }
}

/// 逻辑矩形四角经仿射变换到设备坐标（TL/TR/BR/BL）。
pub(super) fn glyph_device_corners(
    rect: Rect,
    transform: Transform,
    offset_x: f32,
    offset_y: f32,
) -> [[f32; 2]; 4] {
    let map = |x: f32, y: f32| {
        let point = transform.transform_point(Point::new(x + offset_x, y + offset_y));
        [point.x, point.y]
    };
    [
        map(rect.x, rect.y),
        map(rect.x + rect.w, rect.y),
        map(rect.x + rect.w, rect.y + rect.h),
        map(rect.x, rect.y + rect.h),
    ]
}

/// 四边形轴对齐包围盒。
pub(super) fn quad_aabb(corners: [[f32; 2]; 4]) -> (f32, f32, f32, f32) {
    let mut min_x = corners[0][0];
    let mut min_y = corners[0][1];
    let mut max_x = corners[0][0];
    let mut max_y = corners[0][1];
    for corner in &corners[1..] {
        min_x = min_x.min(corner[0]);
        min_y = min_y.min(corner[1]);
        max_x = max_x.max(corner[0]);
        max_y = max_y.max(corner[1]);
    }
    (min_x, min_y, max_x, max_y)
}
