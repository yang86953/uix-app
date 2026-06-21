// ============================================================================
// platform/linux/wayland/presenter.rs — IPresenter + 呈现相关内部方法
// ============================================================================

use std::fs::File;
use std::os::unix::io::AsRawFd;

use wayland_client::protocol::{wl_buffer, wl_shm, wl_shm_pool, wl_surface};
use wayland_client::Main;

use uix_diag::Error;
use crate::INativeHandle;
use crate::IPresenter;

use super::ShmBuffer;
use super::WaylandBackend;

// ════════════════════════════════════════════════════════════════════════════
// IPresenter
// ════════════════════════════════════════════════════════════════════════════

impl IPresenter for WaylandBackend {
    fn present(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        dirty_rect: Option<(i32, i32, i32, i32)>,
    ) -> Result<(), Error> {
        self.present_pixels(pixels, width, height, dirty_rect);
        Ok(())
    }

    fn resize(&mut self, _width: i32, _height: i32) -> Result<(), Error> {
        Ok(())
    }
}

// ════════════════════════════════════════════════════════════════════════════
// INativeHandle
// ════════════════════════════════════════════════════════════════════════════

impl INativeHandle for WaylandBackend {
    fn native_window(&self) -> *mut std::ffi::c_void {
        self.surface.as_ref().map_or(std::ptr::null_mut(), |s| {
            s as *const Main<wl_surface::WlSurface> as *mut std::ffi::c_void
        })
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 内部方法
// ════════════════════════════════════════════════════════════════════════════

impl WaylandBackend {
    /// 将 ARGB 像素数据呈现到窗口表面（SHM 双缓冲）。
    pub(crate) fn present_pixels(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        dirty_rect: Option<(i32, i32, i32, i32)>,
    ) {
        // 尺寸变化时重建双缓冲
        // 注意：先创建缓冲再更新 self.width/height，失败时保持旧状态
        let needs_resize = width != self.width
            || height != self.height
            || self.shm_buffers[0].is_none()
            || self.shm_buffers[1].is_none();
        if needs_resize {
            let mut new_bufs: [Option<ShmBuffer>; 2] = [None, None];
            for (i, buf) in new_bufs.iter_mut().enumerate() {
                match Self::create_shm_buffer(&self._shm, width, height, i) {
                    Ok(b) => *buf = Some(b),
                    Err(e) => {
                        log::warn!("Wayland: SHM buffer[{}] {}x{} 创建失败: {}", i, width, height, e);
                        return;  // 保持旧 self.width/height，下次重试
                    }
                }
            }
            self.width = width;
            self.height = height;
            self.shm_buffers = new_bufs;
        }

        // 写入 compositor 未读取的缓冲区
        let write_idx = 1 - self.active_buffer;
        let shm = match self.shm_buffers[write_idx].as_mut() {
            Some(s) => s,
            None => return,
        };
        if let Err(e) = shm.write_pixels(pixels) {
            log::warn!("Wayland: write_pixels 失败: {}", e);
            return;
        }

        let surface = match self.surface.as_ref() {
            Some(s) => s,
            None => return,
        };

        // 每次 present 时重新设置输入区域
        {
            let region = self._compositor.create_region();
            region.add(0, 0, width, height);
            surface.set_input_region(Some(&region));
            self.input_region = Some(region);
        }

        surface.attach(Some(&shm.buffer), 0, 0);
        match dirty_rect {
            Some((x, y, w, h)) if w > 0 && h > 0 => {
                surface.damage_buffer(x, y, w, h);
            }
            _ => {
                surface.damage(0, 0, width, height);
            }
        }
        surface.commit();
        self.active_buffer = write_idx;

        if !self.shown {
            self.shown = true;
        }

        self.try_dispatch();
    }

    /// 创建 SHM 双缓冲中的一个缓冲区。
    /// `buffer_index` 用于生成唯一的临时文件路径，避免双缓冲之间的路径冲突。
    fn create_shm_buffer(
        shm: &Main<wl_shm::WlShm>,
        width: i32,
        height: i32,
        buffer_index: usize,
    ) -> Result<ShmBuffer, String> {
        let stride = width * 4;
        let size = (stride * height) as usize;

        // 每个 buffer 使用独立 temp 文件，避免路径冲突
        let tmp_path = std::env::temp_dir()
            .join(format!("uix-shm-{}-{}", std::process::id(), buffer_index));
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_path)
            .map_err(|e| format!("shm temp file create: {}", e))?;

        file.set_len(size as u64).map_err(|e| format!("shm set_len: {}", e))?;

        // 预分配文件页（写入最后一个字节触发内核分配所有页）
        {
            use std::io::{Seek, Write};
            file.seek(std::io::SeekFrom::Start((size - 1) as u64))
                .map_err(|e| format!("shm seek: {}", e))?;
            file.write_all(&[0u8])
                .map_err(|e| format!("shm write tail: {}", e))?;
            file.flush().ok();
        }

        let raw_fd = file.as_raw_fd();
        let pool = shm.create_pool(raw_fd, size as i32);
        let buffer = pool.create_buffer(0, width, height, stride, wl_shm::Format::Argb8888);

        let _ = std::fs::remove_file(&tmp_path);

        Ok(ShmBuffer {
            file,
            size,
            pool,
            buffer,
        })
    }
}
