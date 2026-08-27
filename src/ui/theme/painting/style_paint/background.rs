//! Style 背景图层到 draw System 的私有绘制适配。

// 引入背景绘制使用的矩形几何。
use crate::core::{Rect, Size};
// 引入背景渐变使用的最终颜色与方向。
use crate::draw::{Color, GradientDirection};
// 引入 draw System 的公开绘制上下文。
use crate::draw::painting::PaintContext;
// 引入 UI System 自有的背景值契约。
use crate::ui::theme::style::{BackgroundImage, BackgroundPosition, BackgroundRepeat};
// 引入主题令牌解析边界。
use crate::ui::theme::traits::ThemeTokens;

/// 已在 UI 边界解析颜色后的背景图层。
pub(super) enum ResolvedBackground<'a> {
    /// 不绘制背景图层。
    None,
    /// 绘制本地图片路径。
    Url(&'a str),
    /// 绘制上下方向的双色线性渐变。
    Linear(Color, Color),
    /// 绘制从中心向外的双色径向渐变。
    Radial(Color, Color),
}

/// 将 Style 背景图来源解析为 draw System 可消费的值。
pub(super) fn resolve_background<'a>(
    // 接收仍归 Style 所有的背景图来源。
    image: Option<&'a BackgroundImage>,
    // 接收当前主题令牌。
    tokens: &dyn ThemeTokens,
    // 返回借用路径且已解析颜色的闭合绘制值。
) -> ResolvedBackground<'a> {
    // 按公开背景来源建立私有绘制适配。
    match image {
        // 未声明与显式 none 都不产生图层。
        None | Some(BackgroundImage::None) => ResolvedBackground::None,
        // 本地路径只借用，不复制每帧字符串。
        Some(BackgroundImage::Url(path)) => ResolvedBackground::Url(path),
        // 线性渐变在 UI 边界解析主题颜色。
        Some(BackgroundImage::LinearGradient { start, end }) => {
            // 将两个端点转换为 draw 颜色。
            ResolvedBackground::Linear(start.resolve(tokens), end.resolve(tokens))
        }
        // 径向渐变在 UI 边界解析主题颜色。
        Some(BackgroundImage::RadialGradient { inner, outer }) => {
            // 将中心与外缘转换为 draw 颜色。
            ResolvedBackground::Radial(inner.resolve(tokens), outer.resolve(tokens))
        }
    }
}

/// 在背景色之后、边框之前绘制单层背景图。
pub(super) fn paint_background(
    // 接收 draw System 的公开绘制上下文。
    ctx: &mut PaintContext,
    // 接收当前组件的边框盒矩形。
    rect: Rect,
    // 接收已经解析的背景图来源。
    background: ResolvedBackground<'_>,
    // 接收确定的二维背景定位。
    position: BackgroundPosition,
    // 接收确定的背景重复方式。
    repeat: BackgroundRepeat,
    // 完成当前单层背景绘制。
) {
    // 按闭合背景来源选择绘制机制。
    match background {
        // 无背景图时保持既有背景色。
        ResolvedBackground::None => {}
        // 本地图片由 ImageService 统一加载与缓存。
        ResolvedBackground::Url(path) => {
            // 图片编解码能力开启时才存在路径加载入口。
            #[cfg(feature = "image-codecs")]
            // 只在图片成功加载并能读取固有尺寸时绘制。
            if let Some(handle) = ctx.image_service().ensure_loaded(path) {
                // 在不可变资源借用结束前复制固有尺寸。
                let image_size = ctx.image_service().with_slot(handle, |slot| {
                    // 将位图像素尺寸转换为布局浮点尺寸。
                    Size::new(slot.width() as f32, slot.height() as f32)
                });
                // 无效或竞态失效句柄不会产生半成品图层。
                if let Some(image_size) = image_size {
                    // 背景图必须裁剪在组件矩形内。
                    ctx.push_clip(rect);
                    // 逐个绘制由纯几何算法生成的可见平铺矩形。
                    for tile in background_tiles(rect, image_size, position, repeat) {
                        // 保持图片固有尺寸并让矩形裁剪处理边缘部分。
                        ctx.draw_image_fill(handle, tile);
                    }
                    // 恢复调用方原有裁剪栈。
                    ctx.pop_clip();
                }
            }
            // 无图片编解码能力时保留路径值但不尝试加载。
            #[cfg(not(feature = "image-codecs"))]
            // 标记路径已被有意消费，避免能力裁剪构建告警。
            let _ = path;
        }
        // 线性渐变始终填满背景盒并忽略定位与重复。
        ResolvedBackground::Linear(start, end) => {
            // 使用当前 draw System 的上下方向渐变原语。
            ctx.fill_linear_gradient(rect, start, end, GradientDirection::Vertical);
        }
        // 径向渐变始终从背景盒中心覆盖到最远角。
        ResolvedBackground::Radial(inner, outer) => {
            // 计算背景盒中心横坐标。
            let center_x = rect.x + rect.w * 0.5;
            // 计算背景盒中心纵坐标。
            let center_y = rect.y + rect.h * 0.5;
            // 使用半宽和半高的斜边覆盖四个角。
            let radius = (rect.w * 0.5).hypot(rect.h * 0.5);
            // 径向原语是完整圆形，必须先限制到当前组件的背景盒。
            ctx.push_clip(rect);
            // 使用当前 draw System 的径向渐变原语。
            ctx.fill_radial_gradient(center_x, center_y, 0.0, radius, inner, outer);
            // 恢复调用方进入背景绘制前的裁剪栈。
            ctx.pop_clip();
        }
    }
}

