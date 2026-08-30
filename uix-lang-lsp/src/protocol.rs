//! LSP JSON-RPC stdio 协议 Module：帧读写、URI 与路径换算及响应封装。

use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

// 读取一条 Content-Length 分帧的 JSON-RPC 消息；输入耗尽返回 None。
pub(crate) fn read_message(input: &mut impl BufRead) -> Result<Option<Value>, String> {
    // 逐行消费头部，直到空行为止。
    let mut length = None;
    let mut saw_header = false;
    let mut line = String::new();
    loop {
        line.clear();
        let read = input
            .read_line(&mut line)
            .map_err(|error| error.to_string())?;
        // 输入在头部中途耗尽属于协议错误；正常耗尽表示服务器可以退出。
        if read == 0 {
            return if saw_header {
                Err("LSP header 不完整".into())
            } else {
                Ok(None)
            };
        }
        if line == "\n" || line == "\r\n" {
            break;
        }
        saw_header = true;
        let header = line.trim_end_matches(['\r', '\n']);
        if let Some(value) = header.strip_prefix("Content-Length:") {
            length = Some(value.trim().parse::<usize>().map_err(|e| e.to_string())?);
        }
    }
    let length = length.ok_or("缺少 Content-Length")?;
    // 只为当前消息申请精确 body 容量；处理完成后立即释放。
    let mut body = Vec::new();
    body.try_reserve_exact(length)
        .map_err(|error| format!("LSP body 内存申请失败：{error}"))?;
    body.resize(length, 0);
    input.read_exact(&mut body).map_err(|error| {
        if error.kind() == std::io::ErrorKind::UnexpectedEof {
            "LSP body 不完整".to_string()
        } else {
            error.to_string()
        }
    })?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|e| e.to_string())
}

// 写出一条带 Content-Length 头的 JSON-RPC 消息。
pub(crate) fn write_message(out: &mut impl Write, value: &Value) -> Result<(), String> {
    let body = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    write!(out, "Content-Length: {}\r\n\r\n", body.len())
        .and_then(|_| out.write_all(&body))
        .map_err(|e| e.to_string())
}

// 把 file:// URI 还原为本地路径；非文件 URI 返回 None。
pub(crate) fn uri_to_path(uri: &str) -> Option<PathBuf> {
    uri.strip_prefix("file://").map(|p| {
        PathBuf::from(if p.starts_with('/') {
            p.to_string()
        } else {
            format!("/{p}")
        })
    })
}

// 把本地路径编码为 file:// URI，统一正斜杠分隔。
pub(crate) fn path_to_uri(path: &Path) -> String {
    #[cfg(windows)]
    {
        format!("file:///{}", path.display().to_string().replace('\\', "/"))
    }
    #[cfg(not(windows))]
    {
        format!("file://{}", path.display())
    }
}

// 把编译器来源名（规范路径）映射回 URI；内嵌或虚拟来源回退请求 URI。
pub(crate) fn source_name_to_uri(source_name: &str, fallback: &str) -> String {
    // 绝对路径按 file URI 编码；其余来源名不是可定位路径。
    if Path::new(source_name).is_absolute() {
        path_to_uri(Path::new(source_name))
    } else {
        fallback.to_string()
    }
}

// 组装对请求的成功响应帧。
pub(crate) fn response(id: Option<Value>, result: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"result":result})
}

// 组装对请求的错误响应帧。
pub(crate) fn error_response(id: Option<Value>, code: i64, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}

// 组装一条服务器主动通知。
pub(crate) fn notification(method: &str, params: Value) -> Value {
    json!({"jsonrpc":"2.0","method":method,"params":params})
}

#[cfg(test)]
mod tests {
    use super::{path_to_uri, read_message, uri_to_path, write_message};
    use serde_json::{Value, json};
    use std::io::Cursor;

    // 把消息编码为 LSP 分帧字节。
    fn frame(value: &Value) -> Vec<u8> {
        let body = serde_json::to_vec(value).expect("测试消息必须可序列化");
        let mut data = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
        data.extend_from_slice(&body);
        data
    }

    #[test]
    fn framing_round_trip() {
        let mut input = Cursor::new(frame(&json!({"method": "shutdown"})));
        assert_eq!(
            read_message(&mut input)
                .expect("完整帧必须可读")
                .expect("必须返回一条消息")["method"],
            "shutdown"
        );
    }

    #[test]
    fn streaming_reader_stops_at_the_current_frame() {
        // 两条消息连发时单次读取不得越过帧边界。
        let first = frame(&json!({"id":1,"method":"initialize"}));
        let first_length = first.len() as u64;
        let mut bytes = first;
        bytes.extend_from_slice(&frame(&json!({"id":2,"method":"shutdown"})));
        let mut input = Cursor::new(bytes);

        assert_eq!(
            read_message(&mut input)
                .expect("首帧必须可读")
                .expect("必须返回消息")["id"],
            1
        );
        assert_eq!(
            input.position(),
            first_length,
            "单次读取不得消费下一条 LSP 消息"
        );
        assert_eq!(
            read_message(&mut input)
                .expect("次帧必须可读")
                .expect("必须返回消息")["id"],
            2
        );
        assert!(
            read_message(&mut input)
                .expect("耗尽必须正常结束")
                .is_none()
        );
    }

    #[test]
    fn streaming_reader_rejects_a_truncated_body() {
        let mut input = Cursor::new(b"Content-Length: 5\r\n\r\n{}".to_vec());
        assert_eq!(
            read_message(&mut input).expect_err("截断 body 必须失败"),
            "LSP body 不完整",
        );
    }

    #[test]
    fn non_file_uris_have_no_local_path() {
        assert!(uri_to_path("untitled:Untitled-1").is_none());
        assert_eq!(
            uri_to_path("file:///tmp/demo.uix"),
            Some(std::path::PathBuf::from("/tmp/demo.uix"))
        );
    }

    #[test]
    fn path_uri_round_trip_keeps_absolute_paths() {
        let path = std::path::Path::new("/tmp/demo.uix");
        let uri = path_to_uri(path);
        assert_eq!(uri_to_path(&uri).as_deref(), Some(path));
    }

    #[test]
    fn written_messages_use_content_length_framing() {
        let mut buffer = Vec::new();
        write_message(&mut buffer, &json!({"id":1,"result":null})).expect("内存写入必须成功");
        let header = String::from_utf8_lossy(&buffer);
        let length: usize = header
            .split("Content-Length: ")
            .nth(1)
            .and_then(|value| value.split("\r\n").next())
            .and_then(|value| value.parse().ok())
            .expect("帧头必须携带可解析的 Content-Length");
        let header_length = header.find("\r\n\r\n").expect("必须有头部结束标记") + 4;
        assert_eq!(buffer.len() - header_length, length);
    }
}
