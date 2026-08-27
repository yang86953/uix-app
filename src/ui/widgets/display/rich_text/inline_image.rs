//! RichText 内联图片的资源状态、原子几何与绘制组件。

// 图片能力关闭时仍保留空状态类型，供共享布局签名稳定编译。
use std::collections::HashMap;

// 图片能力开启时引入原子布局需要的几何类型。
#[cfg(feature = "image-codecs")]
use crate::core::Size;
// 图片能力开启时引入共享位图句柄。
#[cfg(feature = "image-codecs")]
use crate::draw::resources::image::BitmapHandle;

// 保存单个图片段的运行时资源事实。
#[cfg(feature = "image-codecs")]
#[derive(Debug, Clone, Default)]
pub(crate) struct InlineImageState {
    // 保存当前状态对应的资源路径。
    src: String,
    // 保存已经登记且仍需代际校验的位图句柄。
    handle: Option<BitmapHandle>,
    // 保存按当前 DPI 换算的固有逻辑尺寸。
    intrinsic_size: Option<Size>,
    // 保存最近一次异步加载失败文本。
    error: Option<String>,
    // 标记后台请求是否已经启动。
    loading: bool,
    // 保存固有尺寸换算使用的设备比例。
    device_scale: f32,
}

// 图片能力关闭时状态表只保留空占位类型以稳定共享布局签名。
#[cfg(not(feature = "image-codecs"))]
#[derive(Debug, Clone, Default)]
pub(crate) struct InlineImageState;

// 按公开段索引保存图片运行时状态。
pub(crate) type InlineImageStates = HashMap<usize, InlineImageState>;

// 返回图片段当前可用于布局的稳定替换对象尺寸。
#[cfg(feature = "image-codecs")]
pub(crate) fn resolved_size(
    // 接收可选运行时固有尺寸状态。
    state: Option<&InlineImageState>,
    // 接收手工宽度覆盖。
    width: Option<f32>,
    // 接收手工高度覆盖。
    height: Option<f32>,
    // 接收加载前的默认行高占位尺寸。
    default_line_height: f32,
) -> Size {
    // 只接受有限正数覆盖。
    let width = width.filter(|value| value.is_finite() && *value > 0.0);
    // 高度遵守相同有效性边界。
    let height = height.filter(|value| value.is_finite() && *value > 0.0);
    // 读取已经按 DPI 换算的固有尺寸。
    let intrinsic = state.and_then(|state| state.intrinsic_size);
    // 加载前使用一个默认行高的正方形占位，避免零尺寸抖动。
    let placeholder = default_line_height.max(1.0);
    // 根据显式覆盖和固有比例解析最终几何。
    match (width, height, intrinsic) {
        // 双轴覆盖直接决定替换对象尺寸。
        (Some(width), Some(height), _) => Size::new(width, height),
        // 仅宽度覆盖时按固有比例推导高度。
        (Some(width), None, Some(intrinsic)) if intrinsic.w > 0.0 => {
            // 保持固有宽高比。
            Size::new(width, width * intrinsic.h / intrinsic.w)
        }
        // 仅高度覆盖时按固有比例推导宽度。
        (None, Some(height), Some(intrinsic)) if intrinsic.h > 0.0 => {
            // 保持固有宽高比。
            Size::new(height * intrinsic.w / intrinsic.h, height)
        }
        // 只有宽度且尚无固有比例时用占位高度。
        (Some(width), None, _) => Size::new(width, placeholder),
        // 只有高度且尚无固有比例时用占位宽度。
        (None, Some(height), _) => Size::new(placeholder, height),
        // 没有覆盖时优先使用固有尺寸。
        (None, None, Some(intrinsic)) => intrinsic,
        // 加载前保持默认行高正方形。
        (None, None, None) => Size::new(placeholder, placeholder),
    }
}

