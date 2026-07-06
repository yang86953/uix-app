// ============================================================================
// platform/linux/wayland/presenter.rs — Wayland SHM 像素呈现
//
// WaylandPresenter 实现 IPresenter，通过共享内存（SHM）将像素数据呈现到
// Wayland surface。支持双缓冲和脏矩形局部更新。
// ============================================================================

use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::io::AsRawFd;

use wayland_client::protocol::{wl_callback, wl_compositor, wl_shm, wl_surface};
use wayland_client::Main;

use crate::native::traits::present::IPresenter;
use crate::native::traits::present::PresentDamage;

use super::shm_buffer::ShmBuffer;

/// Wayland SHM 像素呈现器。
///
/// 持有与窗口 surface 关联的 SHM 双缓冲，实现 IPresenter。
pub struct WaylandPresenter {
    compositor: Main<wl_compositor::WlCompositor>,
    shm: Main<wl_shm::WlShm>,
    surface: Option<Main<wl_surface::WlSurface>>,
    shm_buffers: [Option<ShmBuffer>; 2],
    active_buffer: usize,
    width: i32,
    height: i32,
    shown: bool,
}

impl WaylandPresenter {
    pub fn new(
        compositor: Main<wl_compositor::WlCompositor>,
        shm: Main<wl_shm::WlShm>,
        surface: Option<Main<wl_surface::WlSurface>>,
        width: i32,
        height: i32,
    ) -> Self {
        Self {
            compositor,
            shm,
            surface,
            shm_buffers: [None, None],
            active_buffer: 0,
            width,
            height,
            shown: false,
        }
    }

    /// 将像素数据提交到 Wayland surface。
    fn present_impl(&mut self, pixels: &[u32], width: i32, height: i32, damage: PresentDamage) {
        let needs_resize = width != self.width
            || height != self.height
            || self.shm_buffers[0].is_none()
            || self.shm_buffers[1].is_none();

        if needs_resize {
            let mut new_bufs: [Option<ShmBuffer>; 2] = [None, None];
            for (i, buf) in new_bufs.iter_mut().enumerate() {
                match self.create_shm_buffer(width, height, i) {
                    Ok(b) => *buf = Some(b),
                    Err(e) => {
                        crate::core::log::warn_fn(format!("Wayland SHM[{}] fail: {}", i, e));
                        return;
                    }
                }
            }
            self.width = width;
            self.height = height;
            self.shm_buffers = new_bufs;
        }

        let write_idx = 1 - self.active_buffer;
        let shm = match self.shm_buffers[write_idx].as_mut() {
            Some(s) => s,
            None => return,
        };

        let byte_len = pixels.len().min(shm.size / 4) * 4;
        let bytes = unsafe { std::slice::from_raw_parts(pixels.as_ptr() as *const u8, byte_len) };

        if shm.file.seek(SeekFrom::Start(0)).is_err() {
            return;
        }
        if shm.file.write(bytes).is_err() {
            return;
        }

        let surface = match self.surface.as_ref() {
            Some(s) => s,
            None => return,
        };

        surface.attach(Some(&shm.buffer), 0, 0);
        match damage {
            PresentDamage::Partial(ref rects) => {
                for &(x, y, w, h) in rects {
                    if w > 0 && h > 0 {
                        surface.damage_buffer(x, y, w, h);
                    }
                }
            }
            PresentDamage::Full => surface.damage(0, 0, width, height),
        }

        let _cb: Main<wl_callback::WlCallback> = surface.frame();
        surface.commit();
        self.active_buffer = write_idx;

        if !self.shown {
            self.shown = true;
        }
    }

    /// 创建 SHM 缓冲区。
    fn create_shm_buffer(
        &self,
        width: i32,
        height: i32,
        buffer_index: usize,
    ) -> Result<ShmBuffer, String> {
        use wayland_client::protocol::wl_shm as wl_shm_proto;

        let stride = width * 4;
        let size = (stride * height) as usize;
        let tmp =
            std::env::temp_dir().join(format!("uix-shm-{}-{}", std::process::id(), buffer_index));

        let mut f = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp)
            .map_err(|e| format!("shm open: {}", e))?;

        f.set_len(size as u64)
            .map_err(|e| format!("shm len: {}", e))?;
        f.seek(SeekFrom::Start((size - 1) as u64)).ok();
        f.write_all(&[0u8]).ok();
        f.flush().ok();

        let fd = f.as_raw_fd();
        let pool = self.shm.create_pool(fd, size as i32);
        let buf = pool.create_buffer(0, width, height, stride, wl_shm_proto::Format::Argb8888);

        let _ = std::fs::remove_file(&tmp);
        Ok(ShmBuffer {
            file: f,
            size,
            pool,
            buffer: buf,
        })
    }
}

// ════════════════════════════════════════════════════════════════════════════
// IPresenter
// ════════════════════════════════════════════════════════════════════════════

impl IPresenter for WaylandPresenter {
    fn present(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        damage: PresentDamage,
    ) -> Result<(), Error> {
        self.present_impl(pixels, width, height, dirty_rect);
        Ok(())
    }

    fn resize(&mut self, _width: i32, _height: i32) -> Result<(), Error> {
        // 实际 resize 在下次 present 时惰性执行
        Ok(())
    }
}
