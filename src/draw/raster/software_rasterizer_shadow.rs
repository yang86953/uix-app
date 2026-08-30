//! SoftwareRasterizer 盒阴影采样实现。

// 引入局部阴影和设备写区使用的矩形。
use crate::core::Rect;
// 引入阴影直通颜色。
use crate::draw::geometry::color::Color;
// 引入四角半径定义。
use crate::draw::geometry::types::Radius;
// 复用既有阴影 SDF、coverage 与颜色量化规则。
use crate::draw::raster::rasterizer::{
    // identity 圆角阴影保持既有像素网格对齐。
    align_rounded_rect,
    // opacity 在颜色 alpha 上先量化再预乘。
    color_to_premul,
    // 圆角矩形局部有符号距离。
    rounded_rect_sdf,
    // 无模糊阴影使用标准抗锯齿 coverage。
    sdf_to_coverage,
    // 定向阴影使用平滑 blur coverage。
    shadow_coverage,
    // 环境阴影使用更柔和的 coverage 曲线。
    shadow_coverage_ambient,
};

// 复用持有 offset、transform、clip、opacity 与 blend 状态的软件执行器。
use super::software_rasterizer::SoftwareRasterizer;

// 把设备浮点边界转换为不会截掉仿射阴影像素中心的扫描区间。
fn scan_bounds(rect: Rect) -> (i32, i32, i32, i32) {
    // 左边界向外取整。
    let x0 = rect.x.floor() as i32;
    // 上边界向外取整。
    let y0 = rect.y.floor() as i32;
    // 右边界向外取整。
    let x1 = (rect.x + rect.w).ceil() as i32;
    // 下边界向外取整。
    let y1 = (rect.y + rect.h).ceil() as i32;
    // 返回半开扫描范围。
    (x0, y0, x1, y1)
}

// 阴影参数与目标尺寸共同构成软件采样契约。
#[allow(
    // Canvas2D 阴影入口需要显式传入目标、几何和效果参数。
    clippy::too_many_arguments,
    // 直接镜像公开契约可避免额外分配临时载荷。
    reason = "shadow geometry, effect, and surface bounds mirror the canvas contract"
)]
// 为共享软件执行器补充状态完整的两种盒阴影入口。
impl SoftwareRasterizer {
    // 定向阴影使用标准 shadow coverage 曲线。
    pub(crate) fn draw_box_shadow(
        // 只读取当前绘制状态。
        &self,
        // 写入调用方持有的像素目标。
        pixels: &mut [u32],
        // 目标宽度用于安全寻址。
        surface_w: i32,
        // 目标高度用于安全寻址。
        surface_h: i32,
        // 阴影来源矩形位于局部空间。
        rect: Rect,
        // 模糊半径同样属于局部空间。
        blur: f32,
        // 定向阴影的局部水平偏移。
        offset_x: f32,
        // 定向阴影的局部垂直偏移。
        offset_y: f32,
        // 阴影直通颜色。
        color: Color,
        // 可选的四角半径。
        radius: Option<Radius>,
    ) {
        // 复用单一实现并选择定向 coverage。
        self.draw_box_shadow_impl(
            // 传入目标像素。
            {
                // 目标像素不做复制。
                pixels
            },
            // 传入目标宽度。
            {
                // 宽度保持调用方事实。
                surface_w
            },
            // 传入目标高度。
            {
                // 高度保持调用方事实。
                surface_h
            },
            // 传入局部矩形。
            {
                // 几何保持局部坐标。
                rect
            },
            // 传入局部 blur。
            {
                // 模糊半径稍后统一规范化。
                blur
            },
            // 传入局部水平偏移。
            {
                // 水平偏移在 transform 前应用。
                offset_x
            },
            // 传入局部垂直偏移。
            {
                // 垂直偏移在 transform 前应用。
                offset_y
            },
            // 传入阴影颜色。
            {
                // 颜色稍后折叠 opacity。
                color
            },
            // 传入角半径。
            {
                // 半径保持每角事实。
                radius
            },
            // false 选择定向 coverage。
            false,
        );
    }

