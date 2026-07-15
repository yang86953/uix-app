use crate::native::graphics::vulkan::platform::context::PresentCompletion;
use crate::tests::common::*;

#[test]
fn acquire_proof_releases_only_after_a_presented_image_is_reacquired_and_submitted() {
    let mut completion = PresentCompletion::new(2).expect("presentation history");

    assert_eq!(
        completion
            .release_count_for_acquire(0, 3)
            .expect("first acquire"),
        0
    );
    completion.mark_presented(0).expect("first present");
    let release_count = completion
        .release_count_for_acquire(0, 3)
        .expect("reacquire presented image");
    assert_eq!(release_count, 3);
    assert_eq!(completion.completed_submission_count(), 0);

    completion.on_submission_queued(release_count);
    assert_eq!(completion.completed_submission_count(), 3);
    assert_eq!(completion.completed_submission_count(), 0);
}

#[test]
fn pending_acquire_proof_survives_a_generation_change() {
    let mut completion = PresentCompletion::new(1).expect("presentation history");
    completion.mark_presented(0).expect("present");
    let release_count = completion
        .release_count_for_acquire(0, 2)
        .expect("reacquire");
    completion.on_submission_queued(release_count);

    completion.replace_generation(vec![false; 3]);
    assert_eq!(completion.completed_submission_count(), 2);
    assert_eq!(
        completion
            .release_count_for_acquire(0, 4)
            .expect("new generation first acquire"),
        0
    );
}

#[test]
fn presentation_history_rejects_unknown_image_slots() {
    let mut completion = PresentCompletion::new(1).expect("presentation history");
    let acquire = completion
        .release_count_for_acquire(1, 0)
        .expect_err("unknown acquire slot");
    assert_eq!(acquire.code(), Errc::InvalidState);

    let present = completion
        .mark_presented(1)
        .expect_err("unknown present slot");
    assert_eq!(present.code(), Errc::InvalidState);
}
