// ============================================================================
// native/shared/nonblocking_read.rs — 有界非阻塞读取累积器
//
// 调用方负责提供 nonblocking reader；本类型只负责跨轮保留字节，并限制单轮工作量。
// ============================================================================

use std::io::{self, Read};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum NonBlockingReadStatus {
    Pending,
    Complete(Vec<u8>),
}

#[derive(Debug, Default)]
pub(crate) struct NonBlockingReadAccumulator {
    bytes: Vec<u8>,
}

impl NonBlockingReadAccumulator {
    /// 读取当前已经可用的字节；`budget` 用尽、WouldBlock 或 Interrupted 时让出本轮。
    pub(crate) fn read_available<R: Read>(
        &mut self,
        reader: &mut R,
        budget: usize,
    ) -> io::Result<NonBlockingReadStatus> {
        let mut remaining = budget;
        let mut chunk = [0_u8; 8 * 1024];

        while remaining > 0 {
            let read_len = remaining.min(chunk.len());
            match reader.read(&mut chunk[..read_len]) {
                Ok(0) => {
                    return Ok(NonBlockingReadStatus::Complete(std::mem::take(
                        &mut self.bytes,
                    )));
                }
                Ok(count) => {
                    self.bytes.extend_from_slice(&chunk[..count]);
                    remaining -= count;
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    return Ok(NonBlockingReadStatus::Pending);
                }
                Err(error) => return Err(error),
            }
        }

        Ok(NonBlockingReadStatus::Pending)
    }
}
