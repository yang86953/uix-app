//! SoftwareRasterizer 渐变填充实现。

// 引入局部和设备空间共同使用的矩形。
use crate::core::Rect;
// 引入渐变端点颜色。
use crate::draw::geometry::color::Color;
// 引入有限的线性渐变方向集合与圆角值。
use crate::draw::geometry::types::{GradientDirection, Radius};
// 引入与填充/阴影共用的圆角归一化与覆盖曲线。
use super::rasterizer::core::{normalize_corner_radius, rounded_rect_sdf};

// 复用持有 transform、clip、opacity 与 blend 状态的软件执行器。
use super::software_rasterizer::SoftwareRasterizer;

// 按既有 8-bit 通道规则插值渐变颜色。
fn mix_color(start: Color, end: Color, t: f32) -> Color {
    // Color::mix 会把插值参数稳定限制在渐变区间内。
    start.mix(&end, t)
}

// 计算指定方向在线性渐变局部矩形中的归一化位置。
fn linear_gradient_t(rect: Rect, x: f32, y: f32, direction: GradientDirection) -> f32 {
    // 把采样点转换到矩形左上角为原点的局部坐标。
    let local_x = x - rect.x;
    // 垂直方向同样保持局部坐标。
    let local_y = y - rect.y;
    // 沿公开方向枚举复用既有渐变参数公式。
    let t = match direction {
        // 水平渐变按局部宽度归一化。
        GradientDirection::Horizontal => local_x / rect.w.max(1.0),
        // 垂直渐变按局部高度归一化。
        GradientDirection::Vertical => local_y / rect.h.max(1.0),
        // 左上到右下渐变沿两个局部轴的和推进。
        GradientDirection::DiagonalTLBR => {
            // 宽高和保持与原纯函数实现相同的退化保护。
            (local_x + local_y) / (rect.w + rect.h).max(1.0)
        }
        // 左下到右上渐变沿一正一负两个局部轴推进。
        GradientDirection::DiagonalBLTR => {
            // 高度偏移把左下角稳定映射到渐变起点。
            (local_x - local_y + rect.h) / (rect.w + rect.h).max(1.0)
        }
    };
    // 最终颜色限制在两个端点之间。
    t.clamp(0.0, 1.0)
}

// 判断逆映射后的像素中心是否仍位于线性渐变局部矩形内。
fn rect_contains(rect: Rect, x: f32, y: f32) -> bool {
    // 左上边界闭合、右下边界开放，保持逐像素填充约定。
    x >= rect.x && y >= rect.y && x < rect.x + rect.w && y < rect.y + rect.h
}

// 把设备空间浮点边界转换成不会截掉旋转或剪切像素中心的扫描区间。
fn scan_bounds(rect: Rect) -> (i32, i32, i32, i32) {
    // 左边界向外取整。
    let x0 = rect.x.floor() as i32;
    // 上边界向外取整。
    let y0 = rect.y.floor() as i32;
    // 右边界向外取整。
    let x1 = (rect.x + rect.w).ceil() as i32;
    // 下边界向外取整。
    let y1 = (rect.y + rect.h).ceil() as i32;
    // 返回半开扫描区间。
    (x0, y0, x1, y1)
}

