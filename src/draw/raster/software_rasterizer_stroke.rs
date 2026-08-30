//! SoftwareRasterizer 矢量描边实现。

use crate::core::Rect;

use crate::draw::geometry::color::Color;
use crate::draw::geometry::path::{FillRule, Path};
use crate::draw::geometry::stroker::StrokeOptions;
use crate::draw::geometry::types::Radius;

use super::software_rasterizer::SoftwareRasterizer;

impl SoftwareRasterizer {
    #[allow(
        clippy::too_many_arguments,
        reason = "shape coordinates and surface bounds mirror the canvas contract"
    )]
    pub(crate) fn stroke_rect(
        &self,
        pixels: &mut [u32],
        surface_w: i32,
        surface_h: i32,
        rect: Rect,
        color: Color,
        line_width: f32,
        radius: Option<Radius>,
    ) {
        let (ox, oy) = (self.offset_x, self.offset_y);
        let rect = if ox != 0.0 || oy != 0.0 {
            Rect::new(rect.x + ox, rect.y + oy, rect.w, rect.h)
        } else {
            rect
        };
        let lw = line_width.max(0.0);
        // 半径规范化每 draw call 一次，与 GPU shape/path lowering 同一 CSS 式规则。
        let rad =
            super::rasterizer::core::normalize_corner_radius(rect.w, rect.h, radius.unwrap_or_default());
        let c = self.apply_opa(Self::premul(color));
        // 只有 identity transform 才能使用直接写轴对齐像素的快速路径。
        let identity = Self::is_identity(&self.transform);
        // Fast path for 1px axis-aligned
        if identity
            && rad.tl == 0.0
            && rad.tr == 0.0
            && rad.bl == 0.0
            && rad.br == 0.0
            && rect.x.fract() == 0.0
            && rect.y.fract() == 0.0
            && rect.w.fract() == 0.0
            && rect.h.fract() == 0.0
            && lw == 1.0
            && lw.fract() == 0.0
        {
            let x0 = rect.x as i32;
            let y0 = rect.y as i32;
            let w = rect.w as i32;
            let h = rect.h as i32;
            let iw = lw as i32;
            if iw * 2 >= w || iw * 2 >= h {
                self.fill_rect_raw(pixels, surface_w, surface_h, x0, y0, w, h, c);
                return;
            }
            let inner_h = h - iw * 2;
            self.fill_rect_raw(pixels, surface_w, surface_h, x0, y0, w, iw, c);
            self.fill_rect_raw(pixels, surface_w, surface_h, x0, y0 + h - iw, w, iw, c);
            self.fill_rect_raw(pixels, surface_w, surface_h, x0, y0 + iw, iw, inner_h, c);
            self.fill_rect_raw(
                pixels,
                surface_w,
                surface_h,
                x0 + w - iw,
                y0 + iw,
                iw,
                inner_h,
                c,
            );
            return;
        }
        let h = lw * 0.5;
        // 圆角矩形对齐物理像素网格：亚像素坐标下 SDF 弧线端点与像素中心错位，
        // 导致四角取整不对称（顶/底圆角视觉半径不一致）。直角矩形无此问题。
        let rect = if rad.tl != 0.0 || rad.tr != 0.0 || rad.bl != 0.0 || rad.br != 0.0 {
            match super::rasterizer::core::align_rounded_rect(rect) {
                Some(rect) => rect,
                None => return,
            }
        } else {
            rect
        };
        // 圆角描边使用"外扩 outer + 内缩 inner"双 SDF(与 GPU 后端一致):
        // 外扩 half 使弧线端点对齐像素中心,消除整数坐标下顶/底圆角起点偏差。
        let outer_rect = Rect::new(rect.x - h, rect.y - h, rect.w + lw, rect.h + lw);
        let inner_rect = Rect::new(
            rect.x + h,
            rect.y + h,
            (rect.w - lw).max(0.0),
            (rect.h - lw).max(0.0),
        );
        let outer_rad = Radius {
            tl: rad.tl + h,
            tr: rad.tr + h,
            br: rad.br + h,
            bl: rad.bl + h,
        };
        let inner_rad = Radius {
            tl: (rad.tl - h).max(0.0),
            tr: (rad.tr - h).max(0.0),
            br: (rad.br - h).max(0.0),
            bl: (rad.bl - h).max(0.0),
        };
        let expand = h + 1.0;
        let expanded = Rect::new(
            outer_rect.x - expand,
            outer_rect.y - expand,
            outer_rect.w + expand * 2.0,
            outer_rect.h + expand * 2.0,
        );
        // 任意仿射下先把保守本地写区映射到 surface。
        let device_bounds = if identity {
            // identity 继续使用原始 SDF 扫描边界。
            expanded
        } else {
            // 可逆仿射在像素循环内通过逆映射恢复本地描边坐标。
            self.transform_rect(&expanded)
        };
        // 只扫描变换后写区与当前 surface clip 的交集。
        if let Some(cr) = self.intersect_clip(&device_bounds) {
            let x0 = cr.x as i32;
            let y0 = cr.y as i32;
            let x1 = (cr.x + cr.w) as i32;
            let y1 = (cr.y + cr.h) as i32;
            let inner_is_positive = inner_rect.w > 0.0 && inner_rect.h > 0.0;
            for py in y0..y1 {
                for px in x0..x1 {
                    // identity 直接使用设备像素中心，仿射路径先逆映射到本地空间。
                    let Some((ux, uy)) = (if identity {
                        // 无变换时保持既有位精确坐标。
                        Some((px as f32 + 0.5, py as f32 + 0.5))
                    } else {
                        // 不可逆矩阵不能产生可验证的描边像素。
                        self.apply_inverse(px as f32 + 0.5, py as f32 + 0.5)
                    }) else {
                        // 当前设备像素无法映射时直接跳过。
                        continue;
                    };
                    let outer_sd = Self::rounded_rect_sdf(ux, uy, &outer_rect, &outer_rad);
                    let coverage = if inner_is_positive {
                        let inner_sd = Self::rounded_rect_sdf(ux, uy, &inner_rect, &inner_rad);
                        Self::sdf_to_coverage(outer_sd) * Self::sdf_to_coverage(-inner_sd)
                    } else {
                        // 描边宽度盖满矩形:直接填充外扩圆角矩形。
                        Self::sdf_to_coverage(outer_sd)
                    };
                    if coverage > 0.0 {
                        self.put_pixel_aa(pixels, surface_w, surface_h, px, py, c, coverage);
                    }
                }
            }
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "shape coordinates and surface bounds mirror the canvas contract"
    )]
    pub(crate) fn stroke_circle(
        &self,
        pixels: &mut [u32],
        surface_w: i32,
        surface_h: i32,
        cx: f32,
        cy: f32,
        r: f32,
        color: Color,
        line_width: f32,
    ) {
        let (ox, oy) = (self.offset_x, self.offset_y);
        let (cx, cy) = if ox != 0.0 || oy != 0.0 {
            (cx + ox, cy + oy)
        } else {
            (cx, cy)
        };
        let lw = line_width.max(0.0);
        let c = self.apply_opa(Self::premul(color));
        let expand = r + lw * 0.5 + 1.0;
        // 保存 identity 事实，避免每个像素重复比较矩阵。
        let identity = Self::is_identity(&self.transform);
        // 构造包含线宽与抗锯齿余量的本地圆形边界。
        let local_bounds = Rect::new(
            // 本地左边界。
            cx - expand,
            // 本地上边界。
            cy - expand,
            // 本地直径。
            expand * 2.0,
            // 本地直径。
            expand * 2.0,
        );
        // 映射保守边界，但把距离计算保留在本地圆坐标中。
        let device_bounds = if identity {
            // identity 不需要额外变换。
            local_bounds
        } else {
            // 任意仿射把圆映射为椭圆或剪切轮廓。
            self.transform_rect(&local_bounds)
        };
        // 只扫描变换后边界与当前 clip 的交集。
        if let Some(cr) = self.intersect_clip(&device_bounds) {
            // 取得扫描左边界。
            let x0 = cr.x as i32;
            // 取得扫描上边界。
            let y0 = cr.y as i32;
            // 取得扫描右边界。
            let x1 = (cr.x + cr.w) as i32;
            // 取得扫描下边界。
            let y1 = (cr.y + cr.h) as i32;
            // 遍历候选设备像素行。
            for py in y0..y1 {
                // 遍历候选设备像素列。
                for px in x0..x1 {
                    // 将设备像素中心恢复到本地圆坐标。
                    let Some((ux, uy)) = (if identity {
                        // identity 保持既有设备坐标。
                        Some((px as f32 + 0.5, py as f32 + 0.5))
                    } else {
                        // 不可逆矩阵没有可验证的本地坐标。
                        self.apply_inverse(px as f32 + 0.5, py as f32 + 0.5)
                    }) else {
                        // 当前像素无法映射时跳过。
                        continue;
                    };
                    // 计算本地水平距离。
                    let dx = ux - cx;
                    // 计算本地垂直距离。
                    let dy = uy - cy;
                    // 计算到本地圆边界的有符号距离。
                    let sd = (dx * dx + dy * dy).sqrt() - r;
                    // 由本地线宽计算覆盖率，使线宽随仿射一起映射。
                    let coverage = Self::sdf_to_coverage(sd.abs() - lw * 0.5);
                    // 仅写入可见覆盖。
                    if coverage > 0.0 {
                        // 复用统一 clip、opacity 与 blend 写入。
                        self.put_pixel_aa(pixels, surface_w, surface_h, px, py, c, coverage);
                    }
                }
            }
        }
    }

    pub(crate) fn stroke_path(
        &self,
        pixels: &mut [u32],
        surface_w: i32,
        surface_h: i32,
        path: &Path,
        color: Color,
        opts: &StrokeOptions,
    ) {
        // offset 按 Canvas2D 契约先于 transform 应用。
        let (ox, oy) = (self.offset_x, self.offset_y);
        // 仅在存在 offset 时创建本地平移副本。
        let local_path = if ox != 0.0 || oy != 0.0 {
            // 把 offset 烘焙到本地路径坐标。
            &path.translated(ox, oy)
        } else {
            // 无 offset 时直接借用原路径。
            path
        };
        // opacity 在透明 scratch 中只折叠一次。
        let c = self.apply_opa(Self::premul(color));
        // 必须先在本地空间生成 width/cap/join/miter 轮廓，再执行仿射。
        let stroked = crate::draw::geometry::stroker::stroke_path(local_path, opts);
        // 空轮廓是安全 no-op。
        if stroked.is_empty() {
            // 不创建空 sampled tile。
            return;
        }
        // 非 identity transform 需要持有映射后的描边轮廓。
        let transformed_stroke;
        // 选择最终供 polygon rasterizer 消费的 surface-space 路径。
        let stroked = if Self::is_identity(&self.transform) {
            // identity 保留既有几何和像素结果。
            &stroked
        } else {
            // 整体变换本地描边轮廓，使各向异性缩放和剪切同时作用于线宽。
            transformed_stroke = stroked.transformed(self.transform);
            // 借用当前调用内有效的 surface-space 轮廓。
            &transformed_stroke
        };
        // 在最终 surface-space 轮廓上执行共享 flatten。
        let polys = crate::draw::geometry::flattener::flatten(stroked.segments(), 0.25);
        let mut global_edges = Vec::new();
        let mut active_edges = Vec::new();
        let clip = self.clip_rect;
        crate::draw::raster::rasterizer::polygon::fill_polygons(
            &polys,
            pixels,
            surface_w,
            surface_h,
            clip,
            c,
            FillRule::NonZero,
            &mut global_edges,
            &mut active_edges,
        );
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "line endpoints and surface bounds mirror the canvas contract"
    )]
    pub(crate) fn draw_line(
        &self,
        pixels: &mut [u32],
        surface_w: i32,
        surface_h: i32,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: Color,
        width: f32,
    ) {
        // 线宽先在本地空间归一化，再随完整仿射映射。
        let half_lw = width.max(0.0) * 0.5;
        // 垂直直线沿用矩形语义，从而保留既有 butt 端点并获得完整 transform。
        if (x1 - x2).abs() < 1e-6 {
            // 构造本地垂直线段矩形。
            let rect = Rect::new(
                // 左边界位于中心线左侧半个线宽。
                x1 - half_lw,
                // 上边界取两个端点的较小值。
                y1.min(y2),
                // 矩形宽度等于线宽。
                width.max(0.0),
                // 矩形高度等于线段长度。
                (y1 - y2).abs(),
            );
            // 复用 fill_rect 的 offset、transform、clip、opacity 与 blend 语义。
            self.fill_rect(pixels, surface_w, surface_h, rect, color, None);
            // 轴对齐线段已经完整处理。
            return;
        }
        // 水平直线同样以本地矩形保留 butt 端点。
        if (y1 - y2).abs() < 1e-6 {
            // 构造本地水平线段矩形。
            let rect = Rect::new(
                // 左边界取两个端点的较小值。
                x1.min(x2),
                // 上边界位于中心线上方半个线宽。
                y1 - half_lw,
                // 矩形宽度等于线段长度。
                (x1 - x2).abs(),
                // 矩形高度等于线宽。
                width.max(0.0),
            );
            // 复用 fill_rect 的完整状态映射。
            self.fill_rect(pixels, surface_w, surface_h, rect, color, None);
            // 轴对齐线段已经完整处理。
            return;
        }
        // 非轴对齐线段先应用本地 offset。
        let (ox, oy) = (self.offset_x, self.offset_y);
        // 生成带 offset 的第一个本地端点横坐标。
        let x1 = x1 + ox;
        // 生成带 offset 的第一个本地端点纵坐标。
        let y1 = y1 + oy;
        // 生成带 offset 的第二个本地端点横坐标。
        let x2 = x2 + ox;
        // 生成带 offset 的第二个本地端点纵坐标。
        let y2 = y2 + oy;
        // 在透明 scratch 中折叠 opacity。
        let c = self.apply_opa(Self::premul(color));
        // 抗锯齿扫描边界在本地线宽之外保留一个像素。
        let expand = half_lw + 1.0;
        // 构造本地保守边界。
        let local_bounds = Rect::new(
            // 本地左边界。
            x1.min(x2) - expand,
            // 本地上边界。
            y1.min(y2) - expand,
            // 本地宽度。
            (x1 - x2).abs() + expand * 2.0,
            // 本地高度。
            (y1 - y2).abs() + expand * 2.0,
        );
        // 缓存 identity 事实，避免像素循环重复判断。
        let identity = Self::is_identity(&self.transform);
        // 把保守写区映射到 surface。
        let device_bounds = if identity {
            // identity 不需要映射。
            local_bounds
        } else {
            // 完整仿射可把本地 capsule 变为缩放或剪切轮廓。
            self.transform_rect(&local_bounds)
        };
        // 只扫描设备边界与 clip 的交集。
        if let Some(cr) = self.intersect_clip(&device_bounds) {
            // 取得扫描左边界。
            let x0 = cr.x as i32;
            // 取得扫描上边界。
            let y0 = cr.y as i32;
            // 取得扫描右边界。
            let x1b = (cr.x + cr.w) as i32;
            // 取得扫描下边界。
            let y1b = (cr.y + cr.h) as i32;
            // 遍历候选设备像素行。
            for py in y0..y1b {
                // 遍历候选设备像素列。
                for px in x0..x1b {
                    // 把设备像素中心恢复到本地线段坐标。
                    let Some((ux, uy)) = (if identity {
                        // identity 保持既有像素中心。
                        Some((px as f32 + 0.5, py as f32 + 0.5))
                    } else {
                        // 不可逆矩阵没有可验证的本地坐标。
                        self.apply_inverse(px as f32 + 0.5, py as f32 + 0.5)
                    }) else {
                        // 当前像素无法逆映射时跳过。
                        continue;
                    };
                    // 计算本地 capsule 描边的有符号距离。
                    let sd = Self::line_segment_sdf(ux, uy, x1, y1, x2, y2) - half_lw;
                    // 把距离转换为覆盖率。
                    let coverage = Self::sdf_to_coverage(sd);
                    // 仅写入可见覆盖。
                    if coverage > 0.0 {
                        // 复用统一 clip、opacity 与 blend 写入。
                        self.put_pixel_aa(pixels, surface_w, surface_h, px, py, c, coverage);
                    }
                }
            }
        }
    }
}
