use std::io;
use std::path::Path;

use super::AcceptedAgentStream;

pub(crate) struct AgentEndpoint;

#[derive(Clone)]
pub(crate) struct AgentEndpointWake;

impl AgentEndpoint {
    pub(crate) fn bind(_process_id: u32, _nonce: &str) -> io::Result<Self> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "agent transport is unsupported on this platform",
        ))
    }

    pub(crate) fn endpoint_name(&self) -> String {
        String::new()
    }

    pub(crate) fn discovery_path(&self) -> &Path {
        Path::new("")
    }

    pub(crate) fn waker(&self) -> AgentEndpointWake {
        AgentEndpointWake
    }

    pub(crate) fn publish_discovery(&mut self, _contents: &[u8], _nonce: &str) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "agent transport is unsupported on this platform",
        ))
    }

    pub(crate) fn accept(&mut self) -> io::Result<AcceptedAgentStream> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "agent transport is unsupported on this platform",
        ))
    }
}

impl AgentEndpointWake {
    pub(crate) fn wake(&self) {}
}

pub(crate) fn agent_hub_endpoint_name() -> io::Result<String> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "agent hub transport is unsupported on this platform",
    ))
}

pub(crate) fn connect(_endpoint: &str) -> io::Result<super::AgentStream> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "agent hub transport is unsupported on this platform",
    ))
}

pub(crate) fn fill_secure_random(_output: &mut [u8]) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "secure random source is unsupported on this platform",
    ))
}

// 平台不支持时的 owner-only 验收占位探针位于 tests-src，仅测试构建编译。
#[cfg(test)]
#[path = "../../../../tests-src/platform/adapters/transport/unsupported_tests.rs"]
mod tests;
