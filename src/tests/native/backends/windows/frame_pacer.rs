use crate::native::backends::windows::frame_pacer::{
    epoch_from_message, epoch_to_message, WindowsFramePacerState,
};
use crate::native::traits::event::FrameRequestToken;
use crate::native::traits::window::NativeFrameRequest;

fn request(generation: u64, request_id: u64) -> NativeFrameRequest {
    NativeFrameRequest::after_present(FrameRequestToken::new(generation, request_id))
}

#[test]
fn windows_frame_pacer_completes_only_the_exact_epoch_once() {
    let mut state = WindowsFramePacerState::default();
    let expected = request(7, 11);
    let (ticket, newly_armed) = state.arm(expected);

    assert!(newly_armed);
    assert!(state.complete(ticket.epoch).is_none());
    assert_eq!(state.mark_submitted(expected.token), Some(ticket));
    assert!(state.complete(ticket.epoch.wrapping_add(1)).is_none());
    assert_eq!(state.complete(ticket.epoch), Some(expected));
    assert!(state.complete(ticket.epoch).is_none());
}

#[test]
fn windows_frame_pacer_replacement_rejects_stale_completion_and_cancel() {
    let mut state = WindowsFramePacerState::default();
    let old_request = request(2, 19);
    let new_request = request(2, 20);
    let (old_ticket, old_was_new) = state.arm(old_request);
    let (same_ticket, duplicate_was_new) = state.arm(old_request);
    let (new_ticket, new_was_new) = state.arm(new_request);

    assert!(old_was_new);
    assert!(!duplicate_was_new);
    assert_eq!(same_ticket, old_ticket);
    assert!(new_was_new);
    assert_ne!(new_ticket.epoch, old_ticket.epoch);

    state.cancel(old_request.token);
    assert!(state.complete(old_ticket.epoch).is_none());
    assert_eq!(state.mark_submitted(new_request.token), Some(new_ticket));
    assert_eq!(state.complete(new_ticket.epoch), Some(new_request));
}

#[test]
fn windows_frame_pacer_releases_only_the_presented_token() {
    let mut state = WindowsFramePacerState::default();
    let expected = request(4, 33);
    let other = request(4, 34);
    let (ticket, _) = state.arm(expected);

    assert!(state.mark_submitted(other.token).is_none());
    assert!(state.complete(ticket.epoch).is_none());
    assert_eq!(state.mark_submitted(expected.token), Some(ticket));
    assert_eq!(state.complete(ticket.epoch), Some(expected));
}

#[test]
fn windows_frame_pacer_message_epoch_round_trips_at_pointer_width() {
    let epoch = 0xfedc_ba98_7654_3210;
    let (wparam, lparam) = epoch_to_message(epoch);

    assert_eq!(epoch_from_message(wparam, lparam), epoch);
}
