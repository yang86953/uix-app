//! `app/agent/agent_transport.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

struct NoopCancel;

impl AgentStreamCancelIo for NoopCancel {
    fn cancel(&self) {}
}

#[test]
fn terminal_reply_drain_is_immediate_when_empty_and_bounded_when_busy() {
    let connections: ConnectionRegistry = Arc::new(Mutex::new(BTreeMap::new()));
    assert!(wait_for_connections_to_drain(&connections, Duration::ZERO));
    connections
        .lock()
        .expect("fixture registry must lock")
        .insert(1, Arc::new(NoopCancel));
    assert!(!wait_for_connections_to_drain(&connections, Duration::ZERO));
}
