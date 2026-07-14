//! Process lifecycle for the authenticated native Agent Bridge endpoint.

use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use serde_json::json;

use crate::app::agent_bridge::AgentProcessBridge;
use crate::app::agent_protocol::{
    encode_session_token, framing_error_reply, AgentProtocolReply, AgentProtocolSession,
    AGENT_PROTOCOL_SCHEMA, MAX_AGENT_CONNECTIONS, MAX_AGENT_MESSAGE_BYTES,
};
use crate::native::agent_transport::{
    fill_secure_random, AcceptedAgentStream, AgentEndpoint, AgentEndpointWake, AgentStream,
    AgentStreamCancelIo,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentTransportInfo {
    pub(crate) endpoint: String,
    pub(crate) discovery_path: PathBuf,
}

#[derive(Debug)]
pub(crate) enum AgentTransportError {
    BridgeDisabled,
    Io {
        stage: &'static str,
        source: io::Error,
    },
    ListenerThread(io::Error),
}

impl fmt::Display for AgentTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BridgeDisabled => formatter.write_str("agent bridge core is not enabled"),
            Self::Io { stage, source } => {
                write!(formatter, "agent transport {stage} failed: {source}")
            }
            Self::ListenerThread(source) => {
                write!(formatter, "agent listener thread failed to start: {source}")
            }
        }
    }
}

impl std::error::Error for AgentTransportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BridgeDisabled => None,
            Self::Io { source, .. } | Self::ListenerThread(source) => Some(source),
        }
    }
}

pub(crate) struct AgentTransportHandle {
    info: AgentTransportInfo,
    shutdown: Arc<AtomicBool>,
    wake_listener: AgentEndpointWake,
    connections: ConnectionRegistry,
    listener_thread: Option<JoinHandle<()>>,
}

type ConnectionRegistry = Arc<Mutex<BTreeMap<u64, Arc<dyn AgentStreamCancelIo>>>>;

impl AgentTransportHandle {
    pub(crate) fn start(bridge: AgentProcessBridge) -> Result<Self, AgentTransportError> {
        let mut token = [0u8; 32];
        fill_secure_random(&mut token).map_err(|source| AgentTransportError::Io {
            stage: "random token generation",
            source,
        })?;
        let encoded_token = encode_session_token(&token);
        let nonce = &encoded_token[..24];
        let process_id = std::process::id();
        let mut endpoint =
            AgentEndpoint::bind(process_id, nonce).map_err(|source| AgentTransportError::Io {
                stage: "endpoint bind",
                source,
            })?;
        let info = AgentTransportInfo {
            endpoint: endpoint.endpoint_name(),
            discovery_path: endpoint.discovery_path().to_path_buf(),
        };
        let descriptor = serde_json::to_vec_pretty(&json!({
            "schema": AGENT_PROTOCOL_SCHEMA,
            "process_id": process_id,
            "endpoint": info.endpoint,
            "token": encoded_token,
            "state": "ready",
        }))
        .map_err(|source| AgentTransportError::Io {
            stage: "discovery serialization",
            source: io::Error::new(io::ErrorKind::InvalidData, source),
        })?;
        endpoint
            .publish_discovery(&descriptor, nonce)
            .map_err(|source| AgentTransportError::Io {
                stage: "discovery publication",
                source,
            })?;

        let shutdown = Arc::new(AtomicBool::new(false));
        let connections: ConnectionRegistry = Arc::new(Mutex::new(BTreeMap::new()));
        let wake_listener = endpoint.waker();
        let session_token = Arc::new(token);
        let listener_shutdown = shutdown.clone();
        let listener_connections = connections.clone();
        let listener_thread = thread::Builder::new()
            .name("uix-agent-listener".to_owned())
            .spawn(move || {
                listener_loop(
                    endpoint,
                    bridge,
                    session_token,
                    listener_shutdown,
                    listener_connections,
                )
            })
            .map_err(AgentTransportError::ListenerThread)?;

        crate::core::log::info_fn(format!(
            "agent bridge ready discovery={}",
            info.discovery_path.display()
        ));
        Ok(Self {
            info,
            shutdown,
            wake_listener,
            connections,
            listener_thread: Some(listener_thread),
        })
    }

    pub(crate) fn info(&self) -> &AgentTransportInfo {
        &self.info
    }

    pub(crate) fn shutdown(&mut self) {
        if !self.shutdown.swap(true, Ordering::AcqRel) {
            self.wake_listener.wake();
            cancel_connections(&self.connections);
        }
        if let Some(listener_thread) = self.listener_thread.take() {
            if listener_thread.join().is_err() {
                crate::core::log::error_fn("agent listener thread panicked during shutdown");
            }
        }
    }
}

