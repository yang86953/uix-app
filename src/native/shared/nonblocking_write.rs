// ============================================================================
// native/shared/nonblocking_write.rs — 有界非阻塞写入游标
//
// 调用方负责提供 nonblocking writer；本类型跨轮保留 offset，并报告本轮写入量。
// ============================================================================

use std::io::{self, Write};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NonBlockingWriteStatus {
    Pending,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NonBlockingWriteProgress {
    pub(crate) status: NonBlockingWriteStatus,
    pub(crate) written: usize,
}

#[derive(Debug)]
pub(crate) struct NonBlockingWriteCursor {
    bytes: Arc<[u8]>,
    offset: usize,
}

impl NonBlockingWriteCursor {
    pub(crate) fn new(bytes: Arc<[u8]>) -> Self {
        Self { bytes, offset: 0 }
    }

    /// 写入当前可接受的字节；`budget` 用尽、WouldBlock 或 Interrupted 时让出本轮。
    pub(crate) fn write_available<W: Write>(
        &mut self,
        writer: &mut W,
        budget: usize,
    ) -> io::Result<NonBlockingWriteProgress> {
        let mut written = 0;

        while self.offset < self.bytes.len() && written < budget {
            let end = self
                .bytes
                .len()
                .min(self.offset.saturating_add(budget - written));
            match writer.write(&self.bytes[self.offset..end]) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "nonblocking writer returned zero before completion",
                    ));
                }
                Ok(count) => {
                    self.offset += count;
                    written += count;
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    break;
                }
                Err(error) => return Err(error),
            }
        }

        Ok(NonBlockingWriteProgress {
            status: if self.offset == self.bytes.len() {
                NonBlockingWriteStatus::Complete
            } else {
                NonBlockingWriteStatus::Pending
            },
            written,
        })
    }
}
