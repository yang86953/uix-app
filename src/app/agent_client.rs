//! `uix.agent.v1` 的官方 Rust 客户端。
//!
//! 服务端 IPC 组装保持私有边界（SMC-06）；本叶是 Agent 协议唯一公开的
//! Rust 消费面，与 `scripts/agent_client.py` 同源同语义，供 Rust 宿主与
//! 集成测试使用。传输复用平台 transport 的同用户字节流，认证走发现文件
//! token 的 `hello` 握手，报文为 JSON Lines 并逐条校验 `schema` 与
//! `request_id` 回显。

use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::platform::adapters::transport::{self, AgentStream};

const PROTOCOL_SCHEMA: &str = "uix.agent.v1";
const INITIAL_MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024;
const INITIAL_MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const HARD_MAX_REQUEST_BYTES: usize = 16 * 1024 * 1024;
const HARD_MAX_RESPONSE_BYTES: usize = 48 * 1024 * 1024;

/// Agent 桥客户端错误：发现文件不可用、传输失败或协议拒绝。
#[derive(Debug)]
pub enum AgentBridgeClientError {
    /// 发现文件定位或读取失败；应用可能尚未发布端点。
    Discovery(String),
    /// 底层字节流 I/O 失败。
    Io(io::Error),
    /// 服务端拒绝或回包不满足协议承诺。
    Protocol { code: String, message: String },
}

impl From<io::Error> for AgentBridgeClientError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for AgentBridgeClientError {
    fn from(error: serde_json::Error) -> Self {
        Self::Io(error.into())
    }
}

impl std::fmt::Display for AgentBridgeClientError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Discovery(message) => {
                write!(formatter, "agent bridge discovery failed: {message}")
            }
            Self::Io(error) => write!(formatter, "agent bridge transport failed: {error}"),
            Self::Protocol { code, message } => {
                write!(
                    formatter,
                    "agent bridge rejected the request: {code}: {message}"
                )
            }
        }
    }
}

impl std::error::Error for AgentBridgeClientError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

/// `list_windows` 返回的窗口身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentWindowEntry {
    pub window_id: u64,
    pub generation: u64,
}

/// 已认证的 Agent 桥连接；一次绑定按 JSON Lines 串行收发请求。
pub struct AgentBridgeClient {
    stream: BufReader<AgentStream>,
    max_request_bytes: usize,
    max_response_bytes: usize,
    next_request_id: u64,
    capabilities: Value,
    connection_usable: bool,
}

impl AgentBridgeClient {
    /// 返回进程发现文件的预期路径；文件就绪与否由调用方判断。
    pub fn discovery_file(process_id: u32) -> Result<PathBuf, AgentBridgeClientError> {
        discovery_directory()
            .map(|directory| directory.join(format!("uix-{process_id}.json")))
            .map_err(|error| AgentBridgeClientError::Discovery(error.to_string()))
    }

    /// 定位进程发现文件、读取端点与 token，并完成 `hello` 握手。
    ///
    /// 调用方通常先轮询 [`Self::discovery_file`] 直到 `state == "ready"`。
    pub fn connect_to_process(process_id: u32) -> Result<Self, AgentBridgeClientError> {
        let path = Self::discovery_file(process_id)?;
        let bytes = std::fs::read(&path)
            .map_err(|error| AgentBridgeClientError::Discovery(error.to_string()))?;
        let descriptor: Value = serde_json::from_slice(&bytes)
            .map_err(|error| AgentBridgeClientError::Discovery(error.to_string()))?;
        let endpoint = descriptor["endpoint"]
            .as_str()
            .ok_or_else(|| AgentBridgeClientError::Discovery("endpoint is missing".into()))?;
        let token = descriptor["token"]
            .as_str()
            .ok_or_else(|| AgentBridgeClientError::Discovery("token is missing".into()))?;
        Self::connect(endpoint, token)
    }