impl Drop for AgentTransportHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn listener_loop(
    mut endpoint: AgentEndpoint,
    bridge: AgentProcessBridge,
    session_token: Arc<[u8; 32]>,
    shutdown: Arc<AtomicBool>,
    connections: ConnectionRegistry,
) {
    let next_connection_id = AtomicU64::new(1);
    let mut workers = Vec::new();
    while !shutdown.load(Ordering::Acquire) {
        let accepted = match endpoint.accept() {
            Ok(accepted) => accepted,
            Err(error) => {
                if !shutdown.load(Ordering::Acquire) {
                    crate::core::log::error_fn(format!(
                        "agent listener stopped after accept failure: {error}"
                    ));
                }
                break;
            }
        };
        if shutdown.load(Ordering::Acquire) {
            accepted.cancel.cancel();
            break;
        }

        let connection_id = next_connection_id.fetch_add(1, Ordering::Relaxed).max(1);
        let AcceptedAgentStream { stream, cancel } = accepted;
        {
            let mut active = connections
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if active.len() >= MAX_AGENT_CONNECTIONS {
                drop(active);
                cancel.cancel();
                continue;
            }
            active.insert(connection_id, cancel.clone());
        }

        let worker_bridge = bridge.clone();
        let worker_token = session_token.clone();
        let worker_shutdown = shutdown.clone();
        let worker_connections = connections.clone();
        match thread::Builder::new()
            .name(format!("uix-agent-connection-{connection_id}"))
            .spawn(move || {
                if let Err(error) = serve_connection(stream, worker_bridge, worker_token) {
                    if !worker_shutdown.load(Ordering::Acquire) {
                        crate::core::log::error_fn(format!(
                            "agent connection ended after I/O failure: {error}"
                        ));
                    }
                }
                worker_connections
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .remove(&connection_id);
            }) {
            Ok(worker) => {
                cancel.bind_worker(&worker);
                workers.push(worker);
            }
            Err(error) => {
                connections
                    .lock()
                    .unwrap_or_else(|lock_error| lock_error.into_inner())
                    .remove(&connection_id);
                crate::core::log::error_fn(format!(
                    "agent connection thread failed to start: {error}"
                ));
            }
        }
    }

    cancel_connections(&connections);
    for worker in workers {
        if worker.join().is_err() {
            crate::core::log::error_fn("agent connection thread panicked during shutdown");
        }
    }
}

fn cancel_connections(connections: &ConnectionRegistry) {
    let active = connections
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .values()
        .cloned()
        .collect::<Vec<_>>();
    for connection in active {
        connection.cancel();
    }
}

fn serve_connection(
    stream: AgentStream,
    bridge: AgentProcessBridge,
    session_token: Arc<[u8; 32]>,
) -> io::Result<()> {
    let mut reader = BufReader::new(stream);
    let mut protocol = AgentProtocolSession::new(bridge, session_token);
    let mut line = Vec::new();
    loop {
        let reply = match read_bounded_line(&mut reader, &mut line)? {
            BoundedLine::Eof => return Ok(()),
            BoundedLine::Line => protocol.handle_line(&line),
            BoundedLine::TooLarge => framing_error_reply("message exceeds the protocol limit"),
            BoundedLine::Unterminated => {
                framing_error_reply("JSON Lines request is missing a newline")
            }
        };
        write_reply(reader.get_mut(), &reply)?;
        if reply.close_connection() {
            return Ok(());
        }
    }
}

fn write_reply(stream: &mut AgentStream, reply: &AgentProtocolReply) -> io::Result<()> {
    stream.write_all(reply.bytes())?;
    stream.flush()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoundedLine {
    Eof,
    Line,
    TooLarge,
    Unterminated,
}

pub(crate) fn read_bounded_line<R: BufRead>(
    reader: &mut R,
    line: &mut Vec<u8>,
) -> io::Result<BoundedLine> {
    line.clear();
    loop {
        let (consumed, ended) = {
            let available = reader.fill_buf()?;
            if available.is_empty() {
                return Ok(if line.is_empty() {
                    BoundedLine::Eof
                } else {
                    BoundedLine::Unterminated
                });
            }
            let newline = available.iter().position(|byte| *byte == b'\n');
            let copied = newline.unwrap_or(available.len());
            if line.len().saturating_add(copied) > MAX_AGENT_MESSAGE_BYTES {
                return Ok(BoundedLine::TooLarge);
            }
            line.extend_from_slice(&available[..copied]);
            (copied + usize::from(newline.is_some()), newline.is_some())
        };
        reader.consume(consumed);
        if ended {
            return Ok(BoundedLine::Line);
        }
    }
}
