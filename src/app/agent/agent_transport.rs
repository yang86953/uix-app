//! 认证原生 Agent Bridge 端点的进程生命周期管理。

use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::json;

use crate::app::agent::agent_bridge::AgentProcessBridge;
use crate::app::agent::agent_protocol::{
    AGENT_PROTOCOL_SCHEMA, AgentProtocolReply, AgentProtocolSession, MAX_AGENT_CONNECTIONS,
    MAX_AGENT_REQUEST_BYTES, encode_session_token, framing_error_reply,
};
use crate::platform::adapters::transport::{
    AcceptedAgentStream, AgentEndpoint, AgentEndpointWake, AgentStream, AgentStreamCancelIo,
    agent_hub_endpoint_name, connect, fill_secure_random,
};

const AGENT_HUB_SCHEMA: &str = "uix.agent.hub.v1";

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
    hub_registration_thread: Option<JoinHandle<()>>,
    listener_thread: Option<JoinHandle<()>>,
}

type ConnectionRegistry = Arc<Mutex<BTreeMap<u64, Arc<dyn AgentStreamCancelIo>>>>;

// 应用已发布 closed 终态后，为在途 wait 回复保留的有界传输排空窗口。
const TERMINAL_REPLY_DRAIN_TIMEOUT: Duration = Duration::from_millis(500);
// Hub 心跳只承担实例存活登记；动作仍走应用自己的已认证端点。
const HUB_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(1);
const HUB_RECONNECT_MIN_DELAY: Duration = Duration::from_millis(250);
const HUB_RECONNECT_MAX_DELAY: Duration = Duration::from_secs(2);

impl AgentTransportHandle {
    pub(crate) fn start(
        bridge: AgentProcessBridge,
        display_name: &str,
    ) -> Result<Self, AgentTransportError> {
        // 生成 32 字节会话 token，并取前 24 字节作为端点 nonce。
        let mut token = [0u8; 32];
        fill_secure_random(&mut token).map_err(|source| AgentTransportError::Io {
            stage: "random token generation",
            source,
        })?;
        let encoded_token = encode_session_token(&token);
        let nonce = &encoded_token[..24];
        let process_id = std::process::id();
        // 绑定命名端点（含进程 id 与 nonce，保证进程间唯一）。
        let mut endpoint =
            AgentEndpoint::bind(process_id, nonce).map_err(|source| AgentTransportError::Io {
                stage: "endpoint bind",
                source,
            })?;
        let info = AgentTransportInfo {
            endpoint: endpoint.endpoint_name(),
            discovery_path: endpoint.discovery_path().to_path_buf(),
        };
        // 序列化发现描述符：协议版本、端点地址与会话 token。
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
        // 发布发现文件，供外部 agent 定位本进程端点。
        endpoint
            .publish_discovery(&descriptor, nonce)
            .map_err(|source| AgentTransportError::Io {
                stage: "discovery publication",
                source,
            })?;

        // Hub 仅保存应用实例到直连端点的租约；token 不进入 Hub 的枚举响应或日志。
        let app_id = default_agent_app_id();
        let display_name = sanitized_agent_display_name(display_name, &app_id);
        let hub_registration = serde_json::to_vec(&json!({
            "schema": AGENT_HUB_SCHEMA,
            "type": "register_app",
            "app_id": app_id,
            "display_name": display_name,
            "instance_id": nonce,
            "process_id": process_id,
            "endpoint": info.endpoint,
            "token": encoded_token,
        }))
        .map_err(|source| AgentTransportError::Io {
            stage: "hub registration serialization",
            source: io::Error::new(io::ErrorKind::InvalidData, source),
        })?;

        let shutdown = Arc::new(AtomicBool::new(false));
        let connections: ConnectionRegistry = Arc::new(Mutex::new(BTreeMap::new()));
        let wake_listener = endpoint.waker();
        let session_token = Arc::new(token);
        // 启动专用监听线程：接受连接、认证并按连接分派 worker 线程。
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

        // Hub 可以晚于应用启动；后台登记失败不降级应用本身，也不影响兼容直连端点。
        let registration_shutdown = shutdown.clone();
        let hub_registration_thread = match thread::Builder::new()
            .name("uix-agent-hub-registration".to_owned())
            .spawn(move || hub_registration_loop(hub_registration, registration_shutdown))
        {
            Ok(thread) => Some(thread),
            Err(error) => {
                // agent 线程基础设施不持有诊断句柄：经显式边界观察入口记录。
                crate::diagnostics::observe_boundary_error(
                    "agent/transport",
                    &crate::core::Error::new(
                        crate::core::Errc::IoError,
                        error.to_string(),
                    ),
                );
                None
            }
        };

        tracing::info!(
            "agent bridge ready discovery={}",
            info.discovery_path.display()
        );
        Ok(Self {
            info,
            shutdown,
            wake_listener,
            connections,
            hub_registration_thread,
            listener_thread: Some(listener_thread),
        })
    }

