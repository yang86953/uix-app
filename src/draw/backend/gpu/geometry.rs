//! 变换与设备几何辅助（从 `gpu` 主模块拆出，控制文件体积）。

use std::sync::Arc;

use crate::core::{Point, Rect};
// 引入共享路径构造类型，供变换圆角矩形复用通用 tessellator。
use crate::draw::geometry::path::{Path, PathBuilder};
use crate::draw::geometry::stroker::StrokeOptions;
use crate::draw::geometry::types::{Radius, Transform};
// 引入所属 graphics backend Module 的纯色网格原语。
use super::GpuSolidMesh;

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

// 把圆角矩形构造成闭合路径，供旋转、剪切和任意仿射 solid mesh 复用。
pub(super) fn rounded_rect_path(rect: Rect, radius: Option<Radius>) -> Path {
    // 先把已验证输入规整为有限非负半径，避免路径 builder 接收异常值。
    let finite_radius = |value: f32| {
        if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        }
    };
    // 保存左上、右上、右下、左下四个角的半径。
    let mut radii = radius
        .map(|value| {
            [
                finite_radius(value.tl),
                finite_radius(value.tr),
                finite_radius(value.br),
                finite_radius(value.bl),
            ]
        })
        .unwrap_or([0.0; 4]);
    // 按相邻边总长统一缩放，保持不相交的圆角轮廓。
    let mut factor = 1.0_f32;
    for (sum, extent) in [
        (radii[0] + radii[1], rect.w),
        (radii[3] + radii[2], rect.w),
        (radii[0] + radii[3], rect.h),
        (radii[1] + radii[2], rect.h),
    ] {
        if sum > extent && sum > 0.0 && extent.is_finite() {
            factor = factor.min((extent / sum).max(0.0));
        }
    }
    // 应用统一比例，处理圆角和矩形尺寸不匹配的边界输入。
    for value in &mut radii {
        *value *= factor;
    }
    // 使用圆弧的三次 Bézier 近似，精度与共享 PathBuilder ellipse 一致。
    const KAPPA: f32 = 0.552_284_8;
    // 读取矩形边界和四个圆角半径。
    let [tl, tr, br, bl] = radii;
    let x0 = rect.x;
    let y0 = rect.y;
    let x1 = rect.x + rect.w;
    let y1 = rect.y + rect.h;
    // 创建顺时针闭合路径，起点位于上边的左圆角结束处。
    let mut builder = PathBuilder::new();
    builder.move_to(x0 + tl, y0).line_to(x1 - tr, y0);
    // 追加右上圆角或直角连接。
    if tr > 0.0 {
        let cx = x1 - tr;
        let cy = y0 + tr;
        builder.cubic_to(cx + KAPPA * tr, cy - tr, cx + tr, cy - KAPPA * tr, x1, cy);
    } else {
        builder.line_to(x1, y0);
    }
    // 追加右边和右下圆角。
    builder.line_to(x1, y1 - br);
    if br > 0.0 {
        let cx = x1 - br;
        let cy = y1 - br;
        builder.cubic_to(cx + br, cy + KAPPA * br, cx + KAPPA * br, cy + br, cx, y1);
    } else {
        builder.line_to(x1, y1);
    }
    // 追加下边和左下圆角。
    builder.line_to(x0 + bl, y1);
    if bl > 0.0 {
        let cx = x0 + bl;
        let cy = y1 - bl;
        builder.cubic_to(cx - KAPPA * bl, cy + bl, cx - bl, cy + KAPPA * bl, x0, cy);
    } else {
        builder.line_to(x0, y1);
    }
    // 追加左边和左上圆角，再闭合路径。
    builder.line_to(x0, y0 + tl);
    if tl > 0.0 {
        let cx = x0 + tl;
        let cy = y0 + tl;
        builder.cubic_to(cx - tl, cy - KAPPA * tl, cx - KAPPA * tl, cy - tl, cx, y0);
    } else {
        builder.line_to(x0, y0);
    }
    // 返回共享几何系统可 tessellate 的不可变路径。
    builder.close().build()
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

/// 检查 TL/TR/BR/BL 顺序的四边形是否有限、凸且非退化。
pub(super) fn convex_quad_is_valid(corners: &[[f32; 2]; 4]) -> bool {
    // 顶点坐标必须可表示，避免后续物理缩放把 NaN 传入 shader。
    if corners.iter().flatten().any(|value| !value.is_finite()) {
        // 异常四边形交回兼容路径。
        return false;
    }
    // 逐个检查连续边的叉积，允许顺时针或逆时针但不允许混绕向。
    let mut winding = 0.0f32;
    // 四个连续的非零转角共同保证四边形为凸形。
    for index in 0..4 {
        // 读取相邻三个顶点。
        let a = corners[index];
        let b = corners[(index + 1) % 4];
        let c = corners[(index + 2) % 4];
        // 计算二维叉积。
        let cross = (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]);
        // 退化或溢出的边不能稳定地映射单位 quad。
        if !cross.is_finite() || cross.abs() <= 1e-5 {
            // 返回 false 让原有路径继续处理。
            return false;
        }
        // 第一个转角建立绕向，后续转角必须保持一致。
        if winding == 0.0 {
            winding = cross.signum();
        } else if cross.signum() != winding {
            // 自交或凹四边形不能稳定 lowering。
            return false;
        }
    }
    // 四边形通过几何门禁。
    true
}

/// 物理像素 1:1、无旋转/剪切时用面积 coverage R8；其余留给 MSDF。
///
/// R8 coverage 已在逻辑像素网格上光栅化；DPR 不为 1 时再次缩放它会发虚，
/// 因而即使 Canvas transform 为 identity 也必须走可缩放的 MSDF。
pub(super) fn outline_uses_area_r8(
    transform: Transform,
    device_pixel_ratio: f32,
    device_w: f32,
    device_h: f32,
    cov_w: usize,
    cov_h: usize,
) -> bool {
    if !device_pixel_ratio.is_finite() || (device_pixel_ratio - 1.0).abs() > 0.01 {
        return false;
    }
    let [a, b, _, c, d, _] = transform.m;
    if b != 0.0 || c != 0.0 || !a.is_finite() || !d.is_finite() {
        return false;
    }
    // 约 1% 容差；超过则保留 MSDF 以保缩放/大字号角点。
    if (a.abs() - 1.0).abs() > 0.01 || (d.abs() - 1.0).abs() > 0.01 {
        return false;
    }
    let cw = cov_w as f32;
    let ch = cov_h as f32;
    (device_w - cw).abs() <= 0.51 && (device_h - ch).abs() <= 0.51
}
