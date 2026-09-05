// 声明本文件只编译富文本使用文档，不启动窗口或原生资源。
#![allow(dead_code)]

// 隔离 rich-text-basic 围栏中的基础段组合。
mod rich_text_basic {
    // 引入文档承诺的富文本公开 prelude。
    use uix::prelude::*;

    // 编译混合文本、链接、换行与代码段的基础构建器。
    fn compile_example() {
        // 构造启用普通文字选择的富文本组件。
        let _rich_text = RichText::new()
            // 按文档顺序声明公开富文本段。
            .content(vec![
                // 添加继承默认样式的普通文本段。
                RichTextSegment::Text {
                    // 保存普通文本内容。
                    content: "查看 ".into(),
                    // 使用公开默认富文本样式。
                    style: RichTextStyle::default(),
                },
                // 添加可提交 URL 的链接段。
                RichTextSegment::Link {
                    // 保存链接显示文本。
                    content: "使用指南".into(),
                    // 保存链接导航目标。
                    url: "https://example.test/guide".into(),
                },
                // 强制后续代码段换行。
                RichTextSegment::NewLine,
                // 添加公开内联代码段。
                RichTextSegment::Code {
                    // 保存代码内容。
                    content: "cargo run".into(),
                },
            ])
            // 显式开启普通文字选择能力。
            .selectable(true)
            // 声明默认逻辑字号。
            .font_size(14.0);
    }
}

// 隔离 rich-text-styles 围栏中的段级样式覆盖。
mod rich_text_styles {
    // 引入文档承诺的富文本与颜色公开 prelude。
    use uix::prelude::*;

    // 编译粗体、颜色与删除线的段级样式组合。
    fn compile_example() {
        // 构造包含两个独立样式段的富文本组件。
        let _rich_text = RichText::new().content(vec![
            // 添加粗体文本段。
            RichTextSegment::Text {
                // 保存粗体展示文本。
                content: "粗体".into(),
                // 只覆盖粗体字段并继承其余默认值。
                style: RichTextStyle {
                    // 开启粗体。
                    bold: true,
                    // 继承其余公开样式字段。
                    ..Default::default()
                },
            },
            // 添加彩色删除线文本段。
            RichTextSegment::Text {
                // 保存删除线展示文本。
                content: "红色删除线".into(),
                // 组合颜色与删除线覆盖。
                style: RichTextStyle {
                    // 使用公开 RGB 构造器声明文字颜色。
                    color: Some(Color::from_rgb(220, 50, 50)),
                    // 开启删除线。
                    strikethrough: true,
                    // 继承其余公开样式字段。
                    ..Default::default()
                },
            },
        ]);
    }
}

// 隔离 rich-text-parse 围栏中的 Markdown 解析入口。
mod rich_text_parse {
    // 引入文档承诺的富文本解析公开 prelude。
    use uix::prelude::*;

    // 编译 Markdown 到富文本段再到组件的公开数据流。
    fn compile_example() {
        // 解析带粗体、代码与链接的 Markdown 文本。
        let segments = parse_rich_text(
            // 提供不依赖外部资源的固定 Markdown 输入。
            "**运行** `cargo build`，查看 [使用指南](https://example.test/guide)。",
        );
        // 把解析结果交给统一 RichText 组件。
        let _rich_text = RichText::new().content(segments);
    }
}

// 隔离 rich-text-layout 围栏中的外部测量入口。
mod rich_text_layout {
    // 引入文档承诺的富文本布局公开 prelude。
    use uix::prelude::*;

    // 编译非组件消费者使用的富文本测量函数。
    fn compile_example() {
        // 解析需要测量的 Markdown 内容。
        let segments = parse_rich_text("查看 `cargo test` 的输出");
        // 使用公开布局函数取得高度、字符数与最大行宽。
        let (_height, _char_count, _max_width) =
            // 在固定宽度、字号和颜色下执行纯布局计算。
            layout_rich_text_segments(&segments, 320.0, 14.0, Color::black());
    }
}

// 隔离 rich-text-on-link 围栏中的应用导航适配器。
mod rich_text_on_link {
    // 引入文档承诺的富文本与嵌入公开 prelude。
    use uix::prelude::*;

    // 提供代表应用自有内部路由策略的最小适配器。
    fn open_internal(url: &str) {
        // 消费公开回调传入的 URL，避免引入真实导航副作用。
        let _url = url;
    }

    // 编译 Markdown、RichText 与链接回调的公开组合。
    fn compile_example(markdown: &str) -> ViewNode {
        // 从 Markdown 构建统一富文本段列表。
        let segments = parse_rich_text(markdown);

        // 把富文本组件嵌入公开视图节点。
        embed(
            // 构造消费解析结果的富文本组件。
            RichText::new()
                // 提交拥有型富文本段。
                .content(segments)
                // 声明默认逻辑字号。
                .font_size(14.0)
                // 把链接提交委托给应用导航策略。
                .on_link(|url| {
                    // 调用最小内部路由适配器。
                    open_internal(url);
                }),
        )
    }
}
