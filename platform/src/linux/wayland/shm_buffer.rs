// ============================================================================
// platform/linux/wayland/shm_buffer.rs — SHM 共享内存缓冲区
//
// RAII 包装：SHM 池 + wl_buffer + 后备文件。
// 用于 Wayland 像素呈现的双缓冲。
// ============================================================================

use std::fs::File;
use wayland_client::protocol::{wl_buffer, wl_shm_pool};
use wayland_client::Main;

/// SHM 缓冲区：SHM 池 + wl_buffer + 后备临时文件。
pub(crate) struct ShmBuffer {
    pub(crate) file: File,
    pub(crate) size: usize,
    #[allow(dead_code)]
    pub(crate) pool: Main<wl_shm_pool::WlShmPool>,
    pub(crate) buffer: Main<wl_buffer::WlBuffer>,
}

impl ShmBuffer {
    /// 将 BGRA 像素数据写入共享内存。
    pub(crate) fn write_pixels(&mut self, pixels: &[u32]) -> std::io::Result<usize> {
        use std::io::{Seek, Write};
        let byte_len = pixels.len().min(self.size / 4) * 4;
        self.file.seek(std::io::SeekFrom::Start(0))?;
        // SAFETY: &[u32] → &[u8] 的安全转换，长度对齐确保不越界。
        let bytes = unsafe { std::slice::from_raw_parts(pixels.as_ptr() as *const u8, byte_len) };
        self.file.write(bytes)
    }
}
