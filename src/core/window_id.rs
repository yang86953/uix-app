/// Stable application/window identifier shared across native and app layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(u64);

impl WindowId {
    pub const ROOT: Self = Self(0);

    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn root() -> Self {
        Self::ROOT
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}
