//! 自包含 CPU 帧图像通过既有可录制绘制链路呈现。

use super::*;
use crate::draw::painting::FrameImage;
use crate::draw::resources::image::fit_dst_rect;

impl PaintContext<'_> {
    /// 按原始宽高比居中绘制自包含帧，目标矩形使用 logical 坐标。
    ///
    /// 不清除留白区域，不提交或等待 present。绘制命令可保留共享像素，
    /// 调用返回后允许释放调用方的帧引用。非有限或非正目标范围不绘制。
    pub fn draw_frame_image(&mut self, image: &FrameImage, bounds: Rect) {
        self.paint_frame_image(image, bounds, true);
    }

    /// 将自包含帧拉伸到整个 logical 目标矩形，不保持宽高比。
    ///
    /// 所有权、状态与无效目标语义同 [`Self::draw_frame_image`]。
    pub fn draw_frame_image_fill(&mut self, image: &FrameImage, bounds: Rect) {
        self.paint_frame_image(image, bounds, false);
    }

    fn paint_frame_image(&mut self, image: &FrameImage, bounds: Rect, fit: bool) {
        if frame_destination(image, bounds, fit).is_none() {
            return;
        }
        self.record_op_lazy(|| PaintOp::DrawFrameImage {
            image: image.clone(),
            bounds,
            fit,
        });
        blit_frame_image(self.spatial.canvas_2d(), image, bounds, fit);
    }
}

fn valid_destination(rect: Rect) -> bool {
    rect.x.is_finite()
        && rect.y.is_finite()
        && rect.w.is_finite()
        && rect.h.is_finite()
        && rect.w > 0.0
        && rect.h > 0.0
        && (rect.x + rect.w).is_finite()
        && (rect.y + rect.h).is_finite()
}

fn frame_destination(image: &FrameImage, bounds: Rect, fit: bool) -> Option<Rect> {
    if !valid_destination(bounds) {
        return None;
    }
    let destination = if fit {
        fit_dst_rect(image.width(), image.height(), bounds)
    } else {
        bounds
    };
    valid_destination(destination).then_some(destination)
}

pub(crate) fn blit_frame_image(
    canvas: &mut dyn Canvas2D,
    image: &FrameImage,
    bounds: Rect,
    fit: bool,
) {
    let Some(destination) = frame_destination(image, bounds, fit) else {
        return;
    };
    canvas.blit_image_shared(
        Arc::clone(&image.pixels),
        image.width(),
        Rect::new(0.0, 0.0, image.width() as f32, image.height() as f32),
        destination,
    );
}
