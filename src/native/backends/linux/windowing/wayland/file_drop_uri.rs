// Wayland 文件拖放 URI Component 严格解析 text/uri-list 的本地文件路径。

// 解析 URI 列表，仅返回当前主机上的 file URI 路径。
pub(super) fn parse_uri_list(bytes: &[u8]) -> Vec<String> {
    // MIME 文本按 UTF-8 解析，非法整体输入不产生猜测路径。
    let Ok(text) = std::str::from_utf8(bytes) else {
        // FileDrop 契约只承载 String。
        return Vec::new();
    };
    // 每个非注释行独立解析，坏条目不影响其他文件。
    text.lines()
        // 去除 CRLF 以及行首尾协议空白。
        .map(str::trim)
        // 空行与 RFC 注释行不代表文件。
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        // 只保留安全解析成功的本地 file URI。
        .filter_map(parse_local_file_uri)
        // 保留 source 声明顺序。
        .collect()
}

// 把一个本地 file URI 转换为 UTF-8 Linux 路径。
fn parse_local_file_uri(uri: &str) -> Option<String> {
    // scheme 按 URI 规则大小写不敏感。
    let lower = uri.to_ascii_lowercase();
    // 常见三斜线 URI 与 localhost authority 进入 authority 分支。
    let encoded_path = if lower.starts_with("file://") {
        // 去除 scheme 与双斜线。
        let remainder = &uri[7..];
        // 以斜线开头表示空 authority 的本地绝对路径。
        if remainder.starts_with('/') {
            // 保留根斜线。
            remainder
        } else {
            // authority 到首个斜线结束。
            let slash = remainder.find('/')?;
            // 只接受显式 localhost，拒绝远程主机路径。
            if !remainder[..slash].eq_ignore_ascii_case("localhost") {
                // 远程 URI 不映射到本地文件系统。
                return None;
            }
            // localhost 后的斜线开始本地路径。
            &remainder[slash..]
        }
    } else if lower.starts_with("file:/") {
        // 单斜线形式去除 scheme 但保留根路径。
        &uri[5..]
    } else {
        // 非 file scheme 明确拒绝。
        return None;
    };
    // 百分号解码可能因非法转义或 UTF-8 失败而拒绝条目。
    percent_decode_path(encoded_path)
}

// 严格解码 URI 路径中的百分号字节。
fn percent_decode_path(encoded: &str) -> Option<String> {
    // 预分配不小于最终长度的字节缓冲。
    let mut decoded = Vec::with_capacity(encoded.len());
    // 使用索引处理三字节百分号转义。
    let bytes = encoded.as_bytes();
    // 当前读取位置。
    let mut index = 0;
    // 逐字节复制或解码。
    while index < bytes.len() {
        // 百分号必须后跟两个十六进制字符。
        if bytes[index] == b'%' {
            // 缺失任一十六进制位则条目非法。
            if index + 2 >= bytes.len() {
                // 不返回部分路径。
                return None;
            }
            // 解码高四位。
            let high = hex_value(bytes[index + 1])?;
            // 解码低四位。
            let low = hex_value(bytes[index + 2])?;
            // 合并成原始路径字节。
            decoded.push((high << 4) | low);
            // 跳过完整转义。
            index += 3;
        } else {
            // 普通 UTF-8 字节原样保留。
            decoded.push(bytes[index]);
            // 前进一个字节。
            index += 1;
        }
    }
    // NUL 不可安全表示为文件路径契约。
    if decoded.contains(&0) {
        // 明确拒绝嵌入 NUL。
        return None;
    }
    // UiEvent 使用 String，因此要求合法 UTF-8。
    String::from_utf8(decoded).ok()
}

// 把 ASCII 十六进制字符转换为四位值。
fn hex_value(byte: u8) -> Option<u8> {
    // 同时接受 URI 中常见的大写和小写形式。
    match byte {
        // 数字映射到 0..9。
        b'0'..=b'9' => Some(byte - b'0'),
        // 小写字母映射到 10..15。
        b'a'..=b'f' => Some(byte - b'a' + 10),
        // 大写字母映射到 10..15。
        b'A'..=b'F' => Some(byte - b'A' + 10),
        // 其他字符不是合法转义。
        _ => None,
    }
}
