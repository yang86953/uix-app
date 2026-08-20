// 引入私有 URI 解析入口。
use super::parse_uri_list;

// 标准 URI 列表应保留顺序、空格与 Unicode。
#[test]
fn parses_local_file_uri_list() {
    // 输入覆盖注释、localhost 与百分号 UTF-8。
    let input = b"# source\r\nfile:///tmp/hello%20world.txt\r\nfile://localhost/tmp/%E6%B5%8B%E8%AF%95.txt\r\n";
    // 解析结果应是两个本地绝对路径。
    assert_eq!(
        // 调用纯解析 Widget。
        parse_uri_list(input),
        // 路径顺序与 source 一致。
        vec!["/tmp/hello world.txt", "/tmp/测试.txt"]
    );
}

// 远程、错误转义与非 file scheme 必须被拒绝。
#[test]
fn rejects_non_local_or_malformed_uri_entries() {
    // 只有最后一项是合法本地路径。
    let input =
        b"https://example.test/a\nfile://remote/share/a\nfile:///tmp/%GG\nfile:/tmp/ok.txt\n";
    // 坏条目不得污染合法结果。
    assert_eq!(
        // 调用纯解析 Widget。
        parse_uri_list(input),
        // 单斜线 file URI 保持根路径。
        vec!["/tmp/ok.txt"]
    );
}
