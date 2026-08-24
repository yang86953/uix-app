use super::*;
use crate::app::queues::agent_command_queue::AgentCommandResult;
use crate::ui::accessibility::semantic_snapshot::SemanticTarget;
use crate::ui::semantic_action::SemanticAction;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn pending(confirm_id: u64, created_at: Instant) -> PendingConfirmation {
    PendingConfirmation {
        confirm_id,
        generation: 1,
        expected_revision: Some(11),
        target: SemanticTarget::AutomationId("danger-button".to_owned()),
        action: SemanticAction::Invoke,
        created_at,
        response: None,
    }
}

fn recv_result(
    rx: mpsc::Receiver<AgentCommandResult>,
) -> Result<AgentCommandResult, RecvTimeoutError> {
    rx.recv_timeout(Duration::from_millis(100))
}

#[test]
fn confirm_ids_increment() {
    let mut state = WindowAgentState::new();
    assert_eq!(state.next_confirm_id(), 1);
    assert_eq!(state.next_confirm_id(), 2);
}

#[test]
fn expire_confirmations_completes_pending_ticket() {
    let mut state = WindowAgentState::new();
    let (tx, rx) = mpsc::sync_channel(1);
    state
        .confirm_pending
        .push(pending(7, Instant::now() - Duration::from_secs(120)));
    state.confirm_pending[0].response = Some(tx);
    state.expire_confirmations();
    assert!(state.confirm_pending.is_empty());
    match recv_result(rx) {
        Ok(Err(AgentCommandError::ConfirmationNotFound { confirm_id })) => {
            assert_eq!(confirm_id, 7);
        }
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn expire_confirmations_keeps_fresh_pending() {
    let mut state = WindowAgentState::new();
    state.confirm_pending.push(pending(7, Instant::now()));
    state.expire_confirmations();
    assert_eq!(state.confirm_pending.len(), 1);
}

#[test]
fn expire_confirmations_compacts_mixed_batch_in_order() {
    let mut state = WindowAgentState::new();
    let (tx1, rx1) = mpsc::sync_channel(1);
    let (tx3, rx3) = mpsc::sync_channel(1);
    let expired_at = Instant::now() - Duration::from_secs(120);
    state.confirm_pending.push(pending(1, expired_at));
    state.confirm_pending[0].response = Some(tx1);
    state.confirm_pending.push(pending(2, Instant::now()));
    state.confirm_pending.push(pending(3, expired_at));
    state.confirm_pending[2].response = Some(tx3);

    state.expire_confirmations();

    assert_eq!(state.confirm_pending.len(), 1);
    assert_eq!(state.confirm_pending[0].confirm_id, 2);
    for (rx, expected_id) in [(rx1, 1), (rx3, 3)] {
        match recv_result(rx) {
            Ok(Err(AgentCommandError::ConfirmationNotFound { confirm_id })) => {
                assert_eq!(confirm_id, expected_id);
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }
}

#[test]
fn fail_confirmations_completes_all_pending() {
    let mut state = WindowAgentState::new();
    let (tx1, rx1) = mpsc::sync_channel(1);
    let (tx2, rx2) = mpsc::sync_channel(1);
    state.confirm_pending.push(pending(1, Instant::now()));
    state.confirm_pending[0].response = Some(tx1);
    state.confirm_pending.push(pending(2, Instant::now()));
    state.confirm_pending[1].response = Some(tx2);
    state.fail_confirmations();
    assert!(state.confirm_pending.is_empty());
    assert!(recv_result(rx1).is_ok());
    assert!(recv_result(rx2).is_ok());
}

#[derive(Default)]
struct RecordingExecutor {
    expected_revision: Mutex<Option<Option<u64>>>,
}

impl AgentCommandExecutor for RecordingExecutor {
    fn perform(
        &self,
        _tree: &mut WidgetTree,
        _semantic_state: &WindowSemanticState,
        _presentable: bool,
        _generation: u64,
        expected_revision: Option<u64>,
        _target: &SemanticTarget,
        _action: &SemanticAction,
    ) -> Result<(), AgentCommandError> {
        *self
            .expected_revision
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(expected_revision);
        Err(AgentCommandError::NotPresentable)
    }

    fn perform_window(
        &self,
        _tree: &mut WidgetTree,
        _semantic_state: &WindowSemanticState,
        _presentable: bool,
        _generation: u64,
        _expected_revision: Option<u64>,
        _window: &mut dyn AgentWindowOps,
        _action: AgentWindowAction,
    ) -> Result<(), AgentCommandError> {
        unreachable!("本测试不执行窗口动作")
    }
}

#[test]
fn resolve_confirmation_preserves_expected_revision() {
    let mut state = WindowAgentState::new();
    let executor = Arc::new(RecordingExecutor::default());
    state.set_executor(executor.clone());
    state.confirm_pending.push(pending(7, Instant::now()));

    let mut tree = WidgetTree::new();
    let semantic_state = WindowSemanticState::new(crate::core::WindowId::ROOT);
    let (tx, rx) = mpsc::sync_channel(1);
    state.handle_resolve_confirmation(&mut tree, &semantic_state, true, tx, 7, true);

    assert_eq!(
        *executor
            .expected_revision
            .lock()
            .unwrap_or_else(|error| error.into_inner()),
        Some(Some(11))
    );
    assert_eq!(recv_result(rx), Ok(Err(AgentCommandError::NotPresentable)));
}

#[test]
fn resolve_confirmation_rejects_expired_pending() {
    let mut state = WindowAgentState::new();
    let executor = Arc::new(RecordingExecutor::default());
    state.set_executor(executor.clone());
    state
        .confirm_pending
        .push(pending(7, Instant::now() - Duration::from_secs(120)));

    let mut tree = WidgetTree::new();
    let semantic_state = WindowSemanticState::new(crate::core::WindowId::ROOT);
    let (tx, rx) = mpsc::sync_channel(1);
    state.handle_resolve_confirmation(&mut tree, &semantic_state, true, tx, 7, true);

    assert_eq!(
        *executor
            .expected_revision
            .lock()
            .unwrap_or_else(|error| error.into_inner()),
        None
    );
    assert_eq!(
        recv_result(rx),
        Ok(Err(AgentCommandError::ConfirmationNotFound {
            confirm_id: 7
        }))
    );
}