// 渐变参数与目标尺寸共同构成软件栅格原语契约。
#[allow(
    // 两种渐变都需要显式接收目标、几何与颜色参数。
    clippy::too_many_arguments,
    // 保留 Canvas2D 参数形状可避免临时载荷和重复转换。
    reason = "gradient geometry and surface bounds mirror the canvas contract"
)]
// 在当前软件状态下绘制线性渐变。
impl SoftwareRasterizer {
    // 线性渐变在设备像素中心逆映射后计算局部颜色。
    pub(crate) fn fill_linear_gradient(
        // 只读取当前状态，不改变 save/restore 栈。
        &self,
        // 写入调用方持有的像素目标。
        pixels: &mut [u32],
        // 传入目标宽度用于安全寻址。
        surface_w: i32,
        // 传入目标高度用于安全寻址。
        surface_h: i32,
        // 线性渐变的局部矩形。
        rect: Rect,
        // 渐变起点颜色。
        color_a: Color,
        // 渐变终点颜色。
        color_b: Color,
        // 渐变推进方向。
        direction: GradientDirection,
        // 圆角裁剪半径；零值保持无掩码旧行为。
        radius: Radius,
    ) {
        // Canvas2D 约定先应用像素 offset，再执行 transform。
        let local_rect = Rect::new(
            // 水平 offset 属于局部几何。
            rect.x + self.offset_x,
            // 垂直 offset 属于局部几何。
            rect.y + self.offset_y,
            // offset 不改变局部宽度。
            rect.w,
            // offset 不改变局部高度。
            rect.h,
        );
        // 把四个局部角映射成保守设备空间 AABB。
        let device_bounds = self.transform_rect(&local_rect);
        // 完全被当前 surface/path clip 排除时无需扫描。
        let Some(clipped) = self.intersect_clip(&device_bounds) else {
            // 保持像素目标不变。
            return;
        };
        // 只遍历变换后写区覆盖的设备像素。
        let (x0, y0, x1, y1) = scan_bounds(clipped);
        // 掩码半径按相邻和规则归一化，与填充/阴影轮廓保持同一规则。
        let corner = normalize_corner_radius(rect.w, rect.h, radius);
        let has_radius =
            corner.tl > 0.0 || corner.tr > 0.0 || corner.br > 0.0 || corner.bl > 0.0;
        // 逐行保持稳定的 painter 顺序。
        for py in y0..y1 {
            // 逐列采样设备像素中心。
            for px in x0..x1 {
                // 任意可逆仿射都在局部渐变空间求值。
                let Some((local_x, local_y)) =
                    // 像素中心避免整数边界产生方向偏差。
                    self.apply_inverse(px as f32 + 0.5, py as f32 + 0.5)
                else {
                    // 退化 transform 不产生不可证明的像素。
                    continue;
                };
                // AABB 外扩部分不得获得渐变颜色。
                if !rect_contains(local_rect, local_x, local_y) {
                    // 跳过原矩形外的设备像素。
                    continue;
                }
                // 在逆映射后的局部点计算方向参数。
                let t = linear_gradient_t(local_rect, local_x, local_y, direction);
                // 先插值非预乘颜色以保持既有通道规则。
                let color = mix_color(color_a, color_b, t);
                // 再按当前有限 opacity 生成预乘源贡献。
                let premultiplied = self.apply_opa(Self::premul(color));
                // 圆角掩码与渐变色在同一局部空间求值；零半径保持满覆盖。
                let coverage = if has_radius {
                    Self::sdf_to_coverage(rounded_rect_sdf(local_x, local_y, &local_rect, &corner))
                } else {
                    1.0
                };
                // 完全落在圆角外的像素不产生源贡献。
                if coverage <= 0.0 {
                    continue;
                }
                // 统一入口负责矩形/path clip 与 SrcOver/Additive 混合。
                self.put_pixel_aa(
                    // 目标像素缓冲。
                    pixels,
                    // 目标宽度。
                    surface_w,
                    // 目标高度。
                    surface_h,
                    // 当前设备像素横坐标。
                    px,
                    // 当前设备像素纵坐标。
                    py,
                    // 已烘焙 opacity 的预乘颜色。
                    premultiplied,
                    // 圆角掩码覆盖与既有渐变满覆盖共用同一入口。
                    coverage,
                );
            }
        }
    }

