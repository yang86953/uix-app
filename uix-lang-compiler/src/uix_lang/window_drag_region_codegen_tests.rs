// 引入待验证的文档解析与 View 生成入口。
use super::{generate_view, parse_document};

// 解析单根 UIX 文档并返回稳定令牌文本。
fn generate(source: &str) -> Result<String, super::Diagnostic> {
    // 解析输入文档。
    let document = parse_document(source)?;
    // 生成根 View 令牌。
    let tokens = generate_view(&document.root)?;
    // 使用过程宏令牌的稳定空白格式返回快照。
    Ok(tokens.to_string())
}

// 验证拖拽区公开映射、形状约束与交互所有权诊断。
#[test]
fn generates_and_validates_window_drag_region_contract() {
    // 构造与 Demo 相同的非交互标题内容和公共布局属性。
    let tokens = generate(
        // 保持窗口控件位于拖拽区之外，本测试只生成拖拽包装器本身。
        r#"<WindowDragRegion flexGrow={1} automationId="title-drag"><Container direction="row"><Icon name="box" /><Text>UIX Demo</Text></Container></WindowDragRegion>"#,
    )
    // 文档化 WindowDragRegion 必须成功生成。
    .expect("WindowDragRegion 文档契约应生成 Rust View");
    // 必须复用公开 window_chrome 拖拽包装器。
    assert!(tokens.contains("window_drag_region"));
    // 唯一内容子树必须完整保留。
    assert!(tokens.contains("Icon :: new") && tokens.contains("UIX Demo"));
    // 公共布局与自动化属性必须继续映射。
    assert!(tokens.contains("flex_grow (1") && tokens.contains("automation_id"));
    // 生成物不得持有平台窗口或复制窗口动作。
    assert!(!tokens.contains("PlatformWindow") && !tokens.contains("WindowAction"));

    // 空拖拽区没有可建立的命中范围。
    let empty = generate(r#"<WindowDragRegion />"#)
        // 提取预期缺失内容诊断。
        .expect_err("空 WindowDragRegion 必须失败");
    // 诊断必须说明唯一可渲染子节点要求。
    assert!(empty.message.contains("一个可渲染直接子节点"));

    // 多个直接子节点不能由宏静默选择布局。
    let multiple = generate(
        // 构造两个直接展示子节点。
        r#"<WindowDragRegion><Text>A</Text><Text>B</Text></WindowDragRegion>"#,
    )
    // 提取预期多子节点诊断。
    .expect_err("多子节点 WindowDragRegion 必须失败");
    // 修复建议必须要求显式容器。
    assert!(multiple.message.contains("只能包含一个") && multiple.suggestion.contains("Container"));

    // 点击事件会与拖拽区拥有的指针手势冲突。
    let event = generate(r#"<WindowDragRegion @click="save()"><Text>A</Text></WindowDragRegion>"#)
        // 提取预期交互所有权诊断。
        .expect_err("WindowDragRegion 点击事件必须失败");
    // 诊断必须引导把交互控件移出拖拽区。
    assert!(event.message.contains("不接受事件") && event.suggestion.contains("同级节点"));

    // 未登记普通属性必须继续走统一拒绝路径。
    let unknown =
        generate(r#"<WindowDragRegion mystery="value"><Text>A</Text></WindowDragRegion>"#)
            // 提取预期未知属性诊断。
            .expect_err("WindowDragRegion 未登记属性必须失败");
    // 诊断必须保留具体属性名。
    assert!(unknown.message.contains("mystery"));
}
