//! 《Agent 控制》公开 Rust 客户端：用私有同用户协议对端验证分帧、限额与失败后不再发请求。
#![cfg(all(unix, feature = "agent-control"))]

use serde_json::{Value, json};
use std::fs::{self, DirBuilder, Permissions};
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;
use uix::app::agent_client::{AgentBridgeClient, AgentBridgeClientError};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
static NEXT_PEER: AtomicU64 = AtomicU64::new(0);

struct Peer {
    directory: PathBuf,
    endpoint: PathBuf,
    worker: Option<thread::JoinHandle<Result>>,
}

impl Peer {
    fn spawn(
        serve: impl FnOnce(&mut BufReader<UnixStream>) -> Result + Send + 'static,
    ) -> Result<Self> {
        let directory = std::env::temp_dir().join(format!(
            "uix-client-{}-{}",
            std::process::id(),
            NEXT_PEER.fetch_add(1, Ordering::Relaxed)
        ));
        DirBuilder::new().mode(0o700).create(&directory)?;
        let endpoint = directory.join("peer.sock");
        let listener = UnixListener::bind(&endpoint)?;
        fs::set_permissions(&endpoint, Permissions::from_mode(0o600))?;
        let worker = thread::spawn(move || {
            let (stream, _) = listener.accept()?;
            stream.set_read_timeout(Some(Duration::from_secs(3)))?;
            stream.set_write_timeout(Some(Duration::from_secs(3)))?;
            serve(&mut BufReader::new(stream))
        });
        Ok(Self {
            directory,
            endpoint,
            worker: Some(worker),
        })
    }

    fn client(&self) -> Result<AgentBridgeClient> {
        Ok(AgentBridgeClient::connect(
            self.endpoint.to_str().ok_or("endpoint encoding")?,
            "fixture-only-token",
        )?)
    }

