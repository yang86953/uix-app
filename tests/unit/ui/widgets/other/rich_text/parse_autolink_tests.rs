// RichText Markdown 尖括号 HTTP(S) 自动链接回归测试。
use super::*;

// 验证 HTTP 与 HTTPS 自动链接复用现有 Link 段并保留原始目标。
#[test]
fn parses_http_and_https_angle_autolinks() {
    // 解析普通文本之间的小写 HTTPS 与大写 HTTP scheme。
    let segments = parse_rich_text(
        "文档 <https://example.test/guide?q=1> 镜像 <HTTP://example.test/download>",
    );
    // 两个自动链接应保持输入 URL，并由普通文本段分隔。
    assert_eq!(
        segments,
        vec![
            // 自动链接之前的普通文本保持默认样式。
            RichTextSegment::Text {
                content: "文档 ".into(),
                style: RichTextStyle::default(),
            },
            // HTTPS 自动链接直接显示并提交原始 URL。
            RichTextSegment::Link {
                content: "https://example.test/guide?q=1".into(),
                url: "https://example.test/guide?q=1".into(),
            },
            // 两个链接之间的普通文本保持原样。
            RichTextSegment::Text {
                content: " 镜像 ".into(),
                style: RichTextStyle::default(),
            },
            // scheme 匹配忽略 ASCII 大小写，但链接值保留作者拼写。
            RichTextSegment::Link {
                content: "HTTP://example.test/download".into(),
                url: "HTTP://example.test/download".into(),
            },
        ]
    );
}

// 验证 Markdown 邮件自动链接显示原始地址并提交 mailto 目标。
#[test]
fn parses_email_angle_autolinks() {
    // 解析带点号/加号的域名邮箱与单 label 域邮箱。
    let segments = parse_rich_text("联系 <User.Name+tag@Example-Domain.test> 或 <ops@localhost>");
    // 两个邮箱都应复用现有 Link 段并由普通文本分隔。
    assert_eq!(
        // 传入实际解析结果。
        segments,
        // 构造保持作者大小写的预期段列表。
        vec![
            // 第一个邮箱之前的普通文本保持默认样式。
            RichTextSegment::Text {
                // 保留原始中文前缀和空格。
                content: "联系 ".into(),
                // 普通正文使用默认样式。
                style: RichTextStyle::default(),
            },
            // 带加号标签的邮箱成为可交互链接。
            RichTextSegment::Link {
                // 显示文本保留作者输入的原始邮箱。
                content: "User.Name+tag@Example-Domain.test".into(),
                // 提交目标增加 mailto scheme。
                url: "mailto:User.Name+tag@Example-Domain.test".into(),
            },
            // 两个邮箱之间的普通文本保持原样。
            RichTextSegment::Text {
                // 保留分隔正文。
                content: " 或 ".into(),
                // 分隔正文使用默认样式。
                style: RichTextStyle::default(),
            },
            // 单 label 域仍属于受支持邮件候选。
            RichTextSegment::Link {
                // 显示原始短邮箱。
                content: "ops@localhost".into(),
                // 提交标准 mailto 目标。
                url: "mailto:ops@localhost".into(),
            },
        ],
    );
}

// 验证非法 local-part、domain 与非 ASCII 邮箱保持字面文本。
#[test]
fn keeps_invalid_email_angle_autolinks_literal() {
    // 收集不能进入邮件链接交互链的代表性边界。
    let inputs = [
        // local-part 不能为空。
        "<@example.test>",
        // domain 不能为空。
        "<user@>",
        // local-part 不能以点号开始。
        "<.user@example.test>",
        // local-part 不能包含连续点号。
        "<user..tag@example.test>",
        // domain label 不能以连字符开始。
        "<user@-example.test>",
        // domain label 不能以连字符结束。
        "<user@example-.test>",
        // domain 不能包含空 label。
        "<user@example..test>",
        // domain 下划线不属于受支持字符。
        "<user@example_test>",
        // 多个 @ 不能形成唯一的 local-part 与 domain 边界。
        "<user@@example.test>",
        // 邮箱候选不能包含空白。
        "<user name@example.test>",
        // 未闭合的邮箱候选保持完整字面文本。
        "<user@example.test",
        // local-part 不能超过六十四个 ASCII 字节。
        "<aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa@example.test>",
        // domain label 不能超过六十三个 ASCII 字节。
        "<user@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.test>",
        // 首批契约不隐式接受国际化邮箱。
        "<用户@example.test>",
    ];
    // 逐项验证解析结果保持一个默认样式文本段。
    for input in inputs {
        // 解析当前非法邮箱候选。
        let segments = parse_rich_text(input);
        // 候选必须逐字保留，不能产生 Link 段。
        assert_eq!(
            // 传入实际解析结果。
            segments,
            // 构造单一普通文本段。
            vec![RichTextSegment::Text {
                // 保留完整尖括号输入。
                content: input.into(),
                // 使用默认文本样式。
                style: RichTextStyle::default(),
            }],
            // 失败时报告具体候选。
            "input: {input}",
        );
    }
}

// 验证歧义、不完整与非 HTTP(S) 候选保持字面文本。
#[test]
fn keeps_invalid_angle_autolinks_literal() {
    // 收集不能进入自动链接交互链的代表性输入。
    let inputs = [
        // 非 HTTP(S) scheme 不属于本批公开契约。
        "<ftp://example.test>",
        // URL 内含空白时保持字面文本。
        "<https://example test>",
        // scheme 后没有目标时不得创建空链接。
        "<https://>",
        // 未闭合尖括号不能吞掉后续文本。
        "<https://example.test",
    ];
    // 逐项验证解析结果只有一个默认样式文本段。
    for input in inputs {
        // 解析当前非法或不完整候选。
        let segments = parse_rich_text(input);
        // 候选必须逐字保留，不能产生 Link 段。
        assert_eq!(
            segments,
            vec![RichTextSegment::Text {
                content: input.into(),
                style: RichTextStyle::default(),
            }],
            "input: {input}"
        );
    }
}

// 验证反斜杠转义尖括号只输出字面 URL 与邮箱文本。
#[test]
fn escaped_angle_autolinks_remain_text() {
    // 转义两组开闭尖括号，阻止 URL 与邮箱进入自动链接解析。
    let segments = parse_rich_text(r"\<https://example.test\> \<user@example.test\>");
    // 转义符被解码，但两个候选仍合并为不可交互的普通文本。
    assert_eq!(
        // 传入实际解析结果。
        segments,
        // 构造单一默认样式文本段。
        vec![RichTextSegment::Text {
            // 保留去除转义符后的 URL、空格与邮箱。
            content: "<https://example.test> <user@example.test>".into(),
            // 使用默认文本样式。
            style: RichTextStyle::default(),
        }],
    );
}
