//! GPU-native 几何/色彩辅助 — gpu 子模块。

use std::sync::Arc;

use crate::core::Rect;
use crate::draw::geometry::color::Color;
use crate::draw::geometry::types::{BlendMode, Transform};

use super::canvas::NativeGpuCanvas2D;
use super::geometry::{
    IntegerFrame, frame_within, glyph_device_corners, quad_aabb, rect_to_integer_frame,
};
use super::pending::DirectImageBlit;
// 引入所属 graphics backend Module 的字形四角辅助与图片原语。
use super::{GpuGlyphBlit, GpuImageBlit};

impl NativeGpuCanvas2D {
    pub(super) fn try_axis_aligned_device_rect(&self, rect: Rect) -> Option<(Rect, (f32, f32))> {
        let [a, b, _, c, d, _] = self.transform.m;
        if b != 0.0 || c != 0.0 || !a.is_finite() || !d.is_finite() || a == 0.0 || d == 0.0 {
            return None;
        }
        let offset = Rect::new(
            rect.x + self.offset_x,
            rect.y + self.offset_y,
            rect.w,
            rect.h,
        );
        let device = if self.transform.m == Transform::identity().m {
            offset
        } else {
            self.transform.transform_rect(offset)
        };
        if !device.x.is_finite()
            || !device.y.is_finite()
            || !device.w.is_finite()
            || !device.h.is_finite()
        {
            return None;
        }
        Some((device, (a, d)))
    }

    pub(super) fn soft_or_reject_transform(&mut self, operation: &str) {
        if self.gpu_only {
            self.reject_unsupported(operation);
        }
    }

    pub(super) fn rgba(&self, color: Color) -> [f32; 4] {
        [
            color.r as f32 / 255.0,
            color.g as f32 / 255.0,
            color.b as f32 / 255.0,
            (color.a as f32 / 255.0) * self.opacity,
        ]
    }

    pub(super) fn solid_rgba(&self, color: Color) -> [f32; 4] {
        let color =
            crate::draw::raster::rasterizer::color_with_premultiplied_opacity(color, self.opacity);
        [
            color.r as f32 / 255.0,
            color.g as f32 / 255.0,
            color.b as f32 / 255.0,
            color.a as f32 / 255.0,
        ]
    }

