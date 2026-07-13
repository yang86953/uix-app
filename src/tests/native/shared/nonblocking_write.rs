use std::collections::VecDeque;
use std::io::{self, Write};
use std::sync::Arc;

use crate::native::shared::nonblocking_write::{NonBlockingWriteCursor, NonBlockingWriteStatus};

enum WriteStep {
    Accept(usize),
    WouldBlock,
}

struct ScriptedWriter {
    steps: VecDeque<WriteStep>,
    bytes: Vec<u8>,
}

impl ScriptedWriter {
    fn new(steps: impl IntoIterator<Item = WriteStep>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
            bytes: Vec::new(),
        }
    }
}

impl Write for ScriptedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        match self
            .steps
            .pop_front()
            .unwrap_or(WriteStep::Accept(bytes.len()))
        {
            WriteStep::Accept(limit) => {
                let count = limit.min(bytes.len());
                self.bytes.extend_from_slice(&bytes[..count]);
                Ok(count)
            }
            WriteStep::WouldBlock => Err(io::Error::from(io::ErrorKind::WouldBlock)),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn nonblocking_write_resumes_after_partial_write_and_would_block() {
    let expected = b"first-second".to_vec();
    let mut write = NonBlockingWriteCursor::new(Arc::from(expected.clone()));
    let mut writer = ScriptedWriter::new([
        WriteStep::Accept(5),
        WriteStep::WouldBlock,
        WriteStep::Accept(usize::MAX),
    ]);

    let first = write.write_available(&mut writer, 64).unwrap();
    assert_eq!(first.status, NonBlockingWriteStatus::Pending);
    assert_eq!(first.written, 5);

    let second = write.write_available(&mut writer, 64).unwrap();
    assert_eq!(second.status, NonBlockingWriteStatus::Complete);
    assert_eq!(second.written, expected.len() - 5);
    assert_eq!(writer.bytes, expected);
}

#[test]
fn nonblocking_write_budget_bounds_each_dispatch() {
    let expected = (0_u8..12).collect::<Vec<_>>();
    let mut write = NonBlockingWriteCursor::new(Arc::from(expected.clone()));
    let mut writer = ScriptedWriter::new([]);

    for _ in 0..2 {
        let progress = write.write_available(&mut writer, 4).unwrap();
        assert_eq!(progress.status, NonBlockingWriteStatus::Pending);
        assert_eq!(progress.written, 4);
    }
    let progress = write.write_available(&mut writer, 4).unwrap();
    assert_eq!(progress.status, NonBlockingWriteStatus::Complete);
    assert_eq!(progress.written, 4);
    assert_eq!(writer.bytes, expected);
}
