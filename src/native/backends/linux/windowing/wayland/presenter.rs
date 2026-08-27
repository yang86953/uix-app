// ============================================================================
// platform/linux/wayland/presenter.rs — Wayland SHM 像素呈现
//
// WaylandPresenter 实现 IPresenter，通过共享内存（SHM）将像素数据呈现到
// Wayland surface。支持双缓冲和脏矩形局部更新。
// ============================================================================

// presenter 与 windowing 共享逐窗 surface metrics owner。
use std::sync::Arc;

use super::compat::Main;
use wayland_client::protocol::{wl_shm, wl_surface};

use crate::core::{Errc, Error, PresentDamage, PresentSurface, Result};
use crate::platform::presentation::validate_pixel_buffer;
// SHM drawable 使用与 EGL 相同的 logical/drawable 原子快照。
use crate::native::presentation::graphics::platform::linux::WaylandSurfaceMetrics;
use crate::platform::presentation::IPresenter;

use super::shm_buffer::{ShmBuffer, create_argb_buffer};

/// Wayland SHM 像素呈现器。
///
/// 持有与窗口 surface 关联的 SHM 双缓冲，实现 IPresenter。
pub(crate) struct WaylandPresenter {
    shm: Main<wl_shm::WlShm>,
    surface: Option<Main<wl_surface::WlSurface>>,
    shm_buffers: [Option<ShmBuffer>; 2],
    active_buffer: usize,
    width: i32,
    height: i32,
    // 逐窗 metrics 是 logical extent、drawable extent 与 DPR 的唯一事实源。
    metrics: Arc<WaylandSurfaceMetrics>,
    // scale 大于一时复用此缓冲保存最近邻放大的物理像素。
    scaled_pixels: Vec<u32>,
    shown: bool,
}

impl WaylandPresenter {
    pub(crate) fn new(
        shm: Main<wl_shm::WlShm>,
        surface: Option<Main<wl_surface::WlSurface>>,
        width: i32,
        height: i32,
        // 接收窗口与 graphics descriptor 共用的逐窗 metrics。
        metrics: Arc<WaylandSurfaceMetrics>,
    ) -> Self {
        // 初始 SHM buffer 尺寸必须使用物理 drawable extent。
        let snapshot = metrics.snapshot().ok();
        Self {
            shm,
            surface,
            shm_buffers: [None, None],
            active_buffer: 0,
            // 快照不可用时保守回退到调用方逻辑宽度。
            width: snapshot.map_or(width, |surface| surface.drawable_width),
            // 快照不可用时保守回退到调用方逻辑高度。
            height: snapshot.map_or(height, |surface| surface.drawable_height),
            // 保存同一逐窗 metrics owner。
            metrics,
            // 首次呈现前不分配缩放像素。
            scaled_pixels: Vec::new(),
            shown: false,
        }
    }