    // 环境阴影使用柔和的 ambient coverage 曲线。
    pub(crate) fn draw_box_shadow_ambient(
        // 只读取当前绘制状态。
        &self,
        // 写入调用方持有的像素目标。
        pixels: &mut [u32],
        // 目标宽度用于安全寻址。
        surface_w: i32,
        // 目标高度用于安全寻址。
        surface_h: i32,
        // 阴影来源矩形位于局部空间。
        rect: Rect,
        // 模糊半径位于局部空间。
        blur: f32,
        // 环境阴影的局部水平偏移。
        offset_x: f32,
        // 环境阴影的局部垂直偏移。
        offset_y: f32,
        // 阴影直通颜色。
        color: Color,
        // 可选的四角半径。
        radius: Option<Radius>,
    ) {
        // 复用单一实现并选择环境 coverage。
        self.draw_box_shadow_impl(
            // 传入目标像素。
            {
                // 目标像素不做复制。
                pixels
            },
            // 传入目标宽度。
            {
                // 宽度保持调用方事实。
                surface_w
            },
            // 传入目标高度。
            {
                // 高度保持调用方事实。
                surface_h
            },
            // 传入局部矩形。
            {
                // 几何保持局部坐标。
                rect
            },
            // 传入局部 blur。
            {
                // 模糊半径稍后统一规范化。
                blur
            },
            // 传入局部水平偏移。
            {
                // 水平偏移在 transform 前应用。
                offset_x
            },
            // 传入局部垂直偏移。
            {
                // 垂直偏移在 transform 前应用。
                offset_y
            },
            // 传入阴影颜色。
            {
                // 颜色稍后折叠 opacity。
                color
            },
            // 传入角半径。
            {
                // 半径保持每角事实。
                radius
            },
            // true 选择环境 coverage。
            true,
        );
    }

