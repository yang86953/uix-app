// 导入管道文件与有缓冲 JSON Lines 读取。
use std::fs::File;
// 导入请求写入、刷新与逐行响应读取。
use std::io::{BufRead, BufReader, Write};

// 导入公开协议 JSON 构造与值类型。
use serde_json::{Value, json};

// 保存 wait 请求允许真实恢复呈现的最长时间。
const PRESENT_TIMEOUT_MS: u64 = 30_000;

// 保存当前 Agent 窗口的跨请求稳定身份。
#[derive(Clone, Copy)]
pub(crate) struct AgentWindow {
    // 保存进程内窗口 ID。
    pub(crate) id: u64,
    // 保存窗口生命周期 generation。
    pub(crate) generation: u64,
}

// 通过公开 uix.agent.v1 执行有界同步 Query/Command。
pub(crate) struct AgentClient {
    // 保存唯一 JSON Lines 管道连接。
    connection: BufReader<File>,
    // 保存单连接内单调 request ID 后缀。
    next_request: u64,
}

// 实现 Agent 协议的最小真实窗口验收 Adapter。
impl AgentClient {
    // 完成首条 hello 鉴权并返回可用客户端。
    pub(crate) fn handshake(stream: File, token: &str) -> Self {
        // 创建尚未发送请求的单连接客户端。
        let mut client = Self {
            // 为逐行响应建立缓冲读取器。
            connection: BufReader::new(stream),
            // 从首个 request ID 开始。
            next_request: 1,
        };
        // 发送不会记录 token 的 hello 请求。
        let _ = client.request(
            // 使用公开 hello operation。
            "hello",
            // 提供 descriptor token 与测试客户端身份。
            json!({
                "token": token,
                "client": { "name": "uix-lang-graphics-recovery-acceptance" },
            }),
        );
        // 返回已经通过鉴权的客户端。
        client
    }

    // 列出并取得专用验收模式的唯一窗口。
    pub(crate) fn list_single_window(&mut self) -> AgentWindow {
        // 执行无额外字段的窗口 Query。
        let response = self.request("list_windows", json!({}));
        // 读取窗口数组。
        let windows = response["windows"]
            // 协议必须返回数组。
            .as_array()
            // 缺失数组时给出明确形状错误。
            .expect("list_windows response windows");
        // 专用图形恢复模式只允许一个根窗口。
        assert_eq!(
            windows.len(),
            1,
            "graphics recovery mode must expose one window"
        );
        // 借用唯一窗口记录。
        let window = &windows[0];
        // 返回跨后续请求使用的 ID 与 generation。
        AgentWindow {
            // 读取数值窗口 ID。
            id: window["window_id"].as_u64().expect("window id"),
            // 读取数值 generation。
            generation: window["generation"].as_u64().expect("window generation"),
        }
    }

    // 读取当前窗口的完整语义快照。
    pub(crate) fn snapshot(&mut self, window: AgentWindow) -> Value {
        // 执行指定窗口的 snapshot Query。
        let response = self.request(
            // 使用公开 snapshot operation。
            "snapshot",
            // 只传递当前窗口 ID。
            json!({ "window_id": window.id }),
        );
        // 返回拥有所有权的快照值。
        response["snapshot"].clone()
    }

    // 对稳定 automation ID 执行公开 invoke Command。
    pub(crate) fn invoke(&mut self, window: AgentWindow, automation_id: &str) -> u64 {
        // 执行带 generation 的语义动作。
        let response = self.request(
            // 使用公开 perform operation。
            "perform",
            // 指定窗口、代际、稳定目标和 invoke 动作。
            json!({
                "window_id": window.id,
                "generation": window.generation,
                "target": { "automation_id": automation_id },
                "action": { "kind": "invoke" },
            }),
        );
        // 返回命令建立的新语义 revision。
        response["revision"]
            // revision 必须是数值。
            .as_u64()
            // 缺失值时报告具体响应形状错误。
            .expect("perform response revision")
    }

    // 等待指定 revision 已由真实窗口完成 present。
    pub(crate) fn wait_for_presented(&mut self, window: AgentWindow, revision: u64) {
        // 使用公开 wait Query 区分语义变化与真实提交。
        let response = self.request(
            // 使用公开 wait operation。
            "wait",
            // 指定窗口、代际、目标 presented revision 与有界超时。
            json!({
                "window_id": window.id,
                "generation": window.generation,
                "presented_revision": revision,
                "timeout_ms": PRESENT_TIMEOUT_MS,
            }),
        );
        // 只有明确 presented outcome 才证明真实提交完成。
        assert_eq!(response["outcome"], "presented");
    }

    // 发送一条带唯一 request ID 的公开协议请求。
    fn request(&mut self, operation: &str, fields: Value) -> Value {
        // 为当前连接生成单调且可诊断的 request ID。
        let request_id = format!("graphics-recovery-{operation}-{}", self.next_request);
        // 推进下一请求后缀，避免超时或失败时复用身份。
        self.next_request = self.next_request.saturating_add(1);
        // 请求附加字段必须是 JSON 对象。
        let mut request = fields
            // 消费调用方字段对象。
            .as_object()
            // 缺失对象属于测试 Adapter 使用错误。
            .expect("Agent request fields must be an object")
            // 克隆为本次请求拥有的可变 map。
            .clone();
        // 安装稳定协议 schema。
        request.insert(
            "schema".to_string(),
            Value::String("uix.agent.v1".to_string()),
        );
        // 安装本次唯一 request ID。
        request.insert(
            // 使用公开字段名。
            "request_id".to_string(),
            // 保存拥有所有权的 request ID。
            Value::String(request_id.clone()),
        );
        // 安装公开 operation 类型。
        request.insert("type".to_string(), Value::String(operation.to_string()));
        // 编码单条 JSON 请求。
        let mut bytes = serde_json::to_vec(&Value::Object(request))
            // 序列化失败属于 Adapter 缺陷。
            .expect("serialize Agent request");
        // JSON Lines 每条请求必须以换行结束。
        bytes.push(b'\n');
        // 写入完整请求帧。
        self.connection
            // 取得底层管道写端。
            .get_mut()
            // 写入全部字节。
            .write_all(&bytes)
            // 写入失败立即报告连接错误。
            .expect("write Agent request");
        // 刷新请求，避免等待响应时仍留在用户态缓冲。
        self.connection
            // 取得底层管道写端。
            .get_mut()
            // 刷新当前帧。
            .flush()
            // 刷新失败立即报告连接错误。
            .expect("flush Agent request");

        // 为单条 JSON Lines 响应创建缓冲。
        let mut response = String::new();
        // 读取一条完整响应。
        let read = self
            // 借用缓冲连接。
            .connection
            // 读取到换行或 EOF。
            .read_line(&mut response)
            // 读取失败立即报告连接错误。
            .expect("read Agent response");
        // EOF 不能被解释为业务拒绝或成功。
        assert!(read > 0, "Agent connection closed without a response");
        // 解析拥有所有权的响应值。
        let response: Value = serde_json::from_str(&response)
            // 非法 JSON 属于协议 Adapter 失败。
            .expect("parse Agent response");
        // 响应必须保持同一 schema。
        assert_eq!(response["schema"], "uix.agent.v1");
        // 响应必须精确关联当前请求。
        assert_eq!(response["request_id"], request_id);
        // 本验收路径的每条请求都必须明确成功。
        assert_eq!(response["ok"], true, "Agent protocol error: {response}");
        // 返回经过关联与成功校验的响应。
        response
    }
}