// 把一个图片段作为不可拆分的替换对象加入当前视觉行。
#[allow(clippy::too_many_arguments)]
#[cfg(feature = "image-codecs")]
pub(crate) fn push_layout_glyph(
    // 接收段索引。
    segment_idx: usize,
    // 接收 alt 在逻辑源中的起点。
    source_offset: usize,
    // 接收 alt 的 Unicode 标量长度。
    source_char_len: usize,
    // 接收图片宽度覆盖。
    width: Option<f32>,
    // 接收图片高度覆盖。
    height: Option<f32>,
    // 接收当前运行时资源状态。
    state: Option<&InlineImageState>,
    // 接收最大行宽。
    max_width: f32,
    // 接收默认行高。
    default_line_height: f32,
    // 接收默认前景色供共享布局字段保持完整。
    default_color: crate::draw::Color,
    // 接收 UIX 声明的布局比例。
    metrics: super::presentation::RichTextMetricsVisual,
    // 接收已完成行列表。
    lines: &mut Vec<super::LayoutLine>,
    // 接收当前行视觉原子。
    glyphs: &mut Vec<super::LayoutGlyph>,
    // 接收当前行水平游标。
    current_x: &mut f32,
    // 接收全局最大行宽。
    max_line_width: &mut f32,
) {
    // 解析当前占位或固有图片几何。
    let size = resolved_size(state, width, height, default_line_height);
    // 负数与 NaN 规范为零，正无穷保持无界布局语义。
    let available_width = max_width.max(0.0);
    // 图片是原子对象；当前行已有内容且容纳不下时先换行。
    if *current_x > 0.0 && *current_x + size.w > available_width && !glyphs.is_empty() {
        // 结算换行前的实际宽度。
        *max_line_width = (*max_line_width).max(*current_x);
        // 复用共享行结算保持前序文本行高。
        super::layout_metrics::flush_line(lines, glyphs, default_line_height, metrics);
        // 图片从新行起点开始。
        *current_x = 0.0;
    }
    // 登记一个不参与文本 run 绘制的图片原子。
    glyphs.push(super::LayoutGlyph {
        // 标记绘制层使用图片路径。
        kind: super::LayoutGlyphKind::InlineImage,
        // 保存资源状态和公开段的共同索引。
        segment_idx,
        // 图片逻辑跨度从 alt 首字符开始。
        global_char_idx: source_offset,
        // 命中两侧边界覆盖完整 alt。
        source_char_len,
        // 图片按周围段落的基础方向参与视觉顺序。
        bidi_level: 0,
        // 使用对象替代符作为调试与行观测字符。
        ch: '\u{fffc}',
        // 保存当前行内水平位置。
        x: *current_x,
        // 图片宽度直接作为原子 advance。
        width: size.w,
        // 图片高度复用 font_size 槽位供共享行高计算。
        font_size: size.h,
        // 图片不使用文本前景色，但保持字段初始化完整。
        color: default_color,
        // 图片自身没有文本背景样式。
        bg_color: None,
        // 图片不进入链接提交生命周期。
        is_link: false,
    });
    // 推进到图片右缘。
    *current_x += size.w;
    // 图片可能自然宽于约束，仍需报告真实内容宽度。
    *max_line_width = (*max_line_width).max(*current_x);
}

