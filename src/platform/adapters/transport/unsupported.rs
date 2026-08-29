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

#[cfg(test)]
pub(super) fn discovery_permissions_are_private_for_test(_path: &Path) -> io::Result<Option<bool>> {
    Ok(None)
}

#[cfg(test)]
pub(super) fn endpoint_permissions_are_private_for_test(
    _endpoint: &str,
) -> io::Result<Option<bool>> {
    Ok(None)
}
