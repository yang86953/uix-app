use crate::core::WindowId;

/// Identity of one native text-input endpoint.
///
/// `native_id` is an opaque backend-owned identity (for example an NSView
/// address). Pairing it with `WindowId` prevents recycled native identities
/// from inheriting a previous window's IME ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NativeImeTarget {
    pub(crate) window_id: WindowId,
    pub(crate) native_id: usize,
}

impl NativeImeTarget {
    pub(crate) fn new(window_id: WindowId, native_id: usize) -> Self {
        Self {
            window_id,
            native_id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NativeImeSession {
    pub(crate) target: NativeImeTarget,
    pub(crate) generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NativeImeActivation {
    pub(crate) previous: Option<NativeImeSession>,
    pub(crate) current: NativeImeSession,
    pub(crate) changed: bool,
}

/// Process-level ownership state for a native IME service shared by windows.
///
/// Selection and activation are deliberately separate: a delayed blur may
/// select its old target in order to stop it, but `deactivate_selected` only
/// succeeds when that target still owns the active generation.
#[derive(Debug, Default)]
pub(crate) struct NativeImeOwner {
    selected: Option<NativeImeTarget>,
    active: Option<NativeImeSession>,
    generation: u64,
}

impl NativeImeOwner {
    pub(crate) fn select(&mut self, target: NativeImeTarget) {
        self.selected = Some(target);
    }

    pub(crate) fn active(&self) -> Option<NativeImeSession> {
        self.active
    }

    pub(crate) fn activate_selected(&mut self) -> Option<NativeImeActivation> {
        let target = self.selected?;
        if let Some(current) = self.active.filter(|session| session.target == target) {
            return Some(NativeImeActivation {
                previous: None,
                current,
                changed: false,
            });
        }

        let current = NativeImeSession {
            target,
            generation: self.next_generation(),
        };
        let previous = self.active.replace(current);
        Some(NativeImeActivation {
            previous,
            current,
            changed: true,
        })
    }

    pub(crate) fn deactivate_selected(&mut self) -> Option<NativeImeSession> {
        let selected = self.selected?;
        if self.active.map(|session| session.target) != Some(selected) {
            return None;
        }
        let active = self.active.take();
        self.next_generation();
        active
    }

    pub(crate) fn deactivate_active(&mut self) -> Option<NativeImeSession> {
        let active = self.active.take();
        if active.is_some() {
            self.next_generation();
        }
        active
    }

    pub(crate) fn fail_activation(&mut self, session: NativeImeSession) -> bool {
        if self.active != Some(session) {
            return false;
        }
        self.active = None;
        self.next_generation();
        true
    }

    pub(crate) fn forget_target(&mut self, target: NativeImeTarget) {
        if self.selected == Some(target) {
            self.selected = None;
        }
        if self.active.map(|session| session.target) == Some(target) {
            self.active = None;
            self.next_generation();
        }
    }

    pub(crate) fn matches(&self, target: NativeImeTarget, generation: u64) -> bool {
        self.active == Some(NativeImeSession { target, generation })
    }

    fn next_generation(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.generation = 1;
        }
        self.generation
    }
}

// cfg(test) 完整辅助实现位于 tests-src，仅测试构建编译。
#[cfg(test)]
#[path = "../../../../tests-src/native/windowing/shared/ime_owner_tests.rs"]
mod ime_owner_tests;