    /// 连接显式端点并完成 `hello` 握手与限额协商。
    pub fn connect(endpoint: &str, token: &str) -> Result<Self, AgentBridgeClientError> {
        let mut client = Self {
            stream: BufReader::new(transport::connect(endpoint)?),
            max_request_bytes: INITIAL_MAX_REQUEST_BYTES,
            max_response_bytes: INITIAL_MAX_RESPONSE_BYTES,
            next_request_id: 0,
            capabilities: Value::Null,
            connection_usable: true,
        };
        let hello = client.exchange(json!({
            "type": "hello",
            "token": token,
        }))?;
        if hello.get("ok").and_then(Value::as_bool) != Some(true) {
            return Err(protocol_error(&hello, "hello"));
        }
        if hello["capabilities"]["background_control"]["isolated_workspace"].as_bool() != Some(true)
        {
            return Err(AgentBridgeClientError::Protocol { code: "background_control_required".into(),
                message: "server does not provide an isolated background workspace; migrate the application before control".into() });
        }
        // v1 旧客户端以 max_message_bytes 作为请求上限别名。
        let limits = &hello["limits"];
        client.max_request_bytes = negotiated_limit(
            limits,
            &["max_request_bytes", "max_message_bytes"],
            INITIAL_MAX_REQUEST_BYTES,
            HARD_MAX_REQUEST_BYTES,
        );
        client.max_response_bytes = negotiated_limit(
            limits,
            &["max_response_bytes"],
            INITIAL_MAX_RESPONSE_BYTES,
            HARD_MAX_RESPONSE_BYTES,
        );
        client.capabilities = hello["capabilities"].clone();
        Ok(client)
    }

    /// 读取握手发布的能力事实；执行后台控制前须确认 isolated_workspace 为 true。
    pub fn capabilities(&self) -> &Value {
        &self.capabilities
    }

    /// 发送完整协议请求，业务错误原样保留在响应信封中；不自动重试动作。
    pub fn request(&mut self, request: Value) -> Result<Value, AgentBridgeClientError> {
        if !request.is_object() {
            return Err(AgentBridgeClientError::Protocol {
                code: "invalid_request".into(),
                message: "agent request must be an object".into(),
            });
        }
        self.exchange(request)
    }

