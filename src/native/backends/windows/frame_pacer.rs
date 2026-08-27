// ============================================================================
// native/backends/windows/frame_pacer.rs — DWM-backed one-shot frame pacing
//
// DwmFlush is deliberately called on a lazy worker thread: it may block until
// the next compositor present and therefore must never run in the UI thread.
// The worker posts an app-private message whose epoch is accepted only while
// it still names the exact outstanding NativeFrameRequest.
// ============================================================================

#![cfg(windows)]

use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use crate::core::error::{Errc, Error, Result};
use crate::diagnostics::PendingFailureSource;
use crate::platform::windowing::event::FrameRequestToken;
use crate::platform::windowing::window::{NativeFrameRequest, NativeFrameRequestPhase};

use super::consts::WM_UIX_FRAME_OPPORTUNITY;
use super::ffi::{DwmFlush, PostMessageW};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FramePacerTicket {
    pub(crate) request: NativeFrameRequest,
    pub(crate) epoch: u64,
}

#[derive(Debug, Default)]
pub(crate) struct WindowsFramePacerState {
    pending: Option<FramePacerTicket>,
    submitted_epoch: Option<u64>,
    next_epoch: u64,
}

impl WindowsFramePacerState {
    /// Arms one exact request. Re-arming the same request is idempotent; a
    /// different request replaces the old ticket so its eventual callback is
    /// necessarily stale.
    pub(crate) fn arm(&mut self, request: NativeFrameRequest) -> (FramePacerTicket, bool) {
        if let Some(ticket) = self.pending {
            if ticket.request == request {
                return (ticket, false);
            }
        }

        self.next_epoch = self.next_epoch.wrapping_add(1);
        if self.next_epoch == 0 {
            self.next_epoch = 1;
        }
        let ticket = FramePacerTicket {
            request,
            epoch: self.next_epoch,
        };
        self.pending = Some(ticket);
        self.submitted_epoch = None;
        (ticket, true)
    }

    pub(crate) fn has_unsubmitted(&self, token: FrameRequestToken) -> bool {
        self.pending.is_some_and(|ticket| {
            ticket.request.token == token && self.submitted_epoch != Some(ticket.epoch)
        })
    }

    pub(crate) fn mark_submitted(&mut self, token: FrameRequestToken) -> Option<FramePacerTicket> {
        let ticket = self.pending.filter(|ticket| {
            ticket.request.token == token && self.submitted_epoch != Some(ticket.epoch)
        })?;
        self.submitted_epoch = Some(ticket.epoch);
        Some(ticket)
    }

    pub(crate) fn matches_submitted(&self, ticket: FramePacerTicket) -> bool {
        self.pending == Some(ticket) && self.submitted_epoch == Some(ticket.epoch)
    }

    pub(crate) fn complete(&mut self, epoch: u64) -> Option<NativeFrameRequest> {
        let ticket = self
            .pending
            .filter(|ticket| ticket.epoch == epoch && self.submitted_epoch == Some(epoch))?;
        self.pending = None;
        self.submitted_epoch = None;
        Some(ticket.request)
    }

    pub(crate) fn discard(&mut self, ticket: FramePacerTicket) {
        if self.pending == Some(ticket) {
            self.pending = None;
            self.submitted_epoch = None;
        }
    }

    pub(crate) fn cancel(&mut self, token: FrameRequestToken) {
        if self
            .pending
            .is_some_and(|ticket| ticket.request.token == token)
        {
            self.pending = None;
            self.submitted_epoch = None;
        }
    }

    pub(crate) fn clear(&mut self) {
        self.pending = None;
        self.submitted_epoch = None;
    }
}

pub(crate) type SharedWindowsFramePacerState = Arc<Mutex<WindowsFramePacerState>>;

pub(crate) fn shared_frame_pacer_state() -> SharedWindowsFramePacerState {
    Arc::new(Mutex::new(WindowsFramePacerState::default()))
}

pub(crate) fn complete_posted_frame(
    state: &SharedWindowsFramePacerState,
    wparam: usize,
    lparam: isize,
) -> Option<NativeFrameRequest> {
    let epoch = epoch_from_message(wparam, lparam);
    state
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .complete(epoch)
}

pub(crate) fn clear_pending_frame(state: &SharedWindowsFramePacerState) {
    state
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clear();
}

enum FramePacerCommand {
    Wait(FramePacerTicket),
}

struct FramePacerWorker {
    sender: SyncSender<FramePacerCommand>,
    _join: JoinHandle<()>,
}

