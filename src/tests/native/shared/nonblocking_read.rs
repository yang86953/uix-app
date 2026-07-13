use std::collections::VecDeque;
use std::io::{self, Cursor, Read};

use crate::native::shared::nonblocking_read::{NonBlockingReadAccumulator, NonBlockingReadStatus};

enum ReadStep {
    Bytes(Vec<u8>),
    WouldBlock,
    Eof,
}

struct ScriptedReader {
    steps: VecDeque<ReadStep>,
}

impl ScriptedReader {
    fn new(steps: impl IntoIterator<Item = ReadStep>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
        }
    }
}

impl Read for ScriptedReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match self.steps.pop_front().unwrap_or(ReadStep::Eof) {
            ReadStep::Bytes(mut bytes) => {
                let count = bytes.len().min(buffer.len());
                buffer[..count].copy_from_slice(&bytes[..count]);
                if count < bytes.len() {
                    bytes.drain(..count);
                    self.steps.push_front(ReadStep::Bytes(bytes));
                }
                Ok(count)
            }
            ReadStep::WouldBlock => Err(io::Error::from(io::ErrorKind::WouldBlock)),
            ReadStep::Eof => Ok(0),
        }
    }
}

#[test]
fn nonblocking_read_preserves_partial_bytes_across_would_block() {
    let mut reader = ScriptedReader::new([
        ReadStep::Bytes(b"first".to_vec()),
        ReadStep::WouldBlock,
        ReadStep::Bytes(b"-second".to_vec()),
        ReadStep::Eof,
    ]);
    let mut read = NonBlockingReadAccumulator::default();

    assert_eq!(
        read.read_available(&mut reader, 64).unwrap(),
        NonBlockingReadStatus::Pending
    );
    assert_eq!(
        read.read_available(&mut reader, 64).unwrap(),
        NonBlockingReadStatus::Complete(b"first-second".to_vec())
    );
}

#[test]
fn nonblocking_read_budget_bounds_each_dispatch_until_eof() {
    let bytes = (0_u8..12).collect::<Vec<_>>();
    let mut reader = Cursor::new(bytes.clone());
    let mut read = NonBlockingReadAccumulator::default();

    assert_eq!(
        read.read_available(&mut reader, 4).unwrap(),
        NonBlockingReadStatus::Pending
    );
    assert_eq!(
        read.read_available(&mut reader, 4).unwrap(),
        NonBlockingReadStatus::Pending
    );
    assert_eq!(
        read.read_available(&mut reader, 4).unwrap(),
        NonBlockingReadStatus::Pending
    );
    assert_eq!(
        read.read_available(&mut reader, 4).unwrap(),
        NonBlockingReadStatus::Complete(bytes)
    );
}