    /// 枚举当前窗口身份；应用尚未发布窗口时返回空表。
    pub fn list_windows(&mut self) -> Result<Vec<AgentWindowEntry>, AgentBridgeClientError> {
        let reply = self.exchange(json!({ "type": "list_windows" }))?;
        if reply.get("ok").and_then(Value::as_bool) != Some(true) {
            return Err(protocol_error(&reply, "list_windows"));
        }
        Ok(reply["windows"]
            .as_array()
            .map(|windows| {
                windows
                    .iter()
                    .filter_map(|window| {
                        Some(AgentWindowEntry {
                            window_id: window["window_id"].as_u64()?,
                            generation: window["generation"].as_u64()?,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    /// 轮询 `list_windows` 直到目标应用发布至少一个窗口。
    pub fn wait_for_window(
        &mut self,
        timeout: Duration,
    ) -> Result<AgentWindowEntry, AgentBridgeClientError> {
        let deadline = Instant::now() + timeout;
        loop {
            let windows = self.list_windows()?;
            if let Some(window) = windows.first().copied() {
                return Ok(window);
            }
            if Instant::now() >= deadline {
                return Err(AgentBridgeClientError::Discovery(
                    "the application did not publish a window".into(),
                ));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// 读取窗口语义快照（`snapshot` 载荷：`nodes` / `revision` /
    /// `presented_revision` 等）。
    pub fn snapshot(&mut self, window_id: u64) -> Result<Value, AgentBridgeClientError> {
        let reply = self.exchange(json!({
            "type": "snapshot",
            "window_id": window_id,
        }))?;
        if reply.get("ok").and_then(Value::as_bool) != Some(true) {
            return Err(protocol_error(&reply, "snapshot"));
        }
        reply
            .get("snapshot")
            .cloned()
            .ok_or_else(|| AgentBridgeClientError::Protocol {
                code: "missing_snapshot".into(),
                message: "snapshot reply is missing the snapshot payload".into(),
            })
    }

    /// 轮询快照直到谓词满足；返回满足谓词的那份快照。
    pub fn wait_for_snapshot(
        &mut self,
        window_id: u64,
        timeout: Duration,
        predicate: impl Fn(&Value) -> bool,
    ) -> Result<Value, AgentBridgeClientError> {
        let deadline = Instant::now() + timeout;
        loop {
            let current = self.snapshot(window_id)?;
            if predicate(&current) {
                return Ok(current);
            }
            if Instant::now() >= deadline {
                return Err(AgentBridgeClientError::Discovery(format!(
                    "timed out waiting for the snapshot predicate: {current}"
                )));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// 对目标执行 `invoke` 动作并返回服务端确认的语义修订号。
    pub fn perform_invoke(
        &mut self,
        window_id: u64,
        generation: u64,
        automation_id: &str,
    ) -> Result<u64, AgentBridgeClientError> {
        self.perform_raw(
            window_id,
            generation,
            automation_id,
            json!({ "kind": "invoke" }),
        )
    }

    /// 对目标执行 `set_value` 动作并返回服务端确认的语义修订号。
    pub fn perform_set_value(
        &mut self,
        window_id: u64,
        generation: u64,
        automation_id: &str,
        value: &str,
    ) -> Result<u64, AgentBridgeClientError> {
        self.perform_raw(
            window_id,
            generation,
            automation_id,
            json!({ "kind": "set_value", "value": value }),
        )
    }

    /// 以原始动作载荷执行 `perform`；供协议新增动作先行使用。
    pub fn perform_raw(
        &mut self,
        window_id: u64,
        generation: u64,
        automation_id: &str,
        action: Value,
    ) -> Result<u64, AgentBridgeClientError> {
        let reply = self.exchange(json!({
            "type": "perform",
            "window_id": window_id,
            "generation": generation,
            "target": { "automation_id": automation_id },
            "action": action,
        }))?;
        if reply.get("ok").and_then(Value::as_bool) != Some(true) {
            return Err(protocol_error(&reply, "perform"));
        }
        reply["revision"]
            .as_u64()
            .ok_or_else(|| AgentBridgeClientError::Protocol {
                code: "missing_revision".into(),
                message: "perform reply is missing the confirmed revision".into(),
            })
    }

    /// 等待修订完成呈现；非 `presented` 结局按协议错误返回。
    pub fn wait_until_presented(
        &mut self,
        window_id: u64,
        generation: u64,
        revision: u64,
        timeout: Duration,
    ) -> Result<(), AgentBridgeClientError> {
        let reply = self.exchange(json!({
            "type": "wait",
            "window_id": window_id,
            "generation": generation,
            "presented_revision": revision,
            "timeout_ms": timeout.as_millis() as u64,
        }))?;
        if reply.get("ok").and_then(Value::as_bool) != Some(true) {
            return Err(protocol_error(&reply, "wait"));
        }
        if reply["outcome"] == "presented" {
            Ok(())
        } else {
            Err(AgentBridgeClientError::Protocol {
                code: "not_presented".into(),
                message: format!("revision {revision} did not reach presentation: {reply}"),
            })
        }
    }

    /// 发送一个 JSON 请求并读取一行响应，校验 schema 与 `request_id` 回显。
    fn exchange(&mut self, request: Value) -> Result<Value, AgentBridgeClientError> {
        if !self.connection_usable {
            return Err(AgentBridgeClientError::Protocol {
                code: "connection_unusable".into(),
                message: "an earlier response was not trustworthy; stop this connection and verify the outcome before any further action".into(),
            });
        }
        self.next_request_id = self.next_request_id.saturating_add(1);
        let request_id = format!("uix-client-{}", self.next_request_id);
        let mut request = request;
        request["schema"] = json!(PROTOCOL_SCHEMA);
        request["request_id"] = json!(request_id);
        let mut encoded = serde_json::to_vec(&request)?;
        if encoded.len() > self.max_request_bytes {
            return Err(AgentBridgeClientError::Protocol {
                code: "request_too_large".into(),
                message: "agent request exceeds the negotiated protocol limit".into(),
            });
        }
        encoded.push(b'\n');
        // 一旦开始写入，任何 I/O、分帧或关联失败都不得继续派发该连接的后续动作。
        // 本地参数拒绝在此前返回；可信业务拒绝仍可在此连接上继续读取。
        self.connection_usable = false;
        self.stream.get_mut().write_all(&encoded)?;
        self.stream.get_mut().flush()?;
        let line = read_bounded_line(&mut self.stream, self.max_response_bytes)?;
        let reply: Value = serde_json::from_slice(&line)?;
        if reply["schema"] != PROTOCOL_SCHEMA {
            return Err(AgentBridgeClientError::Protocol {
                code: "schema_mismatch".into(),
                message: "agent reply schema does not match the client schema".into(),
            });
        }
        if reply["request_id"].as_str() != Some(request_id.as_str()) {
            return Err(AgentBridgeClientError::Protocol {
                code: "request_id_mismatch".into(),
                message: "agent reply request_id does not match the request".into(),
            });
        }
        // 保留可信的业务错误信封，但已经开始执行且终态未知时也不能继续派发。
        self.connection_usable = reply["error"]["code"] != "outcome_unknown";
        Ok(reply)
    }
}

/// 服务端发现目录的只读定位；目录由服务端创建，客户端不重复建立。
fn discovery_directory() -> io::Result<PathBuf> {
    #[cfg(windows)]
    {
        let local_app_data = std::env::var_os("LOCALAPPDATA").ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA is unavailable")
        })?;
        Ok(PathBuf::from(local_app_data).join("uix-agent"))
    }
    #[cfg(unix)]
    {
        let effective_uid = unsafe {
            // SAFETY: `geteuid` 无参数且无内存安全前置条件；
            // 与 transport 发现目录实现保持同一身份来源。
            libc::geteuid()
        };
        let base = std::env::var_os("XDG_RUNTIME_DIR")
            .filter(|value| !value.is_empty())
            .map_or_else(std::env::temp_dir, PathBuf::from);
        Ok(base.join(format!("uix-agent-{effective_uid}")))
    }
    #[cfg(not(any(windows, unix)))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "agent discovery is unsupported on this platform",
        ))
    }
}

fn negotiated_limit(limits: &Value, names: &[&str], fallback: usize, hard_max: usize) -> usize {
    for name in names {
        if let Some(value) = limits[*name].as_u64() {
            return (value as usize).min(hard_max).max(1);
        }
    }
    fallback
}

fn protocol_error(reply: &Value, operation: &str) -> AgentBridgeClientError {
    AgentBridgeClientError::Protocol {
        code: reply["error"]["code"]
            .as_str()
            .unwrap_or("unknown")
            .to_owned(),
        message: format!(
            "{operation} failed: {}",
            reply["error"]["message"].as_str().unwrap_or("unspecified")
        ),
    }
}

/// 按缓冲块读取一行响应；协商上限包含结尾换行，不读取或丢弃后续帧。
fn read_bounded_line(reader: &mut impl BufRead, maximum_bytes: usize) -> io::Result<Vec<u8>> {
    let mut line = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            if line.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "agent bridge closed before a reply",
                ));
            }
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "agent bridge closed mid-reply",
            ));
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let copied = newline.unwrap_or(available.len());
        let consumed = copied + usize::from(newline.is_some());
        if line.len().saturating_add(consumed) > maximum_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "agent reply exceeds the negotiated protocol limit",
            ));
        }
        line.extend_from_slice(&available[..copied]);
        reader.consume(consumed);
        if newline.is_some() {
            return Ok(line);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn discovery_directory_follows_the_documented_unix_layout() {
        let directory = discovery_directory().expect("unix discovery directory");
        let base = std::env::var_os("XDG_RUNTIME_DIR")
            .filter(|value| !value.is_empty())
            .map_or_else(std::env::temp_dir, PathBuf::from);
        let expected_suffix = format!("uix-agent-{}", unsafe {
            // SAFETY: `geteuid` 无参数且无内存安全前置条件。
            libc::geteuid()
        });
        assert!(directory.starts_with(&base));
        assert!(directory.ends_with(expected_suffix));
    }

    #[test]
    fn discovery_file_names_the_process_descriptor() {
        let path = AgentBridgeClient::discovery_file(4711)
            .unwrap_or_else(|error| panic!("discovery file: {error}"));
        assert!(path.to_string_lossy().ends_with("uix-4711.json"));
    }

    #[test]
    fn negotiated_limit_prefers_current_names_and_caps_hard_maximum() {
        let limits = json!({ "max_request_bytes": u64::MAX });
        assert_eq!(
            negotiated_limit(&limits, &["max_request_bytes"], 1024, 4096),
            4096
        );
        let legacy = json!({ "max_message_bytes": 2048 });
        assert_eq!(
            negotiated_limit(
                &legacy,
                &["max_request_bytes", "max_message_bytes"],
                1024,
                4096
            ),
            2048
        );
        assert_eq!(
            negotiated_limit(&json!({}), &["max_request_bytes"], 1024, 4096),
            1024
        );
    }
}