    pub(crate) fn info(&self) -> &AgentTransportInfo {
        &self.info
    }

    pub(crate) fn shutdown(&mut self) {
        // 首次关闭：置位标志、唤醒监听线程并取消全部活跃连接。
        if !self.shutdown.swap(true, Ordering::AcqRel) {
            self.wake_listener.wake();
            cancel_connections(&self.connections);
        }
        if let Some(registration_thread) = self.hub_registration_thread.take() {
            if registration_thread.join().is_err() {
                tracing::error!("agent hub registration thread panicked during shutdown");
            }
        }
        // 等待监听线程退出，避免进程结束前残留未回收线程。
        if let Some(listener_thread) = self.listener_thread.take() {
            if listener_thread.join().is_err() {
                tracing::error!("agent listener thread panicked during shutdown");
            }
        }
    }

    pub(crate) fn shutdown_after_terminal_reply(&mut self) {
        // close_all 已唤醒 wait；先允许终态帧写回，超时后仍由普通 shutdown 强制回收。
        let _ = wait_for_connections_to_drain(&self.connections, TERMINAL_REPLY_DRAIN_TIMEOUT);
        self.shutdown();
    }
}

fn default_agent_app_id() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "uix-app".to_owned())
}

fn sanitized_agent_display_name(display_name: &str, fallback: &str) -> String {
    let sanitized: String = display_name
        .chars()
        .filter(|character| !character.is_control())
        .take(512)
        .collect();
    if sanitized.is_empty() {
        fallback.to_owned()
    } else {
        sanitized
    }
}

fn hub_registration_loop(registration: Vec<u8>, shutdown: Arc<AtomicBool>) {
    let Ok(hub_endpoint) = agent_hub_endpoint_name() else {
        return;
    };
    let heartbeat = format!("{{\"schema\":\"{AGENT_HUB_SCHEMA}\",\"type\":\"heartbeat\"}}\n");
    let mut reconnect_delay = HUB_RECONNECT_MIN_DELAY;
    while !shutdown.load(Ordering::Acquire) {
        let mut stream = match connect(&hub_endpoint) {
            Ok(stream) => stream,
            Err(_) => {
                sleep_until_shutdown(&shutdown, reconnect_delay);
                reconnect_delay = (reconnect_delay * 2).min(HUB_RECONNECT_MAX_DELAY);
                continue;
            }
        };
        reconnect_delay = HUB_RECONNECT_MIN_DELAY;
        if stream.write_all(&registration).is_err()
            || stream.write_all(b"\n").is_err()
            || stream.flush().is_err()
        {
            continue;
        }
        tracing::debug!("agent instance registered with local hub");
        while !shutdown.load(Ordering::Acquire) {
            sleep_until_shutdown(&shutdown, HUB_HEARTBEAT_INTERVAL);
            if shutdown.load(Ordering::Acquire) {
                break;
            }
            if stream.write_all(heartbeat.as_bytes()).is_err() || stream.flush().is_err() {
                break;
            }
        }
    }
}

