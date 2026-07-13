//! Exact one-shot request state for native display callbacks that are released
//! only after the associated frame commits.

use crate::native::traits::event::FrameRequestToken;
use crate::native::traits::window::NativeFrameRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeFrameArmResult {
    Duplicate,
    Armed { replaced_submitted: bool },
}

#[derive(Debug, Default)]
pub(crate) struct NativeFrameMailbox {
    pending: Option<NativeFrameRequest>,
    submitted: bool,
}

impl NativeFrameMailbox {
    /// Arms `request`, replacing a different pending request. The caller must
    /// invalidate its native callback source when a submitted request was
    /// replaced, so that source cannot complete the new token.
    pub(crate) fn arm(&mut self, request: NativeFrameRequest) -> NativeFrameArmResult {
        if self.pending == Some(request) {
            return NativeFrameArmResult::Duplicate;
        }
        let replaced_submitted = self.submitted;
        self.pending = Some(request);
        self.submitted = false;
        NativeFrameArmResult::Armed { replaced_submitted }
    }

    pub(crate) fn mark_submitted(&mut self, token: FrameRequestToken) -> bool {
        if self.submitted || !self.pending.is_some_and(|request| request.token == token) {
            return false;
        }
        self.submitted = true;
        true
    }

    pub(crate) fn take_submitted(&mut self) -> Option<NativeFrameRequest> {
        if !self.submitted {
            return None;
        }
        self.submitted = false;
        self.pending.take()
    }

    /// Cancels only the exact token and reports whether its native source had
    /// already been released after present.
    pub(crate) fn cancel(&mut self, token: FrameRequestToken) -> bool {
        if !self.pending.is_some_and(|request| request.token == token) {
            return false;
        }
        let was_submitted = self.submitted;
        self.pending = None;
        self.submitted = false;
        was_submitted
    }

    pub(crate) fn clear(&mut self) -> bool {
        let was_submitted = self.submitted;
        self.pending = None;
        self.submitted = false;
        was_submitted
    }
}
