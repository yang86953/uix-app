// ============================================================================
// native/shared/input_serial.rs — 有输入事件来源的协议 serial
// ============================================================================

/// 尚未收到输入事件时没有 serial；不得用常量零伪造协议授权。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InputSerial {
    latest: Option<u32>,
}

impl InputSerial {
    pub(crate) fn record(&mut self, serial: u32) {
        self.latest = Some(serial);
    }

    pub(crate) fn latest(self) -> Option<u32> {
        self.latest
    }
}