impl FramePacerWorker {
    fn spawn(
        hwnd: usize,
        state: SharedWindowsFramePacerState,
        pending_failures: PendingFailureSource,
    ) -> Result<Self> {
        // One command may wait behind the in-flight DwmFlush. Further
        // replacements fall back instead of growing an unbounded queue.
        let (sender, receiver) = mpsc::sync_channel(1);
        let join = thread::Builder::new()
            .name(format!("uix-dwm-frame-{hwnd:x}"))
            .spawn(move || {
                while let Ok(command) = receiver.recv() {
                    let FramePacerCommand::Wait(ticket) = command;

                    let still_pending = state
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .matches_submitted(ticket);
                    if !still_pending {
                        continue;
                    }

                    // SAFETY: DwmFlush 无参数，工作线程只同步等待 DWM 合成机会并检查 HRESULT。
                    let result = unsafe { DwmFlush() };
                    let still_pending = state
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .matches_submitted(ticket);
                    if result < 0 {
                        if still_pending {
                            state
                                .lock()
                                .unwrap_or_else(|error| error.into_inner())
                                .discard(ticket);
                            enqueue_worker_failure(
                                &pending_failures,
                                Error::new(Errc::PlatformError, format_dwm_flush_failure(result)),
                            );
                        }
                        // A stale ticket cannot report a failure against a
                        // replacement request. The owner source also closes
                        // during platform teardown, making late callbacks no-ops.
                        continue;
                    }
                    if !still_pending {
                        continue;
                    }

                    let (wparam, lparam) = epoch_to_message(ticket.epoch);
                    // SAFETY: 保存的整数只还原为不透明 HWND，消息参数不含指针；窗口失效时 PostMessageW 会安全失败。
                    let posted = unsafe {
                        PostMessageW(
                            hwnd as *mut std::ffi::c_void,
                            WM_UIX_FRAME_OPPORTUNITY,
                            wparam,
                            lparam,
                        )
                    };
                    if posted == 0 {
                        let error = super::util::windows_diag(
                            Errc::PlatformError,
                            "Windows DWM frame pacer: PostMessageW failed",
                        );
                        state
                            .lock()
                            .unwrap_or_else(|error| error.into_inner())
                            .discard(ticket);
                        enqueue_worker_failure(&pending_failures, error);
                    }
                }
            })
            .map_err(|error| {
                Error::new(
                    Errc::PlatformError,
                    format!("failed to start Windows DWM frame pacer: {error}"),
                )
            })?;
        Ok(Self {
            sender,
            _join: join,
        })
    }
}

pub(crate) struct WindowsFramePacer {
    hwnd: usize,
    state: SharedWindowsFramePacerState,
    pending_failures: PendingFailureSource,
    worker: Option<FramePacerWorker>,
}

impl WindowsFramePacer {
    pub(crate) fn new(
        hwnd: *mut std::ffi::c_void,
        state: SharedWindowsFramePacerState,
        pending_failures: PendingFailureSource,
    ) -> Self {
        Self {
            hwnd: hwnd as usize,
            state,
            pending_failures,
            worker: None,
        }
    }

    pub(crate) fn request(&mut self, request: NativeFrameRequest) -> Result<bool> {
        if request.phase != NativeFrameRequestPhase::AfterPresent {
            return Ok(false);
        }
        let _ = self
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .arm(request);
        Ok(true)
    }

    /// Releases the exact pre-armed request only after its frame committed.
    /// A request whose frame failed or remained idle is never submitted to
    /// DwmFlush and is eventually invalidated by cancellation/fallback.
    pub(crate) fn presented(&mut self, token: FrameRequestToken) -> Result<()> {
        let has_unsubmitted = self
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .has_unsubmitted(token);
        if !has_unsubmitted {
            return Ok(());
        }
        if self.worker.is_none() {
            self.worker = Some(FramePacerWorker::spawn(
                self.hwnd,
                Arc::clone(&self.state),
                self.pending_failures.clone(),
            )?);
        }

        let Some(ticket) = self
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .mark_submitted(token)
        else {
            return Ok(());
        };
        let Some(worker) = self.worker.as_ref() else {
            return Err(Error::new(
                Errc::InvalidState,
                "Windows DWM frame pacer worker is unavailable",
            ));
        };
        match worker.sender.try_send(FramePacerCommand::Wait(ticket)) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => {
                self.state
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .discard(ticket);
                Err(Error::new(
                    Errc::WouldBlock,
                    "Windows DWM frame pacer queue is full; fallback remains armed",
                ))
            }
            Err(TrySendError::Disconnected(_)) => {
                self.state
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .discard(ticket);
                self.worker = None;
                Err(Error::new(
                    Errc::PlatformError,
                    "Windows DWM frame pacer worker stopped unexpectedly",
                ))
            }
        }
    }

    pub(crate) fn cancel(&mut self, token: FrameRequestToken) {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .cancel(token);
    }
}

fn enqueue_worker_failure(pending_failures: &PendingFailureSource, error: Error) {
    let _ = pending_failures.enqueue(error);
}

fn format_dwm_flush_failure(result: i32) -> String {
    format!(
        "Windows DWM frame pacer: DwmFlush failed with HRESULT 0x{:08x}",
        result as u32
    )
}

impl Drop for WindowsFramePacer {
    fn drop(&mut self) {
        clear_pending_frame(&self.state);
        // Dropping the sender disconnects the bounded channel. The detached
        // worker skips queued stale tickets and exits after any in-flight
        // DwmFlush; joining here could block the UI thread for that interval.
        self.worker = None;
    }
}

#[cfg(target_pointer_width = "64")]
pub(crate) fn epoch_to_message(epoch: u64) -> (usize, isize) {
    (epoch as usize, 0)
}

#[cfg(target_pointer_width = "64")]
pub(crate) fn epoch_from_message(wparam: usize, _lparam: isize) -> u64 {
    wparam as u64
}

#[cfg(target_pointer_width = "32")]
pub(crate) fn epoch_to_message(epoch: u64) -> (usize, isize) {
    let low = epoch as u32 as usize;
    let high = ((epoch >> 32) as u32 as i32) as isize;
    (low, high)
}

#[cfg(target_pointer_width = "32")]
pub(crate) fn epoch_from_message(wparam: usize, lparam: isize) -> u64 {
    u64::from(wparam as u32) | (u64::from(lparam as u32) << 32)
}
