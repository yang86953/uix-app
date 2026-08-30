//! SoftwareRasterizer 字形 coverage 采样实现。

// 引入局部字形和设备写区使用的矩形。
use crate::core::Rect;
// 引入字形的直通颜色。
use crate::draw::geometry::color::Color;
// 复用共享 premul 量化与 coverage 调制规则（参考执行同一函数）。
use crate::draw::raster::rasterizer::{color_to_premul, modulate_coverage};

// 复用持有 offset、transform、clip、opacity 与 blend 的软件执行器。
use super::software_rasterizer::SoftwareRasterizer;

// 判断逆映射后的像素中心是否仍位于局部字形矩形。
fn target_contains(rect: Rect, x: f32, y: f32) -> bool {
    // 使用左上闭合、右下开放的半开矩形规则。
    x >= rect.x && y >= rect.y && x < rect.x + rect.w && y < rect.y + rect.h
}

// 把设备浮点边界转换为不会截掉旋转或剪切像素中心的扫描区间。
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

// surface、coverage 与字形几何共同构成软件采样契约。
#[allow(
    // Canvas2D 字形入口需要显式传入所有 coverage 和目标事实。
    clippy::too_many_arguments,
    // 直接镜像公开契约可避免额外分配临时载荷。
    reason = "glyph coverage, destination, and surface bounds mirror the canvas contract"
)]
// 为共享软件执行器补充状态完整的字形 coverage 采样入口。
impl SoftwareRasterizer {
    // 把设备像素中心逆映射到 offset 后局部字形并执行 point sampling。
    pub(crate) fn blit_glyph(
        // 只读取当前绘制状态。
        &self,
        // 写入调用方持有的像素目标。
        pixels: &mut [u32],
        // 目标宽度用于安全寻址。
        surface_w: i32,
        // 目标高度用于安全寻址。
        surface_h: i32,
        // 字形本地横坐标。
        x: i32,
        // 字形本地纵坐标。
        y: i32,
        // 单通道 coverage 载荷。
        coverage: &[u8],
        // coverage 行宽。
        width: usize,
        // coverage 行数。
        height: usize,
        // 字形直通颜色。
        color: Color,
    ) {
        // 非正目标尺寸或空字形不能形成有效像素。
        if surface_w <= 0 || surface_h <= 0 || width == 0 || height == 0 {
            // 保持安全 no-op。
            return;
        }
        // 目标载荷必须至少覆盖声明的完整 surface。
        let Some(surface_len) = (surface_w as usize).checked_mul(surface_h as usize) else {
            // 地址空间溢出时不写越界像素。
            return;
        };
        // coverage 数量乘法必须可表达。
        let Some(coverage_len) = width.checked_mul(height) else {
            // 畸形字形保持安全 no-op。
            return;
        };
        // 短目标或短 coverage 都不能安全寻址。
        if pixels.len() < surface_len || coverage.len() < coverage_len {
            // 保持两个载荷不变。
            return;
        }
        // Canvas2D 顺序要求 offset 在 transform 之前应用。
        let local_target = Rect::new(
            // 水平 offset 移动局部字形。
            x as f32 + self.offset_x,
            // 垂直 offset 移动局部字形。
            y as f32 + self.offset_y,
            // coverage 宽度定义局部字形宽度。
            width as f32,
            // coverage 高度定义局部字形高度。
            height as f32,
        );
        // 非有限 offset 无法形成稳定采样坐标。
        if !local_target.x.is_finite() || !local_target.y.is_finite() {
            // 非法几何保持 no-op。
            return;
        }
        // 先按历史顺序量化 color alpha 与全局 opacity，再做预乘。
        let premultiplied = color_to_premul(
            // 保留红通道。
            color.r,
            // 保留绿通道。
            color.g,
            // 保留蓝通道。
            color.b,
            // 保留颜色自身 alpha。
            color.a,
            // 全局 opacity 只应用一次。
            self.opacity(),
        );
        // 量化为透明时不扫描设备像素。
        if premultiplied == 0 {
            // 保持目标不变。
            return;
        }
        // 映射局部四角得到设备空间保守 AABB。
        let device_bounds = self.transform_rect(&local_target);
        // 完全位于当前 clip 外时无需扫描。
        let Some(clipped) = self.intersect_clip(&device_bounds) else {
            // 保持目标不变。
            return;
        };
        // 取得覆盖整个仿射写区的整数扫描范围。
        let (x0, y0, x1, y1) = scan_bounds(clipped);
        // 按稳定行序遍历设备像素。
        for target_y in y0..y1 {
            // 逐列逆映射像素中心。
            for target_x in x0..x1 {
                // 任意可逆仿射都在局部字形空间决定 coverage texel。
                let Some((local_x, local_y)) =
                    // 采用设备像素中心避免整数边界方向偏差。
                    self.apply_inverse(target_x as f32 + 0.5, target_y as f32 + 0.5)
                else {
                    // 退化 transform 不产生不可证明的像素。
                    continue;
                };
                // AABB 中但局部字形外的像素不得读取 coverage。
                if !target_contains(local_target, local_x, local_y) {
                    // 跳过局部矩形外部。
                    continue;
                }
                // point sampling 选择局部字形中的 coverage 列。
                let source_x = ((local_x - local_target.x).floor() as usize).min(width - 1);
                // point sampling 选择局部字形中的 coverage 行。
                let source_y = ((local_y - local_target.y).floor() as usize).min(height - 1);
                // 已验证的紧行索引安全读取 coverage。
                let sampled = coverage[source_y * width + source_x];
                // 完全透明 coverage 不产生源贡献。
                if sampled == 0 {
                    // 跳过透明 texel。
                    continue;
                }
                // coverage 在预乘颜色上只调制一次。
                let source_pixel = modulate_coverage(premultiplied, sampled);
                // 共享入口执行矩形/path clip 与 SrcOver/Additive 混合。
                self.put_pixel_aa(
                    // 目标像素缓冲。
                    pixels,
                    // 目标宽度。
                    surface_w,
                    // 目标高度。
                    surface_h,
                    // 当前设备横坐标。
                    target_x,
                    // 当前设备纵坐标。
                    target_y,
                    // 已应用 opacity 与 glyph coverage 的预乘颜色。
                    source_pixel,
                    // coverage 已经完成整数调制，避免重复相乘。
                    1.0,
                );
            }
        }
    }
}