fn sleep_until_shutdown(shutdown: &AtomicBool, duration: Duration) {
    let deadline = Instant::now() + duration;
    while !shutdown.load(Ordering::Acquire) {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        thread::sleep(remaining.min(Duration::from_millis(100)));
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
    // 循环接受新连接，直到收到关闭信号。
    while !shutdown.load(Ordering::Acquire) {
        let accepted = match endpoint.accept() {
            Ok(accepted) => accepted,
            Err(error) => {
                // 非关闭状态下的 accept 失败视为致命错误，终止监听。
                if !shutdown.load(Ordering::Acquire) {
                    // agent 线程基础设施不持有诊断句柄：经显式边界观察入口记录。
                crate::diagnostics::observe_boundary_error(
                    "agent/transport",
                    &crate::core::Error::new(
                        crate::core::Errc::IoError,
                        error.to_string(),
                    ),
                );
                }
                break;
            }
        };
        // 关闭竞态：刚接受就收到关闭信号则取消该连接。
        if shutdown.load(Ordering::Acquire) {
            accepted.cancel.cancel();
            break;
        }

        // 分配连接序号（从 1 开始）并登记取消句柄。
        let connection_id = next_connection_id.fetch_add(1, Ordering::Relaxed).max(1);
        let AcceptedAgentStream { stream, cancel } = accepted;
        {
            let mut active = connections
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            // 活跃连接数达到上限时直接拒绝并取消新连接。
            if active.len() >= MAX_AGENT_CONNECTIONS {
                drop(active);
                cancel.cancel();
                continue;
            }
            active.insert(connection_id, cancel.clone());
        }

        // 每个连接一个 worker 线程，串行处理该连接上的协议请求。
        let worker_bridge = bridge.clone();
        let worker_token = session_token.clone();
        let worker_shutdown = shutdown.clone();
        let worker_connections = connections.clone();
        match thread::Builder::new()
            .name(format!("uix-agent-connection-{connection_id}"))
            .spawn(move || {
                if let Err(error) = serve_connection(stream, worker_bridge, worker_token) {
                    if !worker_shutdown.load(Ordering::Acquire) {
                        crate::diagnostics::observe_boundary_error(
                    "agent/transport",
                    &crate::core::Error::new(
                        crate::core::Errc::IoError,
                        error.to_string(),
                    ),
                );
                    }
                }
                // worker 退出时从注册表摘除自身，释放连接名额。
                worker_connections
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .remove(&connection_id);
            }) {
            Ok(worker) => {
                // 让取消句柄持有 worker，便于关闭时终止阻塞中的线程。
                cancel.bind_worker(&worker);
                workers.push(worker);
            }
            Err(error) => {
                // 线程创建失败：回滚登记并记录错误。
                connections
                    .lock()
                    .unwrap_or_else(|lock_error| lock_error.into_inner())
                    .remove(&connection_id);
                crate::diagnostics::observe_boundary_error(
                    "agent/transport",
                    &crate::core::Error::new(
                        crate::core::Errc::IoError,
                        error.to_string(),
                    ),
                );
            }
        }
    }

    // 监听结束：取消全部连接并回收 worker 线程。
    cancel_connections(&connections);
    for worker in workers {
        if worker.join().is_err() {
            tracing::error!("agent connection thread panicked during shutdown");
        }
    }
}

fn cancel_connections(connections: &ConnectionRegistry) {
    // 快照全部连接后逐个取消，避免持锁期间调用平台取消逻辑。
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

fn wait_for_connections_to_drain(connections: &ConnectionRegistry, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if connections
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .is_empty()
        {
            return true;
        }
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return false;
        };
        if remaining.is_zero() {
            return false;
        }
        thread::sleep(remaining.min(Duration::from_millis(2)));
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
    // 逐行读取并响应，直到 EOF 或协议要求关闭连接。
    loop {
        let reply = match read_bounded_line(&mut reader, &mut line)? {
            // 客户端关闭连接：正常结束。
            BoundedLine::Eof => return Ok(()),
            BoundedLine::Line => protocol.handle_line(&line),
            // 帧过长或无换行结尾：以框架错误回复并关闭连接。
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
            // 取缓冲中当前可用的一段，避免整行读到内存后才发现超限。
            let available = reader.fill_buf()?;
            if available.is_empty() {
                // 数据流已尽：无内容为 EOF，有半行则为未终止帧。
                return Ok(if line.is_empty() {
                    BoundedLine::Eof
                } else {
                    BoundedLine::Unterminated
                });
            }
            // 复制到首个换行符为止（无换行则复制整段）。
            let newline = available.iter().position(|byte| *byte == b'\n');
            let copied = newline.unwrap_or(available.len());
            // 累计长度超过协议上限即判定超限，不再继续累积。
            if line.len().saturating_add(copied) > MAX_AGENT_REQUEST_BYTES {
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

// 帧划分纯逻辑专项测试（仅 agent-control 能力下编译，不启动线程与 IO）。

#[cfg(test)]
mod tests {
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
}
