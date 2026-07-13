//! Bounded graphics recovery state machine.
//!
//! Recovery is decided at frame boundaries from a typed [`GraphicsFailure`].
//! It never performs a hot probe during a frame: callers execute the returned
//! action and report the next failure back to this state machine.

use super::GraphicsFailure;

/// The next permitted recovery action after a failed graphics frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    RebuildSurface,
    RebuildRecipe,
    TryNextRecipe,
    UseSoftware,
    AbortOutOfMemory,
    Abort,
}

/// Per-window bounded recovery progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphicsRecovery {
    step: u8,
    software_available: bool,
}

impl GraphicsRecovery {
    pub const fn new(software_available: bool) -> Self {
        Self {
            step: 0,
            software_available,
        }
    }

    /// Maps a typed failure to the next action in the required, finite order:
    /// surface rebuild -> same recipe rebuild -> next recipe -> Software.
    /// Out-of-memory is terminal and must never be silently downgraded.
    pub fn on_failure(&mut self, failure: &GraphicsFailure) -> RecoveryAction {
        if matches!(failure, GraphicsFailure::OutOfMemory(_)) {
            return RecoveryAction::AbortOutOfMemory;
        }
        let action = match self.step {
            0 => RecoveryAction::RebuildSurface,
            1 => RecoveryAction::RebuildRecipe,
            2 => RecoveryAction::TryNextRecipe,
            _ if self.software_available => RecoveryAction::UseSoftware,
            _ => RecoveryAction::Abort,
        };
        self.step = self.step.saturating_add(1);
        action
    }

    /// A successful present closes the failure episode and resets the bounded
    /// sequence for a future, independent surface/device loss.
    pub fn on_presented(&mut self) {
        self.step = 0;
    }
}

