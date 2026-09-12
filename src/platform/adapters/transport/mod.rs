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

#[cfg(any(unix, test))]
mod user_identity;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub(crate) use unix::{
    AgentEndpoint, AgentEndpointWake, agent_hub_endpoint_name, connect, fill_secure_random,
};
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::{
    AgentEndpoint, AgentEndpointWake, agent_hub_endpoint_name, connect, fill_secure_random,
};
#[cfg(not(any(unix, windows)))]
mod unsupported;
#[cfg(not(any(unix, windows)))]
pub(crate) use unsupported::{
    AgentEndpoint, AgentEndpointWake, agent_hub_endpoint_name, connect, fill_secure_random,
};
#[cfg(all(test, not(any(unix, windows))))]
use unsupported::{
    discovery_permissions_are_private_for_test as platform_discovery_permissions_are_private_for_test,
    endpoint_permissions_are_private_for_test as platform_endpoint_permissions_are_private_for_test,
};