    // 在局部阴影空间求值，再把设备像素中心逆映射后混合。
    #[allow(
        // 内部共享实现需要保留公开阴影入口的全部参数。
        clippy::too_many_arguments,
        // 单一实现避免定向与环境阴影复制状态和边界逻辑。
        reason = "shared shadow sampling keeps directed and ambient semantics aligned"
    )]
    // 执行共同的仿射阴影采样流程。
    fn draw_box_shadow_impl(
        // 只读取当前执行器状态。
        &self,
        // 写入目标像素。
        pixels: &mut [u32],
        // 目标宽度。
        surface_w: i32,
        // 目标高度。
        surface_h: i32,
        // 来源局部矩形。
        rect: Rect,
        // 局部模糊半径。
        blur_radius: f32,
        // 局部效果水平偏移。
        offset_x: f32,
        // 局部效果垂直偏移。
        offset_y: f32,
        // 阴影颜色。
        color: Color,
        // 可选四角半径。
        radius: Option<Radius>,
        // 是否使用环境阴影曲线。
        ambient: bool,
    ) {
        // 非正目标、空矩形或透明颜色不产生源贡献。
        if surface_w <= 0 || surface_h <= 0 || rect.w <= 0.0 || rect.h <= 0.0 || color.a == 0 {
            // 保持像素目标不变。
            return;
        }
        // 目标载荷长度乘法必须可表达。
        let Some(surface_len) = (surface_w as usize).checked_mul(surface_h as usize) else {
            // 地址空间溢出时保持安全 no-op。
            return;
        };
        // 短目标不能被安全寻址。
        if pixels.len() < surface_len {
            // 保持畸形载荷不变。
            return;
        }
        // 非有限几何或效果偏移不能形成稳定局部采样空间。
        if !rect.x.is_finite()
            // 检查局部纵坐标。
            || !rect.y.is_finite()
            // 检查局部宽度。
            || !rect.w.is_finite()
            // 检查局部高度。
            || !rect.h.is_finite()
            // 检查模糊半径。
            || !blur_radius.is_finite()
            // 检查效果水平偏移。
            || !offset_x.is_finite()
            // 检查效果垂直偏移。
            || !offset_y.is_finite()
        {
            // 非法输入保持历史安全 no-op 边界。
            return;
        }
        // 负模糊半径沿用既有零模糊语义。
        let blur = blur_radius.max(0.0);
        // Canvas offset 与阴影 offset 都必须在 transform 前应用。
        let mut shadow_rect = Rect::new(
            // 水平位置包含画布和效果两个局部偏移。
            rect.x + self.offset_x + offset_x,
            // 垂直位置包含画布和效果两个局部偏移。
            rect.y + self.offset_y + offset_y,
            // 局部宽度不受偏移影响。
            rect.w,
            // 局部高度不受偏移影响。
            rect.h,
        );
        // 缺省角半径表示直角矩形；规范化与 GPU shadow lowering 同一 CSS 式规则。
        let radius =
            crate::draw::raster::rasterizer::core::normalize_corner_radius(rect.w, rect.h, radius.unwrap_or_default());
        // identity 下保持旧实现的圆角像素网格对齐。
        if Self::is_identity(&self.transform)
            // 只有实际圆角才需要对齐。
            && (radius.tl != 0.0 || radius.tr != 0.0 || radius.bl != 0.0 || radius.br != 0.0)
        {
            // 无法形成有效对齐矩形时保持 no-op。
            let Some(aligned) = align_rounded_rect(shadow_rect) else {
                // 不写入不确定像素。
                return;
            };
            // 后续 SDF 使用对齐后的局部矩形。
            shadow_rect = aligned;
        }
        // 模糊与抗锯齿在局部空间共同扩展写区。
        let expand = blur + 1.0;
        // 构造完整覆盖阴影源贡献的局部 AABB。
        let local_bounds = Rect::new(
            // 扩展左边界。
            shadow_rect.x - expand,
            // 扩展上边界。
            shadow_rect.y - expand,
            // 同时扩展左右两侧。
            shadow_rect.w + expand * 2.0,
            // 同时扩展上下两侧。
            shadow_rect.h + expand * 2.0,
        );
        // 映射四角得到任意仿射后的保守设备 AABB。
        let device_bounds = self.transform_rect(&local_bounds);
        // 完全被当前矩形或路径裁剪排除时无需扫描。
        let Some(clipped) = self.intersect_clip(&device_bounds) else {
            // 保持目标不变。
            return;
        };
        // 按旧阴影顺序先量化 color alpha 与全局 opacity，再做预乘。
        let premultiplied = color_to_premul(
            // 保留红通道。
            color.r,
            // 保留绿通道。
            color.g,
            // 保留蓝通道。
            color.b,
            // 保留颜色 alpha。
            color.a,
            // 全局 opacity 只应用一次。
            self.opacity(),
        );
        // 完全透明的量化结果无需扫描。
        if premultiplied == 0 {
            // 保持目标不变。
            return;
        }
        // 大于半像素的 blur 使用阴影专用平滑曲线。
        let use_blur = blur > 0.5;
        // 取得不会截断旋转或剪切边缘的设备扫描范围。
        let (x0, y0, x1, y1) = scan_bounds(clipped);
        // 按稳定行序扫描设备目标。
        for py in y0..y1 {
            // 逐列逆映射设备像素中心。
            for px in x0..x1 {
                // 任意可逆仿射都在局部阴影空间求值。
                let Some((local_x, local_y)) =
                    // 像素中心避免整数边界产生方向偏差。
                    self.apply_inverse(px as f32 + 0.5, py as f32 + 0.5)
                else {
                    // 退化 transform 不产生不可证明的像素。
                    continue;
                };
                // 计算局部圆角矩形的有符号距离。
                let distance = rounded_rect_sdf(local_x, local_y, &shadow_rect, &radius);
                // 按效果类型和 blur 阈值选择既有 coverage 规则。
                let coverage = if use_blur {
                    // 环境阴影只替换 coverage 曲线。
                    if ambient {
                        // 使用更柔和的环境阴影衰减。
                        shadow_coverage_ambient(distance, blur)
                    } else {
                        // 使用标准定向阴影衰减。
                        shadow_coverage(distance, blur)
                    }
                } else {
                    // 零或极小 blur 保持普通 SDF 抗锯齿。
                    sdf_to_coverage(distance)
                };
                // 完全透明 coverage 不产生源贡献。
                if coverage <= 0.0 {
                    // 跳过阴影外部像素。
                    continue;
                }
                // 统一入口处理矩形/path clip 与 SrcOver/Additive 混合。
                self.put_pixel_aa(
                    // 目标像素缓冲。
                    pixels,
                    // 目标宽度。
                    surface_w,
                    // 目标高度。
                    surface_h,
                    // 当前设备横坐标。
                    px,
                    // 当前设备纵坐标。
                    py,
                    // 已应用 opacity 的预乘阴影颜色。
                    premultiplied,
                    // SDF 阴影 coverage 只应用一次。
                    coverage,
                );
            }
        }
    }
}
