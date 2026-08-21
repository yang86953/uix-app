use super::*;

const INITIAL: RhiExtent = RhiExtent::new(640, 480);

#[test]
fn lifecycle_distinguishes_retry_from_presented_recreation() {
    let mut lifecycle = RhiSurfaceLifecycle::uninitialized(INITIAL);
    let initial = lifecycle
        .begin_recreate(INITIAL, RhiSurfaceRecreateReason::Initialize)
        .expect("initial transaction");
    assert!(matches!(
        lifecycle.commit_recreate(initial, INITIAL),
        Ok(RhiSurfaceRecreateCommit::Ready(token)) if token.generation == 0
    ));

    let resized_extent = RhiExtent::new(800, 600);
    let resize = lifecycle
        .begin_recreate(resized_extent, RhiSurfaceRecreateReason::Resize)
        .expect("resize transaction");
    let resize_commit = lifecycle
        .commit_recreate(resize, resized_extent)
        .expect("resize commit");
    assert_eq!(resize_commit.token(), SurfaceToken::new(1, resized_extent));

    let acquire = lifecycle
        .begin_recreate(
            resized_extent,
            RhiSurfaceRecreateReason::AcquisitionRejected,
        )
        .expect("acquire invalidation transaction");
    let retry = lifecycle
        .commit_recreate(acquire, resized_extent)
        .expect("acquire invalidation commit");
    let retry_error = retry
        .complete_frame(Error::new(
            Errc::GraphicsSurfaceLost,
            "adapter acquire status",
        ))
        .expect_err("unpresented frame must retry");
    assert_eq!(retry_error.code(), Errc::GraphicsSurfaceChanged);
    assert_eq!(retry.token().generation, 2);

    let suboptimal = lifecycle
        .begin_recreate(
            resized_extent,
            RhiSurfaceRecreateReason::PresentedNeedsRecreate,
        )
        .expect("presented recreation transaction");
    let presented = lifecycle
        .commit_recreate(suboptimal, resized_extent)
        .expect("presented recreation commit");
    assert!(matches!(presented, RhiSurfaceRecreateCommit::Presented(_)));
    presented
        .complete_frame(Error::new(
            Errc::GraphicsSurfaceLost,
            "adapter suboptimal status",
        ))
        .expect("already presented frame must remain successful");
    assert_eq!(lifecycle.token().generation, 3);
}

#[test]
fn failed_recreation_keeps_old_token_and_requires_a_new_transaction() {
    let mut lifecycle = RhiSurfaceLifecycle::uninitialized(INITIAL);
    let initial = lifecycle
        .begin_recreate(INITIAL, RhiSurfaceRecreateReason::Initialize)
        .expect("initial transaction");
    lifecycle
        .commit_recreate(initial, INITIAL)
        .expect("initial commit");
    let previous = lifecycle.token();

    let failed = lifecycle
        .begin_recreate(INITIAL, RhiSurfaceRecreateReason::PresentationRejected)
        .expect("failed transaction");
    lifecycle
        .abort_recreate(failed)
        .expect("failed transaction rollback");
    assert_eq!(lifecycle.token(), previous);

    let retry = lifecycle
        .begin_recreate(INITIAL, RhiSurfaceRecreateReason::PresentationRejected)
        .expect("retry transaction");
    assert!(matches!(
        lifecycle.commit_recreate(retry, INITIAL),
        Ok(RhiSurfaceRecreateCommit::RetryFrame(token)) if token.generation == 1
    ));
}

#[test]
fn recreate_rejects_zero_extent_before_entering_native_api() {
    let mut lifecycle = RhiSurfaceLifecycle::uninitialized(INITIAL);
    let error = lifecycle
        .begin_recreate(
            RhiExtent::new(0, INITIAL.height),
            RhiSurfaceRecreateReason::Initialize,
        )
        .expect_err("zero extent must be paused above the native boundary");
    assert_eq!(error.code(), Errc::InvalidArgument);
}
