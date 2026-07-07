/// Generational component/node identifier shared by UI, app bridges, and draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentId {
    slot: usize,
    generation: u32,
}

impl ComponentId {
    pub const fn new(slot: usize) -> Self {
        Self {
            slot,
            generation: 0,
        }
    }

    pub const fn from_parts(slot: usize, generation: u32) -> Self {
        Self { slot, generation }
    }

    pub const fn slot(self) -> usize {
        self.slot
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }
}

impl Default for ComponentId {
    fn default() -> Self {
        Self::new(0)
    }
}

impl std::fmt::Display for ComponentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.slot, self.generation)
    }
}