// 图片编解码能力开启时更新 RichText 所有的异步资源状态。
#[cfg(feature = "image-codecs")]
pub(crate) fn prepare(
    // 接收公开段列表。
    segments: &[super::RichTextSegment],
    // 接收组件私有图片状态表。
    states: &mut InlineImageStates,
    // 接收当前绘制上下文及图片服务。
    ctx: &mut crate::ui::widget_runtime::paint_context::PaintContext,
    // 接收组件树失效队列。
    tree: &crate::ui::widget_runtime::widget::WidgetTree,
    // 接收当前组件绘制范围。
    frame: crate::core::Rect,
) -> bool {
    // 保存本轮是否有固有尺寸或失败状态变化。
    let mut layout_changed = false;
    // 保存是否仍有后台任务需要后继帧轮询。
    let mut pending = false;
    // 读取当前设备比例并规范为有限正数。
    let device_scale = ctx.device_pixel_ratio().max(f32::EPSILON);
    // 遍历全部公开段，只处理图片能力下的图片段。
    for (segment_idx, segment) in segments.iter().enumerate() {
        // 提取当前图片资源路径。
        let super::RichTextSegment::Image { src, .. } = segment else {
            // 非图片段不拥有资源状态。
            continue;
        };
        // 获取或创建与段索引对应的私有状态。
        let state = states.entry(segment_idx).or_default();
        // 同一位置资源路径变化时丢弃旧句柄和错误。
        if state.src != *src {
            // 记录新的资源身份。
            state.src = src.clone();
            // 清除旧句柄。
            state.handle = None;
            // 清除旧固有尺寸。
            state.intrinsic_size = None;
            // 清除旧错误。
            state.error = None;
            // 新路径尚未启动加载。
            state.loading = false;
            // 强制重新计算布局占位。
            layout_changed = true;
        }
        // 已完成状态只需在 DPI 变化时重新换算固有逻辑尺寸。
        if let Some(handle) = state
            .handle
            .filter(|handle| ctx.image_service().is_valid(*handle))
        {
            // 从资源槽位读取固有像素尺寸。
            let intrinsic = ctx.image_service().with_slot(handle, |slot| {
                // 按设备比例换算为逻辑像素。
                Size::new(
                    slot.width() as f32 / device_scale,
                    slot.height() as f32 / device_scale,
                )
            });
            // 尺寸或 DPI 变化必须触发后继布局。
            if state.intrinsic_size != intrinsic || state.device_scale != device_scale {
                // 保存新的逻辑固有尺寸。
                state.intrinsic_size = intrinsic;
                // 保存当前设备比例。
                state.device_scale = device_scale;
                // 通知调用方重建布局。
                layout_changed = true;
            }
            // 有效句柄无需继续轮询。
            continue;
        }
        // 已失败图片保持稳定占位，直到资源路径改变。
        if state.error.is_some() {
            // 不重复启动失败请求。
            continue;
        }
        // 非阻塞轮询或启动当前路径后台任务。
        match ctx.image_service().poll_load_from_path(src) {
            // 位图就绪后登记句柄，下一轮立即读取固有尺寸。
            Ok(Some(handle)) => {
                // 保存代际句柄。
                state.handle = Some(handle);
                // 后台任务已经结束。
                state.loading = false;
                // 读取固有像素并换算当前逻辑尺寸。
                state.intrinsic_size = ctx.image_service().with_slot(handle, |slot| {
                    // 使用当前设备比例形成布局尺寸。
                    Size::new(
                        slot.width() as f32 / device_scale,
                        slot.height() as f32 / device_scale,
                    )
                });
                // 保存换算比例。
                state.device_scale = device_scale;
                // 固有尺寸就绪需要重排。
                layout_changed = true;
            }
            // 后台任务尚未完成，保留首帧占位。
            Ok(None) => {
                // 标记请求已启动。
                state.loading = true;
                // 请求后继绘制继续轮询。
                pending = true;
            }
            // typed error 在组件状态中保留可诊断文本并停止轮询。
            Err(error) => {
                // 保存完整框架错误展示文本。
                state.error = Some(error.to_string());
                // 失败任务不再处于加载中。
                state.loading = false;
                // 错误占位需要重绘。
                layout_changed = true;
            }
        }
    }
    // 清理已经从公开段列表移除的旧资源状态。
    states.retain(|segment_idx, _| {
        // 只保留仍指向图片段的索引。
        matches!(
            segments.get(*segment_idx),
            Some(super::RichTextSegment::Image { .. })
        )
    });
    // 后台任务或布局变化都需要通过当前组件失效队列驱动后继帧。
    if (pending || layout_changed)
        // 绘制作用域必须能解析当前 RichText 组件身份。
        && let Some(id) = crate::ui::widget_runtime::paint_scope::current_paint_widget()
        // 失效队列锁失败时保持当前帧结果，不伪造提交成功。
        && let Ok(mut queue) = tree.invalidation_handle().lock()
    {
        // 布局变化需要同时重测与重绘；加载中只轮询绘制。
        if layout_changed {
            // 登记确切组件布局失效。
            queue.push(crate::draw::renderer::Invalidation::Layout(id));
        }
        // 登记当前组件范围的绘制失效。
        queue.push(crate::draw::renderer::Invalidation::Paint {
            // 失效当前 RichText。
            id,
            // 只重绘组件 frame。
            rect: Some(frame),
        });
    }
    // 返回调用方是否必须丢弃布局缓存。
    layout_changed
}

