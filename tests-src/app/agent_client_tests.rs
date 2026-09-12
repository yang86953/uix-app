//! `app/agent_client.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

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