    pub(super) fn scissor_aabb(&self) -> (i32, i32, i32, i32) {
        let c = self.clip_rect;
        if c.w <= 0.0 || c.h <= 0.0 {
            (0, 0, 0, 0)
        } else {
            let x0 = c.x.floor() as i32;
            let y0 = c.y.floor() as i32;
            // 宽高必须由绝对远端计算；亚像素起点下直接 ceil(w/h)
            // 会少包一行/列，使局部清屏后的远边界无法重绘。
            let x1 = (c.x + c.w).ceil() as i32;
            let y1 = (c.y + c.h).ceil() as i32;
            (x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0))
        }
    }

    /// 严格 GPU 路径：整数像素源 crop；目标可 1:1 或缩放；位置允许亚像素。
    /// 轴对齐仿射（平移/缩放，无旋转/剪切）经 [`Self::try_axis_aligned_device_rect`]
    /// 映射到设备空间；裁剪交给 scissor。SrcOver 与 Additive 均可入队。
    ///
    /// `Culled` 表示完全不可见（屏外 / 空 clip），gpu-only 必须 no-op，不得当成未实现。
    pub(super) fn try_queue_direct_image_blit(
        &self,
        pixels: &[u32],
        source_width: i32,
        source_rect: Rect,
        destination_rect: Rect,
    ) -> DirectImageBlit {
        self.try_queue_direct_image_blit_inner(
            pixels,
            None,
            source_width,
            source_rect,
            destination_rect,
        )
    }

    /// 完整图片可把共享像素直接交给待提交操作；裁剪继续生成紧密载荷。
    pub(super) fn try_queue_direct_image_blit_shared(
        &self,
        pixels: Arc<Vec<u32>>,
        source_width: i32,
        source_rect: Rect,
        destination_rect: Rect,
    ) -> DirectImageBlit {
        let retained = Arc::clone(&pixels);
        self.try_queue_direct_image_blit_inner(
            pixels.as_slice(),
            Some(retained),
            source_width,
            source_rect,
            destination_rect,
        )
    }

    fn try_queue_direct_image_blit_inner(
        &self,
        pixels: &[u32],
        shared_pixels: Option<Arc<Vec<u32>>>,
        source_width: i32,
        source_rect: Rect,
        destination_rect: Rect,
    ) -> DirectImageBlit {
        if !self.opacity.is_finite() || self.opacity <= 0.0 {
            return DirectImageBlit::Culled;
        }
        let native_blend = matches!(
            self.blend_mode,
            BlendMode::Alpha | BlendMode::SrcOver | BlendMode::Additive
        );
        if !native_blend {
            return DirectImageBlit::Unsupported;
        }
        let Ok(source_stride) = usize::try_from(source_width) else {
            return DirectImageBlit::Unsupported;
        };
        if source_stride == 0 {
            return DirectImageBlit::Culled;
        }
        let Ok(source_height) = i32::try_from(pixels.len() / source_stride) else {
            return DirectImageBlit::Unsupported;
        };
        let Some(source) = rect_to_integer_frame(source_rect) else {
            return DirectImageBlit::Unsupported;
        };
        if source.width <= 0 || source.height <= 0 {
            return DirectImageBlit::Culled;
        }
        if !frame_within(source, source_width, source_height) {
            return DirectImageBlit::Unsupported;
        }
        if !destination_rect.w.is_finite()
            || !destination_rect.h.is_finite()
            || destination_rect.w <= 0.0
            || destination_rect.h <= 0.0
        {
            return DirectImageBlit::Culled;
        }
        // 轴对齐变换直接复用矩形几何；旋转/剪切改用四角 payload。
        let (device_corners, device_dst) =
            if let Some((rect, _)) = self.try_axis_aligned_device_rect(destination_rect) {
                (
                    GpuGlyphBlit::axis_aligned_corners(rect.x, rect.y, rect.w, rect.h),
                    rect,
                )
            } else {
                // 仿射矩阵不可逆时无法保证采样 UV 的单调映射。
                let [a, b, _, c, d, _] = self.transform.m;
                let determinant = a * d - b * c;
                if !a.is_finite()
                    || !b.is_finite()
                    || !c.is_finite()
                    || !d.is_finite()
                    || !determinant.is_finite()
                    || determinant.abs() < 1e-12
                {
                    return DirectImageBlit::Unsupported;
                }
                // 计算四角和 AABB，surface scissor 负责裁剪旋转后的边界。
                let corners = glyph_device_corners(
                    destination_rect,
                    self.transform,
                    self.offset_x,
                    self.offset_y,
                );
                if corners.iter().flatten().any(|value| !value.is_finite()) {
                    return DirectImageBlit::Unsupported;
                }
                let (min_x, min_y, max_x, max_y) = quad_aabb(corners);
                let rect = Rect::new(min_x, min_y, max_x - min_x, max_y - min_y);
                if rect.w <= 0.0 || rect.h <= 0.0 {
                    return DirectImageBlit::Culled;
                }
                (corners, rect)
            };
        if device_dst.w <= 0.0 || device_dst.h <= 0.0 {
            return DirectImageBlit::Culled;
        }
        if device_dst.x + device_dst.w <= 0.0
            || device_dst.y + device_dst.h <= 0.0
            || device_dst.x >= self.surface_w as f32
            || device_dst.y >= self.surface_h as f32
        {
            // 完全落在表面外：跳过上传，不是能力缺口。
            return DirectImageBlit::Culled;
        }

        let identity = self.transform.m == Transform::identity().m;
        let one_to_one =
            destination_rect.w == source.width as f32 && destination_rect.h == source.height as f32;
        let (blit_x, blit_y, blit_w, blit_h, crop, blit_corners) = if identity && one_to_one {
            let dest_x = device_dst.x;
            let dest_y = device_dst.y;
            let integer_placement = self.offset_x.fract() == 0.0
                && self.offset_y.fract() == 0.0
                && dest_x.fract() == 0.0
                && dest_y.fract() == 0.0;
            if integer_placement {
                let destination = IntegerFrame {
                    x: dest_x as i32,
                    y: dest_y as i32,
                    width: source.width,
                    height: source.height,
                };
                // 与 clip 求交；表面边界一并收窄，避免部分越界被误判为未实现。
                let mut left = destination.x.max(0);
                let mut top = destination.y.max(0);
                let mut right = destination
                    .x
                    .saturating_add(destination.width)
                    .min(self.surface_w);
                let mut bottom = destination
                    .y
                    .saturating_add(destination.height)
                    .min(self.surface_h);
                if let Some(clip) = rect_to_integer_frame(self.clip_rect) {
                    left = left.max(clip.x);
                    top = top.max(clip.y);
                    right = right.min(clip.x.saturating_add(clip.width));
                    bottom = bottom.min(clip.y.saturating_add(clip.height));
                }
                if left >= right || top >= bottom {
                    return DirectImageBlit::Culled;
                }
                let clipped_dst = IntegerFrame {
                    x: left,
                    y: top,
                    width: right - left,
                    height: bottom - top,
                };
                let clipped_src = IntegerFrame {
                    x: source.x.saturating_add(left - destination.x),
                    y: source.y.saturating_add(top - destination.y),
                    width: clipped_dst.width,
                    height: clipped_dst.height,
                };
                (
                    clipped_dst.x as f32,
                    clipped_dst.y as f32,
                    clipped_dst.width as f32,
                    clipped_dst.height as f32,
                    clipped_src,
                    GpuGlyphBlit::axis_aligned_corners(
                        clipped_dst.x as f32,
                        clipped_dst.y as f32,
                        clipped_dst.width as f32,
                        clipped_dst.height as f32,
                    ),
                )
            } else {
                // 亚像素落点：保留完整源 crop，裁剪交给 GPU scissor。
                (
                    dest_x,
                    dest_y,
                    source.width as f32,
                    source.height as f32,
                    source,
                    device_corners,
                )
            }
        } else {
            // 缩放或轴对齐 view 变换：上传完整源 crop，设备尺寸由 GPU 纹理采样。
            (
                device_dst.x,
                device_dst.y,
                device_dst.w,
                device_dst.h,
                source,
                device_corners,
            )
        };

        let Some(pixel_count) =
            usize::try_from(i64::from(crop.width) * i64::from(crop.height)).ok()
        else {
            return DirectImageBlit::Unsupported;
        };
        let full_source = crop.x == 0
            && crop.y == 0
            && crop.width == source_width
            && crop.height == source_height
            && pixel_count == pixels.len();
        let retained =
            match shared_pixels.filter(|shared| full_source && shared.len() == pixel_count) {
                // 共享源由 ImageService 或上游帧值持有，待提交操作只增加一次强引用。
                Some(shared) => shared,
                // 裁剪或借用入口仍复制为紧密载荷，保持原有边界与失败语义。
                None => {
                    let mut retained = Vec::new();
                    if retained.try_reserve_exact(pixel_count).is_err() {
                        return DirectImageBlit::Unsupported;
                    }
                    let copy_width = crop.width as usize;
                    for y in crop.y..crop.y + crop.height {
                        let row = y as usize * source_stride + crop.x as usize;
                        retained.extend_from_slice(&pixels[row..row + copy_width]);
                    }
                    Arc::new(retained)
                }
            };
        DirectImageBlit::Ready(GpuImageBlit {
            x: blit_x,
            y: blit_y,
            w: blit_w,
            h: blit_h,
            corners: blit_corners,
            opacity: self.opacity.clamp(0.0, 1.0),
            additive: matches!(self.blend_mode, BlendMode::Additive),
            pixels: retained,
            pixel_w: crop.width as u32,
            pixel_h: crop.height as u32,
        })
    }
}