// 图片编解码能力开启时绘制布局中的全部图片原子。
#[cfg(all(test, feature = "image-codecs"))]
pub(crate) fn draw(
    ctx: &mut crate::ui::widget_runtime::paint_context::PaintContext,
    segments: &[super::RichTextSegment],
    states: &InlineImageStates,
    lines: &[super::LayoutLine],
    frame: crate::core::Rect,
) {
    let visual = super::presentation::RICH_TEXT_VISUAL;
    let resolved = visual.resolve(crate::draw::Color::default(), true, ctx.tokens());
    draw_visual(ctx, segments, states, lines, frame, visual.image, resolved);
}

// 生产绘制入口消费 UIX 根注入的图片视觉与主题颜色。
#[cfg(feature = "image-codecs")]
pub(crate) fn draw_visual(
    // 接收 UI 绘制上下文。
    ctx: &mut crate::ui::widget_runtime::paint_context::PaintContext,
    // 接收公开段列表。
    segments: &[super::RichTextSegment],
    // 接收组件私有资源状态。
    states: &InlineImageStates,
    // 接收已平移到组件 frame 的布局行。
    lines: &[super::LayoutLine],
    // 接收组件内容 frame。
    frame: crate::core::Rect,
    // 接收 UIX 声明的图片占位视觉。
    visual: super::presentation::RichTextImageVisual,
    // 接收当前主题一次解析完成的颜色。
    resolved: super::presentation::ResolvedRichTextVisual,
) {
    // 遍历全部视觉行。
    for line in lines {
        // 遍历行内文本与图片原子。
        for glyph in &line.glyphs {
            // 只消费图片布局原子。
            if glyph.kind != super::LayoutGlyphKind::InlineImage {
                // 普通文本由 RichText 共享 run 绘制。
                continue;
            }
            // 读取公开图片样式字段。
            let Some(super::RichTextSegment::Image {
                alt, fit, radius, ..
            }) = segments.get(glyph.segment_idx)
            else {
                // 布局缓存与公开段不一致时跳过失效原子。
                continue;
            };
            // 图片在实际行盒内垂直居中。
            let bounds = crate::core::Rect::new(
                // 水平位置相对组件 frame。
                frame.x + glyph.x,
                // 垂直位置相对已经平移的行顶部。
                frame.y + line.y + (line.height - glyph.font_size) * 0.5,
                // 使用布局确定的原子宽度。
                glyph.width,
                // 使用布局确定的原子高度。
                glyph.font_size,
            );
            // 空几何不生成绘制命令。
            if bounds.w <= 0.0 || bounds.h <= 0.0 {
                // 保持布局事实但跳过不可见内容。
                continue;
            }
            // 规范可选圆角半径。
            let radius = radius
                // 只接受有限非负覆盖。
                .filter(|value| value.is_finite() && *value >= 0.0)
                // 默认不使用圆角。
                .unwrap_or(0.0)
                // 圆角不能超过短边一半。
                .min(bounds.w.min(bounds.h) * 0.5);
            // 读取仍有效的资源句柄。
            let handle = states
                // 按公开段索引查找状态。
                .get(&glyph.segment_idx)
                // 提取可选句柄。
                .and_then(|state| state.handle)
                // 每帧绘制前执行代际校验。
                .filter(|handle| ctx.image_service().is_valid(*handle));
            // 先绘制稳定主题占位背景。
            ctx.fill_rect(
                // 使用完整图片原子范围。
                bounds,
                // 使用三级填充色适配主题。
                resolved.fill_tertiary,
                // 应用公开圆角。
                Some(crate::draw::Radius::uniform(radius)),
            );
            // 已加载图片进入位图绘制路径。
            if let Some(handle) = handle {
                // 计算圆角派生图需要的设备像素尺寸。
                let device_scale = ctx.device_pixel_ratio().max(f32::EPSILON);
                // 宽度限制在资源服务允许范围内。
                let target_width = (bounds.w * device_scale)
                    .ceil()
                    .clamp(1.0, visual.max_device_extent) as u32;
                // 高度遵守相同限制。
                let target_height = (bounds.h * device_scale)
                    .ceil()
                    .clamp(1.0, visual.max_device_extent)
                    as u32;
                // 有圆角时复用 ImageService 派生图缓存。
                let drawable = (radius > 0.0)
                    // 生成或复用目标尺寸圆角派生图。
                    .then(|| {
                        ctx.image_service().rounded_rect_sized(
                            // 使用原始位图句柄。
                            handle,
                            // 使用目标设备像素宽度。
                            target_width,
                            // 使用目标设备像素高度。
                            target_height,
                            // 把逻辑圆角换算为设备像素。
                            radius * device_scale,
                            // 复用公开 fit 策略。
                            *fit,
                        )
                    })
                    // 展平可选派生结果。
                    .flatten();
                // 圆角派生图已经精确匹配目标矩形，直接填满绘制。
                if let Some(drawable) = drawable {
                    // 绘制圆角派生位图。
                    ctx.draw_image_fill(drawable, bounds);
                } else if *fit {
                    // 默认 fit 保持固有比例居中。
                    ctx.draw_image(handle, bounds);
                } else {
                    // fill 模式拉伸填满替换对象。
                    ctx.draw_image_fill(handle, bounds);
                }
                // 已加载图片无需绘制 fallback。
                continue;
            }
            // 未加载或失败时绘制主题边框。
            ctx.stroke_rect(
                // 使用完整占位范围。
                bounds,
                // 使用次级边框色。
                resolved.border_secondary,
                // 使用一逻辑像素边框。
                visual.border_stroke,
                // 保持与图片相同圆角。
                Some(crate::draw::Radius::uniform(radius)),
            );
            // 有空间且有 alt 时把替代文本绘制为失败/加载占位。
            if !alt.is_empty()
                && bounds.w >= visual.min_alt_width
                && bounds.h >= visual.min_alt_height
            {
                // 裁剪替代文本避免泄漏到相邻行内内容。
                ctx.push_clip(bounds);
                // 以较小字号在占位中居中展示 alt。
                ctx.text_center(
                    // 使用公开替代文本。
                    alt,
                    // 使用图片原子范围。
                    bounds,
                    // 使用次级正文色。
                    resolved.text_secondary,
                    // 字号受图片高度限制。
                    visual.alt_font_size.min(bounds.h * visual.alt_height_ratio),
                );
                // 恢复外层 RichText 裁剪。
                ctx.pop_clip();
            }
        }
    }
}

// 暴露测试所需的资源事实，不进入公开 API。
#[cfg(all(test, feature = "image-codecs"))]
impl InlineImageState {
    // 构造带固有尺寸的测试状态。
    pub(crate) fn with_intrinsic_size(size: Size) -> Self {
        // 只设置布局关心的事实。
        Self {
            // 测试状态不绑定实际路径。
            src: String::new(),
            // 测试不需要位图句柄。
            handle: None,
            // 保存指定固有尺寸。
            intrinsic_size: Some(size),
            // 测试状态没有错误。
            error: None,
            // 测试状态不在加载中。
            loading: false,
            // 使用标准一倍设备比例。
            device_scale: 1.0,
        }
    }
}