    // 把逻辑 BGRA 像素按 Wayland core 整数 scale 做最近邻放大。
    fn scale_pixels(
        // 借用源逻辑像素。
        pixels: &[u32],
        // 源逻辑宽度。
        width: i32,
        // 源逻辑高度。
        height: i32,
        // 当前正整数 buffer scale。
        scale: i32,
        // 复用 presenter 私有物理像素缓冲。
        output: &mut Vec<u32>,
    ) -> Result<()> {
        // scale=1 时调用方直接写原始像素，无需复制。
        if scale == 1 {
            // 清空旧放大结果，避免无意义保留大块逻辑内容。
            output.clear();
            // identity 路径完成。
            return Ok(());
        }
        // 受检计算物理宽度，拒绝整数溢出。
        let drawable_width = width.checked_mul(scale).ok_or_else(|| {
            // 过大 drawable 属于资源尺寸不足。
            Error::new(
                Errc::InsufficientResources,
                "Wayland SHM scaled width overflow",
            )
        })?;
        // 受检计算物理高度，拒绝整数溢出。
        let drawable_height = height.checked_mul(scale).ok_or_else(|| {
            // 过大 drawable 属于资源尺寸不足。
            Error::new(
                Errc::InsufficientResources,
                "Wayland SHM scaled height overflow",
            )
        })?;
        // 受检计算目标像素总数。
        let output_len = (drawable_width as usize)
            // 高度乘法不得在 usize 上回绕。
            .checked_mul(drawable_height as usize)
            // 映射到统一资源错误。
            .ok_or_else(|| {
                Error::new(
                    // 内存尺寸表达失败属于资源不足。
                    Errc::InsufficientResources,
                    // 保留明确 SHM 缩放阶段。
                    "Wayland SHM scaled pixel count overflow",
                )
            })?;
        // 一次调整可复用缓冲到完整物理像素数。
        output.resize(output_len, 0);
        // 逐逻辑行复制到 scale 个物理行。
        for source_y in 0..height as usize {
            // 逐逻辑像素复制到 scale 个物理列。
            for source_x in 0..width as usize {
                // 读取当前源像素一次。
                let pixel = pixels[source_y * width as usize + source_x];
                // 填充目标 scale×scale 像素块。
                for offset_y in 0..scale as usize {
                    // 计算当前目标行起点。
                    let target_y = source_y * scale as usize + offset_y;
                    // 计算当前像素块目标起点。
                    let target_start =
                        target_y * drawable_width as usize + source_x * scale as usize;
                    // 整块物理列填入同一像素。
                    output[target_start..target_start + scale as usize].fill(pixel);
                }
            }
        }
        // 完整物理缓冲已建立。
        Ok(())
    }

    /// 将像素数据提交到 Wayland surface。
    fn present_impl(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        damage: PresentDamage,
    ) -> Result<()> {
        validate_pixel_buffer(pixels, width, height)?;
        // 从当前 metrics 读取 scale，再用本次帧逻辑尺寸原子刷新快照。
        let current = self.metrics.snapshot()?;
        // presenter 输入是逻辑 Canvas extent，drawable 由共享 scale 换算。
        let snapshot = self.metrics.update(width, height, current.scale)?;
        // scale 大于一时生成与 drawable 完全一致的物理像素。
        Self::scale_pixels(
            // 输入仍是逻辑 Canvas 像素。
            pixels,
            // 传入逻辑宽度。
            width,
            // 传入逻辑高度。
            height,
            // 使用同一快照的整数 scale。
            snapshot.scale,
            // 写入可复用物理像素缓冲。
            &mut self.scaled_pixels,
        )?;
        // SHM 双缓冲的原生尺寸使用物理 drawable。
        let needs_resize = snapshot.drawable_width != self.width
            || snapshot.drawable_height != self.height
            || self.shm_buffers[0].is_none()
            || self.shm_buffers[1].is_none();

        if needs_resize {
            let mut new_bufs: [Option<ShmBuffer>; 2] = [None, None];
            for (i, buf) in new_bufs.iter_mut().enumerate() {
                match self.create_shm_buffer(snapshot.drawable_width, snapshot.drawable_height, i) {
                    Ok(b) => *buf = Some(b),
                    Err(e) => {
                        return Err(Error::new(
                            Errc::PlatformError,
                            format!("WaylandPresenter: create SHM buffer {i} failed: {e}"),
                        ));
                    }
                }
            }
            self.width = snapshot.drawable_width;
            self.height = snapshot.drawable_height;
            self.shm_buffers = new_bufs;
            self.active_buffer = 0;
        }

        let surface = self.surface.as_ref().ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "WaylandPresenter: Wayland surface is unavailable",
            )
        })?;
        let write_idx = [1 - self.active_buffer, self.active_buffer]
            .into_iter()
            .find(|&index| {
                self.shm_buffers[index]
                    .as_ref()
                    .is_some_and(ShmBuffer::try_acquire)
            })
            .ok_or_else(|| {
                Error::new(
                    Errc::WouldBlock,
                    "WaylandPresenter: all SHM buffers are busy",
                )
            })?;
        let shm = self.shm_buffers[write_idx].as_mut().ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "WaylandPresenter: selected SHM buffer is unavailable",
            )
        })?;

        // identity scale 直接写源像素，HiDPI 写最近邻放大后的物理像素。
        let write_result = if snapshot.scale == 1 {
            // scale=1 不产生额外复制。
            shm.write_pixels(pixels)
        } else {
            // scale>1 的缓冲长度与 SHM drawable 完全一致。
            shm.write_pixels(&self.scaled_pixels)
        };
        // 失败必须释放刚取得的 buffer lease。
        if let Err(error) = write_result {
            shm.release();
            return Err(Error::new(
                Errc::PlatformError,
                format!("WaylandPresenter: write SHM buffer failed: {error}"),
            ));
        }

        // 在提交新 buffer 前声明其整数坐标比例。
        surface.set_buffer_scale(snapshot.scale);
        // buffer 原点保持 surface logical 原点。
        surface.attach(Some(&shm.buffer), 0, 0);
        match damage {
            PresentDamage::Partial(ref rects) => {
                for &(x, y, w, h) in rects {
                    if w > 0 && h > 0 {
                        surface.damage_buffer(x, y, w, h);
                    }
                }
            }
            // damage planner 已按 DPR 生成物理 buffer 坐标。
            PresentDamage::Full => surface.damage_buffer(
                // 从物理原点开始损伤。
                0,
                // 从物理原点开始损伤。
                0,
                // 使用完整物理宽度。
                snapshot.drawable_width,
                // 使用完整物理高度。
                snapshot.drawable_height,
            ),
        }

        surface.commit();
        self.active_buffer = write_idx;

        if !self.shown {
            self.shown = true;
        }
        Ok(())
    }

    /// 创建 SHM 缓冲区。
    fn create_shm_buffer(
        &self,
        width: i32,
        height: i32,
        buffer_index: usize,
    ) -> Result<ShmBuffer, String> {
        create_argb_buffer(&self.shm, width, height, &format!("present-{buffer_index}"))
    }
}

