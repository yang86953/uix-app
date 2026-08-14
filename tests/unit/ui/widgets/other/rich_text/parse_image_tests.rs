// 引入当前图片候选解析器的私有结果类型与入口。
use super::{ParsedInlineImage, parse_inline_image};

// 验证平衡分隔符、转义和本地路径被完整解码。
#[test]
fn accepts_balanced_escaped_local_image_candidates() {
    // 构造带转义替代文本和嵌套目标括号的相对路径。
    let parsed = parse_inline_image(r"![图\]](assets/(small).png)tail")
        // 合法候选必须返回结构化结果。
        .expect("balanced local image should parse");
    // 只接受图片结果，不允许退化为字面候选。
    match parsed {
        // 读取解码结果和精确消费边界。
        ParsedInlineImage::Image { alt, src, consumed } => {
            // 转义闭方括号应进入替代文本。
            assert_eq!(alt, "图]");
            // 嵌套圆括号应保留在本地路径中。
            assert_eq!(src, "assets/(small).png");
            // 尾随普通文本不得被图片候选吞掉。
            assert_eq!(&r"![图\]](assets/(small).png)tail"[consumed..], "tail");
        }
        // 合法本地候选不能被标记为字面值。
        ParsedInlineImage::Literal { .. } => panic!("local image unexpectedly stayed literal"),
    }
}

// 验证 Windows 盘符路径与包含普通空格的本地文件名保持可用。
#[test]
fn accepts_windows_drive_paths_with_spaces() {
    // 构造不会把盘符误判为 URI scheme 的 Windows 路径。
    let parsed = parse_inline_image(r"![封面](C:\Images\My Cover.png)")
        // Windows 本地路径必须成功形成候选。
        .expect("windows image path should parse");
    // 校验解析出的本地路径保持反斜杠和空格。
    assert!(matches!(
        parsed,
        // 只关心目标字段。
        ParsedInlineImage::Image { src, .. } if src == r"C:\Images\My Cover.png"
    ));
}

// 验证空边界、URI 与控制字符完整保持字面候选。
#[test]
fn rejects_empty_remote_or_control_character_candidates() {
    // 枚举结构完整但不允许进入本地图片生命周期的输入。
    for source in [
        // 空替代文本。
        "![](assets/a.png)",
        // 空目标路径。
        "![图]()",
        // HTTP 资源不在首版范围。
        "![图](https://example.com/a.png)",
        // data URI 同样不属于本地路径。
        "![图](data:image/png;base64,AA)",
        // 目标中的控制字符不能形成稳定文件身份。
        "![图](assets/\u{0000}.png)",
        // 无 scheme 的网络 URI 也不属于本地路径。
        "![图](//example.com/a.png)",
    ] {
        // 每个完整候选都应返回可精确消费的字面结果。
        let parsed = parse_inline_image(source).expect("complete candidate should be classified");
        // 非本地或无效候选不得生成图片结果。
        assert!(
            matches!(parsed, ParsedInlineImage::Literal { consumed } if consumed == source.len())
        );
    }
}

// 验证未闭合候选不会抢占普通文本解析生命周期。
#[test]
fn leaves_unclosed_candidates_unclassified() {
    // 缺少替代文本闭方括号时没有完整候选。
    assert!(parse_inline_image("![图(assets/a.png)").is_none());
    // 缺少目标闭圆括号时同样没有完整候选。
    assert!(parse_inline_image("![图](assets/a.png").is_none());
}
