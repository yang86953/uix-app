//! SoftwareRasterizer 的路径裁剪 mask lowering。
//!
//! 路径先复用通用 fill tessellator，再以四点 coverage 写入逐像素 mask；
//! mask 与已有路径 mask 相乘，矩形裁剪仍由 SoftwareRasterizer 的整数 scissor 负责。

use crate::core::{Errc, Error, Point, Rect};
use crate::draw::geometry::path::{FillRule, Path};
use crate::draw::geometry::tessellator;
use crate::draw::geometry::types::Transform;

use super::software_rasterizer::SoftwareRasterizer;

impl SoftwareRasterizer {
    /// 将当前 Canvas2D 状态下的路径压入逐像素裁剪栈。
    pub(crate) fn try_push_clip_path(&mut self, path: &Path) -> Result<(), Error> {
        // 路径裁剪必须和普通 path fill 使用同一个设备空间变换。
        let composed = self
            .transform()
            .concat(Transform::translate(self.offset_x, self.offset_y));
        // 先完成变换，再交给共享 tessellator 做有限性和拓扑预算检查。
        let device_path = path.transformed(composed);
        // 非法或超预算路径保持为可观察的 typed failure，不改变当前裁剪状态。
        let triangles =
            tessellator::tessellate_fill(&device_path, FillRule::NonZero).ok_or_else(|| {
                Error::new(
                    Errc::NotImplemented,
                    "path clip tessellation is unsupported",
                )
            })?;
        // 目标尺寸已在构造时钳制为正数，checked_mul 仍保留地址空间安全边界。
        let pixel_count = usize::try_from(self.surface_w)
            .ok()
            .and_then(|width| {
                usize::try_from(self.surface_h)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .ok_or_else(|| {
                Error::new(Errc::GraphicsOutOfMemory, "path clip mask extent overflow")
            })?;
        // mask 先存四个采样点的 bit，最后统一换算成 0/85/170/255 coverage。
        let mut mask = Vec::new();
        // 预留失败必须转成 graphics OOM，而不是让 Vec 在 OOM 时直接 abort。
        mask.try_reserve_exact(pixel_count).map_err(|error| {
            Error::new(
                Errc::GraphicsOutOfMemory,
                format!("path clip mask allocation failed: {error}"),
            )
        })?;
        // 预留成功后填充整张目标，保证每个像素都有稳定的索引。
        mask.resize(pixel_count, 0);
        // 根据 tessellation 三角形边界只扫描可能命中的像素区域。
        for triangle in triangles.chunks_exact(6) {
            // 读取三角形的三个设备空间顶点。
            let a = Point::new(triangle[0], triangle[1]);
            let b = Point::new(triangle[2], triangle[3]);
            let c = Point::new(triangle[4], triangle[5]);
            // 退化三角形不产生 coverage，也不应扩大裁剪边界。
            if triangle_area2(a, b, c).abs() <= f32::EPSILON {
                continue;
            }
            // 计算当前三角形的有限包围盒。
            let min_x = a.x.min(b.x).min(c.x);
            let min_y = a.y.min(b.y).min(c.y);
            let max_x = a.x.max(b.x).max(c.x);
            let max_y = a.y.max(b.y).max(c.y);
            // 非有限边界不应绕过上面的 tessellator 约束。
            if !(min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite()) {
                return Err(Error::new(
                    Errc::NotImplemented,
                    "path clip triangle has non-finite bounds",
                ));
            }
            // 将包围盒转换为半开像素范围，并和目标尺寸求交。
            let x0 = floor_to_i32(min_x).max(0).min(self.surface_w);
            let y0 = floor_to_i32(min_y).max(0).min(self.surface_h);
            let x1 = ceil_to_i32(max_x).max(0).min(self.surface_w);
            let y1 = ceil_to_i32(max_y).max(0).min(self.surface_h);
            // 完全落在目标之外的三角形不需要进入采样循环。
            if x0 >= x1 || y0 >= y1 {
                continue;
            }
            // 对包围盒内的每个像素写入四个固定采样点的覆盖 bit。
            for y in y0..y1 {
                for x in x0..x1 {
                    // 使用像素中心周围的四个样本，兼顾边界 coverage 与成本。
                    let samples = [
                        (x as f32 + 0.25, y as f32 + 0.25),
                        (x as f32 + 0.75, y as f32 + 0.25),
                        (x as f32 + 0.25, y as f32 + 0.75),
                        (x as f32 + 0.75, y as f32 + 0.75),
                    ];
                    // 只有命中三角形的样本才设置对应 coverage bit。
                    let mut bits = mask[(y as usize) * self.surface_w as usize + x as usize];
                    for (sample_index, (sx, sy)) in samples.into_iter().enumerate() {
                        // 边界点归入三角形，避免细缝导致路径裁剪断裂。
                        if point_in_triangle(sx, sy, a, b, c) {
                            bits |= 1u8 << sample_index;
                        }
                    }
                    // 保存合并后的 sample bits，多个三角形形成 union coverage。
                    mask[(y as usize) * self.surface_w as usize + x as usize] = bits;
                }
            }
        }
        // tessellator 输出必须是完整三角形列表，否则拒绝建立部分 mask。
        if !triangles.chunks_exact(6).remainder().is_empty() {
            return Err(Error::new(
                Errc::NotImplemented,
                "path clip tessellation returned incomplete triangle",
            ));
        }
        // 将 sample bits 转换为 8-bit coverage，四个样本各占四分之一。
        for value in &mut mask {
            // 4 个样本的满 coverage 需要在 u8 范围内饱和到 255。
            *value = (value.count_ones() as u16 * 85).min(255) as u8;
        }
        // 路径裁剪与已有路径裁剪相交，保持嵌套 clip 的乘法 coverage 语义。
        if let Some(previous) = self.clip_mask.as_ref() {
            for (value, old_value) in mask.iter_mut().zip(previous.iter().copied()) {
                // premultiplied coverage 的乘法使用四舍五入，减少连续 clip 的偏暗。
                *value = (((*value as u16) * (old_value as u16) + 127) / 255) as u8;
            }
        }
        // 用三角形包围盒收紧逻辑 clip AABB；空路径的 bounds 为零区域。
        let path_bounds = triangle_bounds(&triangles);
        // mask 已完全建立，之后才修改栈，保证失败路径不污染状态。
        self.clip_stack.push(self.clip_rect);
        self.clip_mask_stack.push(self.clip_mask.clone());
        self.clip_mask = Some(mask);
        // 空路径裁剪所有像素；有边界时和旧矩形 clip 求交。
        if let Some(bounds) = path_bounds {
            if let Some(intersection) = self.clip_rect.intersect(&bounds) {
                self.clip_rect = intersection;
                self.sync_clip_int();
            } else {
                self.clip_rect = Rect::zero();
                self.sync_clip_int();
            }
        } else {
            self.clip_rect = Rect::zero();
            self.sync_clip_int();
        }
        // 路径裁剪入栈成功。
        Ok(())
    }
}

// 计算有符号二倍面积，用于过滤退化三角形。
fn triangle_area2(a: Point, b: Point, c: Point) -> f32 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

// 将有限浮点边界安全转换为 i32 像素坐标。
fn floor_to_i32(value: f32) -> i32 {
    value.floor().clamp(i32::MIN as f32, i32::MAX as f32) as i32
}

// 将有限浮点边界安全转换为 i32 半开坐标。
fn ceil_to_i32(value: f32) -> i32 {
    value.ceil().clamp(i32::MIN as f32, i32::MAX as f32) as i32
}

// 判断采样点是否落在三角形内部或其边界上。
fn point_in_triangle(px: f32, py: f32, a: Point, b: Point, c: Point) -> bool {
    // 分别计算采样点相对三条边的叉积符号。
    let ab = cross(a, b, px, py);
    let bc = cross(b, c, px, py);
    let ca = cross(c, a, px, py);
    // 同号说明点在三角形内；极小误差视为边界命中。
    let epsilon = 1e-5;
    (ab >= -epsilon && bc >= -epsilon && ca >= -epsilon)
        || (ab <= epsilon && bc <= epsilon && ca <= epsilon)
}

// 计算边端点到采样点的二维叉积。
fn cross(a: Point, b: Point, px: f32, py: f32) -> f32 {
    (b.x - a.x) * (py - a.y) - (b.y - a.y) * (px - a.x)
}

// 从三角形列表提取路径设备空间边界。
fn triangle_bounds(triangles: &[f32]) -> Option<Rect> {
    // 空三角形列表代表空 clip path。
    if triangles.is_empty() {
        return None;
    }
    // 初始化为第一个顶点，避免 f32::MIN 对负坐标造成错误。
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    // 所有顶点共同形成保守 AABB。
    for coordinate in triangles.chunks_exact(2) {
        min_x = min_x.min(coordinate[0]);
        min_y = min_y.min(coordinate[1]);
        max_x = max_x.max(coordinate[0]);
        max_y = max_y.max(coordinate[1]);
    }
    // 有限且有面积的边界才可进入 clip 状态。
    if min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite() {
        Some(Rect::new(min_x, min_y, max_x - min_x, max_y - min_y))
    } else {
        None
    }
}
