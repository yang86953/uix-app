// 各项自带 cfg(test)，在源模块作用域 include! 展开。

impl AppRuntime {

    // 测试目标保留 transport 信息观测入口，供带 transport 的 GUI 契约测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(all(feature = "agent-control", test))]
    pub(crate) fn agent_transport_info(&self) -> Option<AgentTransportInfo> {
        self.agent_transport
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
            .map(|transport| transport.info().clone())
    }
}
