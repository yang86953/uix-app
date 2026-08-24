use super::*;

#[test]
fn dropping_ticket_cancels_pending_command() {
    let queue = AgentCommandQueue::new();
    let (ticket, _) = queue
        .submit(AgentCommandRequest::Snapshot)
        .expect("命令应成功入队");
    drop(ticket);

    let envelope = queue.pop_front().expect("队列应保留待清理命令");
    assert!(!envelope.try_start());
}

#[test]
fn started_command_cannot_be_reported_as_cancelled() {
    let queue = AgentCommandQueue::new();
    let (ticket, _) = queue
        .submit(AgentCommandRequest::Snapshot)
        .expect("命令应成功入队");
    let envelope = queue.pop_front().expect("命令应可供 UI 消费");

    assert!(envelope.try_start());
    assert!(!ticket.cancel_pending());
}
