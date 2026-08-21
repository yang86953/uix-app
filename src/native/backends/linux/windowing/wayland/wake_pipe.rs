// Wayland wake-pipe Module 独占非阻塞唤醒描述符的排空操作。

// typed error 保留 owner-thread I/O failure 分类。
use crate::core::{Errc, Error};
// 跨线程唤醒句柄封装安全闭包边界。
use crate::platform::windowing::event::EventLoopWaker;

// backend 提供 wake FD 与既有 pending failure source。
use super::WaylandBackend;

impl WaylandBackend {
    // 构造只捕获稳定写端与 runtime failure source 的跨线程句柄。
    pub(crate) fn waker(&self) -> EventLoopWaker {
        // 写端在 backend 生命周期内保持稳定 identity。
        let file_descriptor = self.wake_write_fd;
        // failure source 允许后台唤醒失败回到 owner thread。
        let pending_failures = self.pending_failures.clone();
        // 闭包不捕获 WaylandBackend 本身。
        EventLoopWaker::new(move || {
            // 单字节小于 PIPE_BUF，并发唤醒仍保持原子。
            let byte = [1_u8];
            // SAFETY: file_descriptor 是存活 backend 的非阻塞 pipe 写端，byte 在同步调用期间有效。
            let result = unsafe {
                // write 只读取当前一字节缓冲。
                libc::write(
                    // 使用构造时复制的稳定 FD。
                    file_descriptor,
                    // 传入只读字节首地址。
                    byte.as_ptr().cast(),
                    // 只提交一个唤醒字节。
                    byte.len(),
                )
            };
            // 负数结果需要区分可恢复的非阻塞状态。
            if result < 0 {
                // 读取线程本地 errno。
                let error = std::io::Error::last_os_error();
                // WouldBlock 已有未消费唤醒，Interrupted 可由调用方后续再次唤醒。
                if !matches!(
                    // 检查稳定错误类别。
                    error.kind(),
                    // 两类错误均不表示 backend 永久失效。
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) {
                    // 其他错误进入既有 pending source。
                    let _ = pending_failures.enqueue(Error::new(
                        // pipe write failure 属于 I/O 错误。
                        Errc::IoError,
                        // 保留底层 cause。
                        format!("Wayland wake pipe write failed: {error}"),
                    ));
                }
            }
        })
    }

    // 排空所有已经提交的单字节唤醒信号。
    pub(super) fn drain_wake_pipe(&self) {
        // 固定栈缓冲减少 read syscall 次数。
        let mut buffer = [0_u8; 64];
        // 非阻塞读取直到 WouldBlock。
        loop {
            // SAFETY: wake_read_fd 在 backend 存活期内有效，buffer 可写且 read 同步返回。
            let result = unsafe {
                // 系统调用最多写入 buffer.len() 字节。
                libc::read(
                    // 使用 backend 独占的 pipe 读端。
                    self.wake_read_fd,
                    // 传入当前栈缓冲首地址。
                    buffer.as_mut_ptr().cast(),
                    // 明确缓冲区容量。
                    buffer.len(),
                )
            };
            // 正数表示仍可能存在后续信号。
            if result > 0 {
                // 继续排空。
                continue;
            }
            // 负数需要区分可恢复状态。
            if result < 0 {
                // 读取线程本地 errno。
                let error = std::io::Error::last_os_error();
                // 信号中断时重试同一描述符。
                if error.kind() == std::io::ErrorKind::Interrupted {
                    // 不丢失唤醒。
                    continue;
                }
                // WouldBlock 表示 pipe 已经排空。
                if error.kind() != std::io::ErrorKind::WouldBlock {
                    // 其他错误进入 backend 既有 source。
                    self.enqueue_failure(Error::new(
                        // pipe read failure 属于 I/O 错误。
                        Errc::IoError,
                        // 保留底层 cause。
                        format!("Wayland wake pipe read failed: {error}"),
                    ));
                }
            }
            // EOF、WouldBlock 或已上报错误均结束本轮排空。
            break;
        }
    }
}
