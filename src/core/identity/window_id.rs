/// Stable application/window identifier shared across native and app layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(u64);

impl WindowId {
    /// 应用根窗口保留的稳定身份。
    pub const ROOT: Self = Self(0);

    /// 由调用方提供的原始整数创建窗口身份。
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// 返回应用根窗口的保留身份。
    pub const fn root() -> Self {
        Self::ROOT
    }

    /// 返回该窗口身份的原始整数表示。
    pub const fn raw(self) -> u64 {
        self.0
    }
}
