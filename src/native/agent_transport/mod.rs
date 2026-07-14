//! Platform-local byte streams for the opt-in Agent Bridge.
//!
//! Authentication and JSON framing stay in `app`; this module owns only OS
//! endpoint creation, same-user admission, secure discovery publication, and
//! cryptographic random bytes.

use std::io::{Read, Write};
use std::sync::Arc;

pub(crate) trait AgentStreamIo: Read + Write + Send {}

impl<T> AgentStreamIo for T where T: Read + Write + Send {}

pub(crate) type AgentStream = Box<dyn AgentStreamIo>;

pub(crate) trait AgentStreamCancelIo: Send + Sync {
    fn bind_worker(&self, _worker: &std::thread::JoinHandle<()>) {}

    fn cancel(&self);
}

pub(crate) struct AcceptedAgentStream {
    pub(crate) stream: AgentStream,
    pub(crate) cancel: Arc<dyn AgentStreamCancelIo>,
}

#[cfg(test)]
pub(crate) fn connect_for_test(endpoint: &str) -> std::io::Result<AgentStream> {
    platform_connect_for_test(endpoint)
}

#[cfg(test)]
pub(crate) fn discovery_permissions_are_private_for_test(
    path: &std::path::Path,
) -> std::io::Result<Option<bool>> {
    platform_discovery_permissions_are_private_for_test(path)
}

#[cfg(unix)]
mod unix;
#[cfg(all(test, unix))]
use unix::{
    connect_for_test as platform_connect_for_test,
    discovery_permissions_are_private_for_test as platform_discovery_permissions_are_private_for_test,
};
#[cfg(unix)]
pub(crate) use unix::{fill_secure_random, AgentEndpoint, AgentEndpointWake};

#[cfg(windows)]
mod windows;
#[cfg(all(test, windows))]
use windows::{
    connect_for_test as platform_connect_for_test,
    discovery_permissions_are_private_for_test as platform_discovery_permissions_are_private_for_test,
};
#[cfg(windows)]
pub(crate) use windows::{fill_secure_random, AgentEndpoint, AgentEndpointWake};

#[cfg(not(any(unix, windows)))]
mod unsupported;
#[cfg(all(test, not(any(unix, windows))))]
use unsupported::{
    connect_for_test as platform_connect_for_test,
    discovery_permissions_are_private_for_test as platform_discovery_permissions_are_private_for_test,
};
#[cfg(not(any(unix, windows)))]
pub(crate) use unsupported::{fill_secure_random, AgentEndpoint, AgentEndpointWake};
