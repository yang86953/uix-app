use crate::native::shared::native_frame_mailbox::{NativeFrameArmResult, NativeFrameMailbox};
use crate::native::traits::event::FrameRequestToken;
use crate::native::traits::window::NativeFrameRequest;

fn request(generation: u64, request_id: u64) -> NativeFrameRequest {
    NativeFrameRequest::after_present(FrameRequestToken::new(generation, request_id))
}

#[test]
fn native_frame_mailbox_requires_exact_present_and_completes_once() {
    let mut mailbox = NativeFrameMailbox::default();
    let expected = request(3, 8);
    let other = request(3, 9);

    assert_eq!(
        mailbox.arm(expected),
        NativeFrameArmResult::Armed {
            replaced_submitted: false
        }
    );
    assert!(!mailbox.mark_submitted(other.token));
    assert!(mailbox.take_submitted().is_none());
    assert!(mailbox.mark_submitted(expected.token));
    assert_eq!(mailbox.take_submitted(), Some(expected));
    assert!(mailbox.take_submitted().is_none());
}

#[test]
fn native_frame_mailbox_replacement_reports_stale_native_source() {
    let mut mailbox = NativeFrameMailbox::default();
    let old = request(5, 13);
    let new = request(5, 14);

    assert!(matches!(
        mailbox.arm(old),
        NativeFrameArmResult::Armed { .. }
    ));
    assert_eq!(mailbox.arm(old), NativeFrameArmResult::Duplicate);
    assert!(mailbox.mark_submitted(old.token));
    assert_eq!(
        mailbox.arm(new),
        NativeFrameArmResult::Armed {
            replaced_submitted: true
        }
    );
    assert!(!mailbox.cancel(old.token));
    assert!(mailbox.mark_submitted(new.token));
    assert_eq!(mailbox.take_submitted(), Some(new));
}

#[test]
fn native_frame_mailbox_cancel_is_exact_and_reports_submission() {
    let mut mailbox = NativeFrameMailbox::default();
    let expected = request(7, 21);
    let other = request(7, 22);

    mailbox.arm(expected);
    assert!(!mailbox.cancel(other.token));
    assert!(mailbox.mark_submitted(expected.token));
    assert!(mailbox.cancel(expected.token));
    assert!(mailbox.take_submitted().is_none());
    assert!(!mailbox.clear());
}