    fn finish(mut self) -> Result {
        self.worker
            .take()
            .ok_or("worker missing")?
            .join()
            .map_err(|_| "peer panicked")?
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn request(reader: &mut impl BufRead) -> Result<Value> {
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Err("request missing".into());
    }
    Ok(serde_json::from_str(&line)?)
}

fn response(request: &Value, mut body: Value) -> Vec<u8> {
    body["schema"] = json!("uix.agent.v1");
    body["request_id"] = request["request_id"].clone();
    let mut bytes = serde_json::to_vec(&body).unwrap();
    bytes.push(b'\n');
    bytes
}

fn hello(reader: &mut BufReader<UnixStream>, maximum: usize) -> Result {
    let request = request(reader)?;
    assert_eq!(request["type"], "hello");
    assert_eq!(request["token"], "fixture-only-token");
    let bytes = response(
        &request,
        json!({"ok":true,
        "capabilities":{"background_control":{"isolated_workspace":true}},
        "limits":{"max_request_bytes":1024,"max_response_bytes":maximum}}),
    );
    reader.get_mut().write_all(&bytes)?;
    Ok(())
}

#[test]
fn client_preserves_large_fragmented_unicode_responses_and_sequential_ids() -> Result {
    let payload = "中文与转义\n\"内容\"".repeat(8000);
    let expected = payload.clone();
    let peer = Peer::spawn(move |reader| {
        hello(reader, 1024 * 1024)?;
        for _ in 0..3 {
            let req = request(reader)?;
            let bytes = response(&req, json!({"ok":true,"payload":payload}));
            // Deliberately split UTF-8 and escape sequences, then cross the client buffer size.
            let (prefix, tail) = bytes.split_at(113);
            for byte in prefix {
                reader.get_mut().write_all(&[*byte])?;
            }
            for chunk in tail.chunks(4093) {
                reader.get_mut().write_all(chunk)?;
            }
        }
        Ok(())
    })?;
    let mut client = peer.client()?;
    for id in 2..5 {
        let reply = client.request(json!({"type":"snapshot","window_id":1}))?;
        assert_eq!(reply["payload"], expected);
        assert_eq!(reply["request_id"], format!("uix-client-{id}"));
    }
    drop(client);
    peer.finish()
}

#[test]
fn negotiated_response_limit_includes_the_terminating_newline() -> Result {
    for extra in [0, 1] {
        let peer = Peer::spawn(move |reader| {
            hello(reader, 512)?;
            let req = request(reader)?;
            let mut bytes = response(&req, json!({"ok":true}));
            bytes.pop();
            // JSON trailing whitespace is valid and lets the public frame hit the exact boundary.
            bytes.resize(511 + extra, b' ');
            bytes.push(b'\n');
            reader.get_mut().write_all(&bytes)?;
            Ok(())
        })?;
        let mut client = peer.client()?;
        let result = client.request(json!({"type":"list_windows"}));
        if extra == 0 {
            assert_eq!(result?["ok"], true);
        } else {
            assert!(
                matches!(result, Err(AgentBridgeClientError::Io(ref error)) if error.kind() == io::ErrorKind::InvalidData)
            );
        }
        drop(client);
        peer.finish()?;
    }
    Ok(())
}

#[test]
fn incomplete_or_unbound_response_stops_further_dispatch_on_that_client() -> Result {
    for failure in ["truncated", "identity", "schema", "oversize"] {
        let peer = Peer::spawn(move |reader| {
            hello(reader, 512)?;
            let req = request(reader)?;
            let mut body = json!({"ok":true});
            if failure == "oversize" {
                body["payload"] = json!("x".repeat(1024));
            }
            let mut bytes = response(&req, body);
            if failure == "truncated" {
                bytes.pop();
                reader.get_mut().write_all(&bytes)?;
                reader.get_mut().shutdown(std::net::Shutdown::Write)?;
            } else {
                if failure == "identity" || failure == "schema" {
                    let mut body: Value = serde_json::from_slice(&bytes)?;
                    body[if failure == "identity" {
                        "request_id"
                    } else {
                        "schema"
                    }] = json!("wrong");
                    bytes = serde_json::to_vec(&body)?;
                    bytes.push(b'\n');
                }
                reader.get_mut().write_all(&bytes)?;
            }
            let mut extra = String::new();
            assert_eq!(
                reader.read_line(&mut extra)?,
                0,
                "client dispatched after an untrusted response"
            );
            Ok(())
        })?;
        let mut client = peer.client()?;
        assert!(client.request(json!({"type":"list_windows"})).is_err());
        let later =
            client.request(json!({"type":"perform","window_id":1,"action":{"kind":"invoke"}}));
        assert!(
            matches!(later, Err(AgentBridgeClientError::Protocol { ref code, .. }) if code == "connection_unusable")
        );
        drop(client);
        peer.finish()?;
    }
    Ok(())
}

#[test]
fn local_request_rejection_and_bound_business_errors_do_not_poison_connection() -> Result {
    let peer = Peer::spawn(|reader| {
        hello(reader, 1024)?;
        for ok in [false, true] {
            let req = request(reader)?;
            assert_eq!(req["type"], "list_windows");
            reader.get_mut().write_all(&response(
                &req,
                json!({"ok":ok,"error":{"code":"forbidden"}}),
            ))?;
        }
        Ok(())
    })?;
    let mut client = peer.client()?;
    assert!(client.request(json!(["not an object"])).is_err());
    assert!(
        client
            .request(json!({"type":"perform","payload":"x".repeat(2048)}))
            .is_err()
    );
    assert_eq!(client.request(json!({"type":"list_windows"}))?["ok"], false);
    assert_eq!(client.request(json!({"type":"list_windows"}))?["ok"], true);
    drop(client);
    peer.finish()
}

#[test]
fn trusted_execution_unknown_is_returned_but_stops_subsequent_dispatch() -> Result {
    let peer =
        Peer::spawn(|reader| {
            hello(reader, 1024)?;
            let req = request(reader)?;
            reader.get_mut().write_all(&response(&req,
            json!({"ok":false,"error":{"code":"outcome_unknown","message":"execution started"}})))?;
            let mut extra = String::new();
            assert_eq!(reader.read_line(&mut extra)?, 0);
            Ok(())
        })?;
    let mut client = peer.client()?;
    let reply = client.request(json!({"type":"perform","window_id":1}))?;
    assert_eq!(reply["error"]["code"], "outcome_unknown");
    let later = client.request(json!({"type":"perform","window_id":1}));
    assert!(
        matches!(later, Err(AgentBridgeClientError::Protocol { ref code, .. }) if code == "connection_unusable")
    );
    drop(client);
    peer.finish()
}
