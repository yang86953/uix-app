//! SoftwareRasterizer 图片采样实现。

// 引入局部目标和设备写区使用的矩形。
use crate::core::Rect;

// 复用持有 offset、transform、clip、opacity 与 blend 的软件执行器。
use super::software_rasterizer::SoftwareRasterizer;

// 保存经过源图片边界裁剪后的整数 crop。
struct SourceCrop {
    // 源图片行跨度。
    stride: usize,
    // crop 左边界。
    x: usize,
    // crop 上边界。
    y: usize,
    // crop 宽度。
    width: usize,
    // crop 高度。
    height: usize,
}

// 把公开浮点源矩形收敛到真实图片内的整数 nearest-sampling crop。
fn source_crop(src: &[u32], src_w: i32, rect: Rect) -> Option<SourceCrop> {
    // 非正行跨度或空图片没有可采样源。
    if src_w <= 0 || src.is_empty() {
        // 保持安全 no-op。
        return None;
    }
    // 非有限或非正 crop 无法形成稳定采样坐标。
    if !rect.x.is_finite()
        // 检查垂直起点。
        || !rect.y.is_finite()
        // 检查 crop 宽度。
        || !rect.w.is_finite()
        // 检查 crop 高度。
        || !rect.h.is_finite()
        // 拒绝空水平范围。
        || rect.w <= 0.0
        // 拒绝空垂直范围。
        || rect.h <= 0.0
    {
        // 非法源矩形保持 no-op。
        return None;
    }
    // 已验证的正跨度可以无损转成 usize。
    let stride = src_w as usize;
    // 忽略不足一整行的尾部像素，保持既有图片高度契约。
    let source_height = src.len() / stride;
    // 没有完整源行时不能采样。
    if source_height == 0 {
        // 保持安全 no-op。
        return None;
    }
    // 左边界向零取整并限制到源宽度。
    let x0 = (rect.x as f64).clamp(0.0, stride as f64) as usize;
    // 上边界向零取整并限制到源高度。
    let y0 = (rect.y as f64).clamp(0.0, source_height as f64) as usize;
    // 右边界沿用既有浮点相加后向零取整规则。
    let x1 = (rect.x as f64 + rect.w as f64).clamp(0.0, stride as f64) as usize;
    // 下边界沿用相同规则。
    let y1 = (rect.y as f64 + rect.h as f64).clamp(0.0, source_height as f64) as usize;
    // 空交集不产生源贡献。
    if x0 >= x1 || y0 >= y1 {
        // 保持安全 no-op。
        return None;
    }
    // 返回后续每像素采样需要的紧整数 crop。
    Some(SourceCrop {
        // 保存完整源行跨度。
        stride,
        // 保存 crop 左上角。
        x: x0,
        // 保存 crop 上边界。
        y: y0,
        // 计算 crop 宽度。
        width: x1 - x0,
        // 计算 crop 高度。
        height: y1 - y0,
    })
}

// 判断逆映射后的像素中心是否仍位于局部目标矩形。
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

// 图片载荷、源目标几何与 surface 尺寸共同构成软件采样契约。
#[allow(
    // Canvas2D 图片入口需要显式传入所有源目标事实。
    clippy::too_many_arguments,
    // 直接镜像公开契约可避免额外分配临时载荷。
    reason = "image source, destination, and surface bounds mirror the canvas contract"
)]
// 为共享软件执行器补充状态完整的图片采样入口。
impl SoftwareRasterizer {
    // 以 nearest sampling 把设备像素中心逆映射到 offset 后局部目标。
    pub fn blit_image(
        // 只读取当前绘制状态。
        &self,
        // 写入调用方持有的像素目标。
        pixels: &mut [u32],
        // 目标宽度用于安全寻址。
        surface_w: i32,
        // 目标高度用于安全寻址。
        surface_h: i32,
        // 源图片预乘像素。
        src: &[u32],
        // 源图片行跨度。
        src_w: i32,
        // 源图片 crop。
        src_rect: Rect,
        // 局部目标矩形。
        dst_rect: Rect,
    ) {
        // 非正目标尺寸不能形成有效像素缓冲。
        if surface_w <= 0 || surface_h <= 0 {
            // 保持安全 no-op。
            return;
        }
        // 目标载荷必须至少覆盖声明的完整 surface。
        let Some(surface_len) = (surface_w as usize).checked_mul(surface_h as usize) else {
            // 地址空间溢出时不写越界像素。
            return;
        };
        // 短目标缓冲不能安全寻址。
        if pixels.len() < surface_len {
            // 保持目标不变。
            return;
        }
        // 验证并收敛源 crop。
        let Some(source) = source_crop(src, src_w, src_rect) else {
            // 无有效源像素时保持 no-op。
            return;
        };
        // 目标几何必须是有限正矩形。
        if !dst_rect.x.is_finite()
            // 检查目标垂直起点。
            || !dst_rect.y.is_finite()
            // 检查目标宽度。
            || !dst_rect.w.is_finite()
            // 检查目标高度。
            || !dst_rect.h.is_finite()
            // 拒绝空水平范围。
            || dst_rect.w <= 0.0
            // 拒绝空垂直范围。
            || dst_rect.h <= 0.0
        {
            // 非法目标不产生像素。
            return;
        }
        // Canvas2D 顺序要求 offset 在 transform 之前应用。
        let local_target = Rect::new(
            // 水平 offset 移动局部目标。
            dst_rect.x + self.offset_x,
            // 垂直 offset 移动局部目标。
            dst_rect.y + self.offset_y,
            // offset 不改变局部宽度。
            dst_rect.w,
            // offset 不改变局部高度。
            dst_rect.h,
        );
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
                // 任意可逆仿射都在局部目标空间决定源 texel。
                let Some((local_x, local_y)) =
                    // 采用设备像素中心避免整数边界方向偏差。
                    self.apply_inverse(target_x as f32 + 0.5, target_y as f32 + 0.5)
                else {
                    // 退化 transform 不产生不可证明的像素。
                    continue;
                };
                // AABB 中但局部目标外的像素不得读取源图片。
                if !target_contains(local_target, local_x, local_y) {
                    // 跳过局部矩形外部。
                    continue;
                }
                // 把局部水平位置归一化到 0..1。
                let u = (local_x - local_target.x) / local_target.w;
                // 把局部垂直位置归一化到 0..1。
                let v = (local_y - local_target.y) / local_target.h;
                // nearest sampling 选择 crop 内对应列。
                let source_x = source.x
                    // 限制浮点边缘取整，禁止越过 crop 最后一列。
                    + ((u * source.width as f32).floor() as usize).min(source.width - 1);
                // nearest sampling 选择 crop 内对应行。
                let source_y = source.y
                    // 限制浮点边缘取整，禁止越过 crop 最后一行。
                    + ((v * source.height as f32).floor() as usize).min(source.height - 1);
                // checked crop 已保证该索引位于完整源行内。
                let source_pixel = src[source_y * source.stride + source_x];
                // 全局 opacity 只在源贡献中应用一次。
                let premultiplied = self.apply_opa(source_pixel);
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
                    // 已应用 opacity 的预乘图片像素。
                    premultiplied,
                    // 图片矩形内部保持完整 coverage。
                    1.0,
                );
            }
        }
    }
}