    // 径向渐变在局部圆内按半径距离插值，再由 transform 映射为椭圆或剪切形状。
    #[allow(
        // 半径、颜色与目标尺寸均属于公开 Canvas2D 契约。
        clippy::too_many_arguments,
        // 直接接收参数可保持调用端和线性渐变一致。
        reason = "gradient geometry and surface bounds mirror the canvas contract"
    )]
    // 在当前软件状态下绘制径向渐变。
    pub(crate) fn fill_radial_gradient(
        // 只读取当前状态。
        &self,
        // 写入调用方持有的像素目标。
        pixels: &mut [u32],
        // 目标宽度用于安全寻址。
        surface_w: i32,
        // 目标高度用于安全寻址。
        surface_h: i32,
        // 局部圆心水平坐标。
        cx: f32,
        // 局部圆心垂直坐标。
        cy: f32,
        // 内圈半径。
        inner_r: f32,
        // 外圈半径。
        outer_r: f32,
        // 内圈颜色。
        inner_color: Color,
        // 外圈颜色。
        outer_color: Color,
        // 圆角裁剪的参考矩形（局部坐标）。
        clip_rect: Rect,
        // 圆角裁剪半径；零值保持无掩码旧行为。
        radius: Radius,
    ) {
        // 非正外半径沿用既有安全 no-op 行为。
        if outer_r <= 0.0 {
            // 不产生源贡献。
            return;
        }
        // offset 必须在 transform 之前平移局部圆心。
        let local_cx = cx + self.offset_x;
        // 垂直 offset 使用相同顺序。
        let local_cy = cy + self.offset_y;
        // 构造完整覆盖局部外圆的正方形。
        let local_bounds = Rect::new(
            // 左边界位于圆心减外半径。
            local_cx - outer_r,
            // 上边界位于圆心减外半径。
            local_cy - outer_r,
            // 水平直径覆盖完整外圆。
            outer_r * 2.0,
            // 垂直直径覆盖完整外圆。
            outer_r * 2.0,
        );
        // 任意仿射后的外圆写区由四角 AABB 保守覆盖。
        let device_bounds = self.transform_rect(&local_bounds);
        // 完全不可见时保持 no-op。
        let Some(clipped) = self.intersect_clip(&device_bounds) else {
            // 无需触碰目标。
            return;
        };
        // 半径差与既有渐变公式一致。
        let range = outer_r - inner_r;
        // 参考矩形与圆心一样先叠加 offset，保持同一局部空间。
        let mask_rect = Rect::new(
            clip_rect.x + self.offset_x,
            clip_rect.y + self.offset_y,
            clip_rect.w,
            clip_rect.h,
        );
        // 掩码半径按相邻和规则归一化，与填充/阴影轮廓保持同一规则。
        let corner = normalize_corner_radius(clip_rect.w, clip_rect.h, radius);
        let has_radius =
            corner.tl > 0.0 || corner.tr > 0.0 || corner.br > 0.0 || corner.bl > 0.0;
        // 取得不会截断仿射边缘的设备扫描区间。
        let (x0, y0, x1, y1) = scan_bounds(clipped);
        // 按稳定行序扫描设备目标。
        for py in y0..y1 {
            // 逐像素逆映射到局部圆。
            for px in x0..x1 {
                // 读取局部采样点。
                let Some((local_x, local_y)) =
                    // 以设备像素中心执行逆映射。
                    self.apply_inverse(px as f32 + 0.5, py as f32 + 0.5)
                else {
                    // 不可逆变换不产生像素。
                    continue;
                };
                // 计算局部圆空间中的水平距离。
                let dx = local_x - local_cx;
                // 计算局部圆空间中的垂直距离。
                let dy = local_y - local_cy;
                // 仿射只改变映射，不改变局部径向距离定义。
                let distance = (dx * dx + dy * dy).sqrt();
                // 外圆之外不属于渐变源。
                if distance > outer_r {
                    // 跳过 AABB 中的圆外区域。
                    continue;
                }
                // 圆角掩码在参考矩形局部空间求值；零半径保持满覆盖。
                let coverage = if has_radius {
                    Self::sdf_to_coverage(rounded_rect_sdf(
                        local_x,
                        local_y,
                        &mask_rect,
                        &corner,
                    ))
                } else {
                    1.0
                };
                // 完全落在圆角外的像素不产生源贡献。
                if coverage <= 0.0 {
                    continue;
                }
                // 把局部距离映射到内外圈之间。
                let t = ((distance - inner_r) / range).clamp(0.0, 1.0);
                // 按既有非预乘通道插值生成颜色。
                let color = mix_color(inner_color, outer_color, t);
                // 全局 opacity 在源 tile 中仅烘焙一次。
                let premultiplied = self.apply_opa(Self::premul(color));
                // 统一像素入口保留 clip、path mask 与实际 blend。
                self.put_pixel_aa(
                    // 目标像素缓冲。
                    pixels,
                    // 目标宽度。
                    surface_w,
                    // 目标高度。
                    surface_h,
                    // 当前设备像素横坐标。
                    px,
                    // 当前设备像素纵坐标。
                    py,
                    // 已预乘并应用 opacity 的源颜色。
                    premultiplied,
                    // 圆角掩码覆盖；无掩码时保持完整 coverage。
                    coverage,
                );
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../tests-src/draw/raster/s4_gradient_mask_tests.rs"]
mod s4_gradient_mask_tests;
