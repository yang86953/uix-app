//! FrameEncoder 字形轮廓到通用 RHI MSDF 的 lowering。

// 引入通用 RHI 的 MSDF 载荷。
use crate::draw::backend::rhi_renderer::RhiMsdfQuad;
// 引入字形轮廓与整数裁剪类型。
use crate::draw::painting::{FrameGlyphOutline, FrameRect};
// 引入共享引用计数所有权。
use std::sync::Arc;

// 引入父模块统一的缩放、裁剪与 RHI 队列类型。
use super::{RhiOp, RhiViewport, axis_aligned_corners, color_rgba, scaled_scissor};

// 把一个有序字形轮廓批次降低为 GPU MSDF quad。
pub(super) fn append_glyph_outlines(
    // 接收保持 painter order 的 RHI 队列。
    operations: &mut Vec<RhiOp>,
    // 接收有序字形轮廓。
    glyphs: &[FrameGlyphOutline],
    // 接收统一整数裁剪。
    clip: FrameRect,
    // 接收逻辑目标边界。
    bounds: FrameRect,
    // 接收物理视口。
    viewport: RhiViewport,
    // 接收水平缩放。
    scale_x: f32,
    // 接收垂直缩放。
    scale_y: f32,
) -> bool {
    // 完全不可见的批次安全视为 no-op。
    let Some(scissor) = scaled_scissor(clip, bounds, viewport, scale_x, scale_y) else {
        // 报告命令已经安全处理。
        return true;
    };
    // 保持字形原始 painter order。
    for glyph in glyphs {
        // 再次验证轮廓边 ABI，防止异常命令进入 GPU。
        if !crate::draw::resources::font::glyph_outline::is_outline_edges(glyph.edges().as_ref()) {
            // 无法保真降低时交回上层处理。
            return false;
        }
        // 把逻辑位置与目标尺寸缩放到物理空间。
        let (x, y, width, height) = (
            // 缩放水平位置。
            glyph.x() as f32 * scale_x,
            // 缩放垂直位置。
            glyph.y() as f32 * scale_y,
            // 缩放目标宽度。
            glyph.width() as f32 * scale_x,
            // 缩放目标高度。
            glyph.height() as f32 * scale_y,
        );
        // 拒绝溢出或反向的物理几何。
        if !x.is_finite()
            || !y.is_finite()
            || !width.is_finite()
            || !height.is_finite()
            || width <= 0.0
            || height <= 0.0
        {
            // 无法保真降低时交回上层处理。
            return false;
        }
        // 追加由 GPU 端生成 coverage 的 MSDF quad。
        operations.push(RhiOp::Msdf(RhiMsdfQuad {
            // 保存物理左边界。
            x,
            // 保存物理上边界。
            y,
            // 保存物理宽度。
            w: width,
            // 保存物理高度。
            h: height,
            // 保存轴对齐设备四角。
            corners: axis_aligned_corners(x, y, width, height),
            // 使用字体模块登记的稳定 MSDF 距离范围。
            range: crate::draw::resources::font::glyph_outline::MSDF_RANGE,
            // 保留共享轮廓边列表。
            edges: Arc::clone(glyph.edges()),
            // 保存逻辑字形纹理宽度。
            pixel_w: glyph.width(),
            // 保存逻辑字形纹理高度。
            pixel_h: glyph.height(),
            // 保存规范化直通颜色。
            rgba: color_rgba(glyph.color()),
            // 保存物理整数裁剪。
            scissor: Some(scissor),
        }));
    }
    // 报告整个批次已经安全降低。
    true
}
