    use super::*;
    use crate::app::queues::agent_command_queue::AgentCommandResult;
    use crate::ui::accessibility::semantic_snapshot::SemanticTarget;
    use crate::ui::semantic_action::SemanticAction;
    use std::sync::mpsc::{self, RecvTimeoutError};
    use std::time::Duration;

    fn pending(confirm_id: u64, created_at: Instant) -> PendingConfirmation {
        PendingConfirmation {
            confirm_id,
            generation: 1,
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
