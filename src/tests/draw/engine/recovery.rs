use crate::draw::engine::recovery::*;
use crate::draw::engine::GraphicsFailure;
use crate::tests::common::*;

fn surface_lost() -> GraphicsFailure {
    GraphicsFailure::SurfaceLost(Error::new(Errc::GraphicsSurfaceLost, "test surface lost"))
}

#[test]
fn recoverable_failures_follow_the_bounded_contract_order() {
    let mut recovery = GraphicsRecovery::new(true);
    assert_eq!(
        recovery.on_failure(&surface_lost()),
        RecoveryAction::RebuildSurface
    );
    assert_eq!(
        recovery.on_failure(&surface_lost()),
        RecoveryAction::RebuildRecipe
    );
    assert_eq!(
        recovery.on_failure(&surface_lost()),
        RecoveryAction::TryNextRecipe
    );
    assert_eq!(
        recovery.on_failure(&surface_lost()),
        RecoveryAction::UseSoftware
    );
    assert_eq!(recovery.on_failure(&surface_lost()), RecoveryAction::Abort);
}

#[test]
fn out_of_memory_never_silently_falls_back_to_software() {
    let mut recovery = GraphicsRecovery::new(true);
    let oom = GraphicsFailure::OutOfMemory(Error::new(
        Errc::GraphicsOutOfMemory,
        "test allocation failure",
    ));
    assert_eq!(recovery.on_failure(&oom), RecoveryAction::AbortOutOfMemory);
    assert_eq!(
        recovery.on_failure(&surface_lost()),
        RecoveryAction::RebuildSurface
    );
}

#[test]
fn successful_present_resets_a_previous_failure_episode() {
    let mut recovery = GraphicsRecovery::new(false);
    assert_eq!(
        recovery.on_failure(&surface_lost()),
        RecoveryAction::RebuildSurface
    );
    recovery.on_presented();
    assert_eq!(
        recovery.on_failure(&surface_lost()),
        RecoveryAction::RebuildSurface
    );
}