// ════════════════════════════════════════════════════════════════════════════
// IPresenter
// ════════════════════════════════════════════════════════════════════════════

impl IPresenter for WaylandPresenter {
    // 返回逐窗 drawable、DPR 与 surface revision 的同代快照。
    fn present_surface(
        // 借用当前 presenter。
        &self,
        // metrics 损坏时使用调用方提供的 drawable 宽度。
        drawable_width: i32,
        // metrics 损坏时使用调用方提供的 drawable 高度。
        drawable_height: i32,
        // metrics 损坏时使用调用方提供的 DPR。
        device_pixel_ratio: f32,
    ) -> PresentSurface {
        // 健康 metrics 直接生成不可拆分的 native surface 快照。
        match self.metrics.snapshot() {
            // Wayland core scale 同时定义 drawable extent 与 DPR。
            Ok(snapshot) => PresentSurface::identity(
                // 使用当前物理宽度。
                snapshot.drawable_width,
                // 使用当前物理高度。
                snapshot.drawable_height,
                // 整数 scale 转成公开 DPR。
                snapshot.scale as f32,
                // metrics revision 在尺寸或 scale 变化时推进。
                snapshot.revision,
            ),
            // 无返回错误通道时保留调用方提供的安全回退。
            Err(_) => PresentSurface::identity(
                // 回退 drawable 宽度。
                drawable_width,
                // 回退 drawable 高度。
                drawable_height,
                // 回退 renderer DPR。
                device_pixel_ratio,
                // 损坏快照不伪造可追踪代次。
                0,
            ),
        }
    }

    fn present(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        damage: PresentDamage,
    ) -> Result<(), Error> {
        self.present_impl(pixels, width, height, damage)
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        // 读取当前整数 scale，与新 logical extent 一次提交。
        let current = self.metrics.snapshot()?;
        // SHM buffer 仍在下次 present 时按新物理 extent 惰性重建。
        self.metrics
            .update(width.max(1), height.max(1), current.scale)?;
        // metrics 更新成功。
        Ok(())
    }
}
