// ============================================================================
// platform/linux/wayland/shm_buffer.rs — SHM 共享内存缓冲区
//
// RAII 包装：SHM 池 + wl_buffer + 后备文件。
// 用于 Wayland 像素呈现的双缓冲。
// ============================================================================

use super::compat::Main;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::io::AsRawFd;
use wayland_client::protocol::{wl_buffer, wl_shm, wl_shm_pool};

use crate::native::windowing::shared::buffer_lease::BufferLease;

// 创建 presenter 与客户端阴影共用的 ARGB8888 SHM buffer。
pub(crate) fn create_argb_buffer(
    shm: &Main<wl_shm::WlShm>,
    width: i32,
    height: i32,
    label: &str,
) -> Result<ShmBuffer, String> {
    if width <= 0 || height <= 0 {
        return Err(format!("invalid SHM extent {width}x{height}"));
    }
    let stride = width
        .checked_mul(4)
        .ok_or_else(|| format!("SHM stride overflow for width {width}"))?;
    let size = stride
        .checked_mul(height)
        .ok_or_else(|| format!("SHM size overflow for {width}x{height}"))? as usize;
    let pool_size =
        i32::try_from(size).map_err(|_| format!("SHM size exceeds Wayland i32 limit: {size}"))?;
    let tmp = std::env::temp_dir().join(format!("uix-shm-{}-{label}", std::process::id()));
    let mut file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&tmp)
        .map_err(|error| format!("shm open: {error}"))?;
    file.set_len(size as u64)
        .map_err(|error| format!("shm len: {error}"))?;
    file.seek(SeekFrom::Start((size - 1) as u64))
        .map_err(|error| format!("shm seek: {error}"))?;
    file.write_all(&[0u8])
        .map_err(|error| format!("shm initialize: {error}"))?;
    file.flush()
        .map_err(|error| format!("shm flush: {error}"))?;
    let pool = shm.create_pool(file.as_raw_fd(), pool_size);
    let buffer = pool.create_buffer(0, width, height, stride, wl_shm::Format::Argb8888);
    let lease = BufferLease::new();
    let released = lease.clone();
    buffer.quick_assign(move |_, event, _| {
        if matches!(event, wl_buffer::Event::Release) {
            released.release();
        }
    });
    let _ = std::fs::remove_file(&tmp);
    Ok(ShmBuffer {
        file,
        size,
        pool,
        buffer,
        lease,
    })
}

/// SHM 缓冲区：SHM 池 + wl_buffer + 后备临时文件。
pub(crate) struct ShmBuffer {
    pub(crate) file: File,
    pub(crate) size: usize,
    #[allow(dead_code)]
    pub(crate) pool: Main<wl_shm_pool::WlShmPool>,
    pub(crate) buffer: Main<wl_buffer::WlBuffer>,
    pub(crate) lease: BufferLease,
}

impl ShmBuffer {
    pub(crate) fn try_acquire(&self) -> bool {
        self.lease.try_acquire()
    }

    pub(crate) fn release(&self) {
        self.lease.release();
    }

    /// 将 BGRA 像素数据写入共享内存。
    pub(crate) fn write_pixels(&mut self, pixels: &[u32]) -> std::io::Result<()> {
        use std::io::{Seek, Write};
        let byte_len = pixels.len().min(self.size / 4) * 4;
        self.file.seek(std::io::SeekFrom::Start(0))?;
        // SAFETY: &[u32] → &[u8] 的安全转换，长度对齐确保不越界。
        let bytes = unsafe { std::slice::from_raw_parts(pixels.as_ptr() as *const u8, byte_len) };
        self.file.write_all(bytes)
    }
}

// 让 SHM buffer Component 在所有回滚与替换路径统一释放 callback owner。
impl Drop for ShmBuffer {
    // buffer handle 仍存活时注销唯一 wl_buffer::Release callback。
    fn drop(&mut self) {
        // 只删除兼容层 registry entry，不发送协议请求或伪造 Release。
        self.buffer.clear_callback();
    }
}