/// 生成与背景盒相交的固有尺寸图片平铺矩形。
pub(super) fn background_tiles(
    // 接收背景盒矩形。
    target: Rect,
    // 接收图片固有尺寸。
    image_size: Size,
    // 接收二维定位值。
    position: BackgroundPosition,
    // 接收轴重复方式。
    repeat: BackgroundRepeat,
    // 返回确定顺序的可见图片目标矩形。
) -> Vec<Rect> {
    // 无效背景盒或图片尺寸不能形成可绘制平铺。
    if !valid_extent(target.w)
        // 同时验证背景盒高度。
        || !valid_extent(target.h)
        // 同时验证图片宽度。
        || !valid_extent(image_size.w)
        // 同时验证图片高度。
        || !valid_extent(image_size.h)
        // 背景盒坐标也必须是有限值。
        || !target.x.is_finite()
        // 垂直坐标同样必须是有限值。
        || !target.y.is_finite()
    {
        // 无效输入安全返回空集合。
        return Vec::new();
    }
    // 百分比按背景盒与图片之间的剩余水平空间解析。
    let anchor_x = target.x + position.x.resolve(target.w - image_size.w);
    // 百分比按背景盒与图片之间的剩余垂直空间解析。
    let anchor_y = target.y + position.y.resolve(target.h - image_size.h);
    // 生成横轴所有候选起点。
    let x_positions = tile_axis(
        // 使用背景盒水平起点。
        target.x,
        // 使用背景盒水平终点。
        target.x + target.w,
        // 使用解析后的水平锚点。
        anchor_x,
        // 使用图片固有宽度作为步长。
        image_size.w,
        // 依据重复枚举决定是否平铺横轴。
        repeat.repeats_x(),
    );
    // 生成纵轴所有候选起点。
    let y_positions = tile_axis(
        // 使用背景盒垂直起点。
        target.y,
        // 使用背景盒垂直终点。
        target.y + target.h,
        // 使用解析后的垂直锚点。
        anchor_y,
        // 使用图片固有高度作为步长。
        image_size.h,
        // 依据重复枚举决定是否平铺纵轴。
        repeat.repeats_y(),
    );
    // 预分配完整笛卡尔积的通常容量。
    let mut tiles = Vec::with_capacity(x_positions.len().saturating_mul(y_positions.len()));
    // 按纵轴优先建立稳定的行顺序。
    for y in y_positions {
        // 在每一行按横轴从左到右生成。
        for x in x_positions.iter().copied() {
            // 建立保持图片固有尺寸的候选矩形。
            let tile = Rect::new(x, y, image_size.w, image_size.h);
            // 只保留与背景盒具有正面积交集的图片。
            if tile.intersect(&target).is_some() {
                // 保存完整目标矩形，让裁剪栈处理边缘像素。
                tiles.push(tile);
            }
        }
    }
    // 返回确定顺序的可见平铺集合。
    tiles
}

// 判断尺寸是否为正有限数值。
fn valid_extent(value: f32) -> bool {
    // 正有限尺寸才能参与平铺步进。
    value.is_finite() && value > 0.0
}

// 生成单轴候选平铺起点。
fn tile_axis(start: f32, end: f32, anchor: f32, tile: f32, repeated: bool) -> Vec<f32> {
    // 非重复轴只保留定位产生的单个锚点。
    if !repeated {
        // 返回单元素轴集合。
        return vec![anchor];
    }
    // 将锚点相对起点的相位规范化到一个图片周期内。
    let phase = (anchor - start).rem_euclid(tile);
    // 相位为零时从背景盒起点开始，否则退回一个周期以覆盖左边缘。
    let mut current = if phase <= f32::EPSILON {
        // 精确周期边界避免额外生成一个刚好不相交的图片。
        start
    } else {
        // 非零相位需要保留跨越背景盒起点的图片。
        start + phase - tile
    };
    // 按背景盒长度估算所需容量并为边缘图片留余量。
    let capacity = ((end - start) / tile).ceil().max(0.0) as usize + 2;
    // 建立单轴平铺起点集合。
    let mut positions = Vec::with_capacity(capacity);
    // 只生成起点仍位于背景盒终点之前的图片。
    while current < end {
        // 保存当前图片起点。
        positions.push(current);
        // 按固有图片尺寸移动到下一个周期。
        current += tile;
    }
    // 返回从小到大排列的单轴起点。
    positions
}
