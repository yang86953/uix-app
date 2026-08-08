//! FrameRecordingCanvas raw image 直达辅助。

// 引入稳定的错误类型与公开图片几何。
use crate::core::{Errc, Error, Rect};
// 引入不可变图片、opacity 与 sampled 目标载荷。
use crate::draw::painting::{FrameImage, FrameOpacity, FrameRect};
// 引入 Canvas2D trait 以读取共享 scratch 的当前裁剪。
use crate::draw::Canvas2D;

// 复用录制画布及其几何、scratch 和 encoder 状态。
use super::canvas::FrameRecordingCanvas;
// 统一把 FrameEncoder 构造错误映射到 graphics Error。
use super::geometry::frame_encoder_error;

// 为 raw image 提供只在语义完整时启用的 Additive 纹理直达路径。
impl FrameRecordingCanvas {
    // identity transform、完整 surface clip 下保留 raw image 为 PictureBlit。
    pub(super) fn record_additive_image_blit(
        // 修改当前命令流并按需 flush 前置 scratch。
        &mut self,
        // 源图片预乘像素。
        pixels: &[u32],
        // 源图片行跨度。
        source_width: i32,
        // 源 crop。
        source_rect: Rect,
        // 目标矩形。
        destination_rect: Rect,
        // 已验证的有限非透明 opacity。
        opacity: FrameOpacity,
    ) -> Result<bool, Error> {
        // PictureBlit 当前不携带任意 canvas clip，只有完整 surface clip 才能直达。
        if self.scratch.has_clip_mask() || self.scratch.current_clip() != self.full_rect() {
            // 部分 clip 必须由 software source tile 烘焙。
            return Ok(false);
        }
        // 只接受 sampled Picture ABI 可表达的 identity transform 几何。
        let Some((source, destination)) =
            // 支持有限 offset、亚像素落点与轴对齐缩放。
            self.sampled_picture_geometry(source_rect, destination_rect)
        else {
            // 任意仿射交给共享软件图片逆映射。
            return Ok(false);
        };
        // 非正源跨度不能形成完整图片行。
        let Ok(source_stride) = usize::try_from(source_width) else {
            // 保持既有无效图片 no-op 边界。
            return Ok(false);
        };
        // 零跨度同样没有有效源。
        if source_stride == 0 {
            // 保持安全 no-op。
            return Ok(false);
        }
        // 只统计完整源行。
        let Ok(source_height) = i32::try_from(pixels.len() / source_stride) else {
            // 无法表达的高度交给软件路径验证。
            return Ok(false);
        };
        // sampled crop 必须完整位于真实源图片内。
        if !source.is_within(source_width, source_height) {
            // 越界源交给软件路径按既有 clamp 规则处理。
            return Ok(false);
        }
        // 计算 retained crop 像素数并保留地址空间错误类型。
        let pixel_count = usize::try_from(i64::from(source.width) * i64::from(source.height))
            // 转换失败代表 crop 超出可寻址内存。
            .map_err(|_| {
                // 返回稳定 graphics OOM。
                Error::new(
                    // 使用图形内存错误码。
                    Errc::GraphicsOutOfMemory,
                    // 说明失败发生在 Additive raw image crop。
                    "additive image blit crop exceeds addressable memory",
                )
            })?;
        // 创建空 retained 像素缓冲，随后显式尝试预留。
        let mut retained = Vec::new();
        // 预留失败必须映射为可观察的 graphics OOM。
        retained.try_reserve_exact(pixel_count).map_err(|error| {
            // 保留 crop 尺寸供诊断。
            Error::new(
                // 使用稳定错误码。
                Errc::GraphicsOutOfMemory,
                // 报告分配尺寸和底层原因。
                format!(
                    // 避免泄露源像素内容，只记录尺寸。
                    "additive image blit crop {}x{} allocation failed: {error}",
                    // crop 宽度。
                    source.width,
                    // crop 高度。
                    source.height,
                ),
            )
        })?;
        // 每行只复制选定 crop，避免保留调用方多余图片数据。
        let copy_width = source.width as usize;
        // 遍历 crop 覆盖的源行。
        for y in source.y..source.y + source.height {
            // 计算当前 crop 行的起始索引。
            let row = y as usize * source_stride + source.x as usize;
            // 追加当前紧行。
            retained.extend_from_slice(&pixels[row..row + copy_width]);
        }
        // 构造拥有独立生命周期的不可变 FrameImage。
        let image = FrameImage::new(source.width, source.height, retained)
            // 统一映射 FrameEncoder 参数错误。
            .map_err(frame_encoder_error)?;
        // retained 图片的源 crop 从自身原点覆盖全图。
        let retained_source = FrameRect::new(0, 0, source.width, source.height);
        // 前置软件源贡献必须先按原 painter order 封口。
        self.flush_scratch()?;
        // 记录复用生产 RHI Additive textured pipeline 的 PictureBlit。
        self.encoder_mut()?.blit_picture_with_opacity_blend(
            // 保留紧 crop 图片。
            image,
            // 从 retained 原点采样完整 crop。
            retained_source,
            // 保留亚像素或缩放目标。
            destination,
            // opacity 由 PictureBlit 在最终目标上只应用一次。
            opacity,
            // 明确选择 One+One Additive 混合。
            true,
        );
        // 告知调用方图片已经完整记录。
        Ok(true)
    }
}
