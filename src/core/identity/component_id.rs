/// Generational component/node identifier shared by UI, app bridges, and draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentId {
    tree_scope: u64,
    slot: usize,
    generation: u32,
}

impl ComponentId {
    pub const fn new(slot: usize) -> Self {
        Self {
            tree_scope: 0,
            slot,
            generation: 0,
        }
    }

    pub const fn from_parts(slot: usize, generation: u32) -> Self {
        Self {
            tree_scope: 0,
            slot,
            generation,
        }
    }

    pub(crate) const fn from_scoped_parts(tree_scope: u64, slot: usize, generation: u32) -> Self {
        Self {
            tree_scope,
            slot,
            generation,
        }
    }

    pub const fn slot(self) -> usize {
        self.slot
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }

    pub(crate) const fn tree_scope(self) -> u64 {
        self.tree_scope
    }
}

impl Default for ComponentId {
    fn default() -> Self {
        Self::new(0)
    }
}

impl std::fmt::Display for ComponentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.tree_scope == 0 {
            write!(f, "{}:{}", self.slot, self.generation)
        } else {
            write!(f, "{}:{}:{}", self.tree_scope, self.slot, self.generation)
        }
    }
}
