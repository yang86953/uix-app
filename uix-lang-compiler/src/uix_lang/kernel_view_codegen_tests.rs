// 引入解析与核心生成入口。
use super::{generate_view, parse_document, Diagnostic};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证基础 View 表达式按所有权进入 UIX 组合树。
#[test]
fn generates_kernel_view_expression_without_wrapper() {
    // 使用普通 Rust 函数调用构造基础 View。
    let snapshot = generate(r#"<KernelView value={build_window_control()} />"#)
        // 合法框架桥接必须生成。
        .expect("KernelView 应接受 Rust View 表达式");
    // 生成物必须保留调用且只通过统一 View trait 物化一次。
    assert!(snapshot.contains("build_window_control"));
    // 桥接不能生成额外 Container 或克隆基础 View。
    assert!(!snapshot.contains("Container :: new") && !snapshot.contains("clone"));
    // 通用 View 样式允许由 UIX 外壳拥有。
    let styled = generate(r#"<KernelView value={build()} width="20px" />"#)
        .expect("KernelView 应允许通用 View 样式");
    // 宽度必须应用在基础 View 返回值上。
    assert!(styled.contains("width (20.0)"));

    // 条件分支必须把基础 View 构造调用保留在分支内部。
    let conditional = generate(
        r#"<Container><If {visible}><KernelView value={build_window_control()} /></If></Container>"#,
    )
    .expect("条件 KernelView 应生成");
    // 构造调用只能出现在条件判断之后，避免隐藏分支提前分配。
    assert!(
        conditional.find("if visible").expect("应生成条件")
            < conditional
                .find("build_window_control")
                .expect("应保留基础 View 构造")
    );
}

// 验证 KernelHost 把唯一 UIX 展示子树交给 Rust 基础内核函数。
#[test]
fn generates_kernel_host_with_single_uix_child() {
    // UIX 完整拥有图标名称、尺寸与宿主通用样式。
    let snapshot = generate(
        r#"<KernelHost value={build_control} width="46px"><Icon name="minus" size="14px" /></KernelHost>"#,
    )
    .expect("KernelHost 应接收一个 UIX 展示根");
    // Rust 函数项必须以生成后的 Icon ViewNode 为唯一参数。
    assert!(snapshot.contains("build_control") && snapshot.contains("Icon :: new (\"minus\")"));
    // UIX 静态尺寸必须应用到宿主返回的基础 View。
    assert!(snapshot.contains("width (46.0)"));
}

// 验证 KernelChildren 把拥有型 ViewNode 列表无克隆交给 UIX 容器。
#[test]
fn generates_kernel_children_without_clone_or_wrapper() {
    // 外层容器拥有排列，Rust 只交付已完成行为配置的节点列表。
    let snapshot = generate(
        r#"<Container direction="row" gap="0px"><KernelChildren value={positioned_buttons} /></Container>"#,
    )
    .expect("KernelChildren 应接收拥有型 ViewNode 列表");
    // 生成物必须直接消费列表并追加到容器子项。
    assert!(snapshot.contains("extend") && snapshot.contains("positioned_buttons"));
    // 列表桥接不得因语言迁移引入无条件克隆或额外容器。
    assert!(!snapshot.contains("clone"));
}

// 验证桥接元素拒绝第二套属性与子树所有权。
#[test]
fn rejects_invalid_kernel_view_shapes() {
    // 缺失 value 时不能生成空占位。
    let missing = generate(r#"<KernelView />"#).expect_err("缺失 value 必须失败");
    // 诊断必须点名必需属性。
    assert!(missing.message.contains("缺少 value"));
    // 字符串不具备 Rust View 所有权。
    let literal = generate(r#"<KernelView value="button" />"#).expect_err("字面量 value 必须失败");
    // 诊断必须说明 Rust View 表达式边界。
    assert!(literal.message.contains("Rust 基础内核表达式"));
    // 可见子节点会形成冲突的第二棵子树。
    let child = generate(r#"<KernelView value={build()}><Text>lost</Text></KernelView>"#)
        .expect_err("KernelView 子节点必须失败");
    // 诊断必须明确拒绝子节点。
    assert!(child.message.contains("不接受子节点"));
    // 组件专有属性不能穿透框架内部桥接。
    let attribute = generate(r#"<KernelView value={build()} mystery="value" />"#)
        .expect_err("KernelView 未登记属性必须失败");
    // 统一未知属性诊断必须保留实际属性名。
    assert!(attribute.message.contains("mystery"));
    // 事件语义必须继续由 Rust 基础内核单独拥有。
    let event = generate(r#"<KernelView value={build()} @click="save()" />"#)
        .expect_err("KernelView 事件必须失败");
    // 诊断必须点名事件所有权冲突。
    assert!(event.message.contains("不接受事件"));

    // KernelHost 缺少展示子树时不能调用基础内核。
    let host_missing =
        generate(r#"<KernelHost value={build_host} />"#).expect_err("KernelHost 缺少子树必须失败");
    // 诊断必须说明唯一展示根要求。
    assert!(host_missing.message.contains("必须包含一个"));
    // 多个展示根不能被桥接层隐式包裹。
    let host_multiple = generate(
        r#"<KernelHost value={build_host}><Icon name="a" /><Icon name="b" /></KernelHost>"#,
    )
    .expect_err("KernelHost 多子树必须失败");
    // 诊断必须引导显式布局。
    assert!(host_multiple.message.contains("只能包含一个"));

    // KernelChildren 必须保持仅 value 的多根交接边界。
    let children_missing = generate(r#"<Container><KernelChildren /></Container>"#)
        .expect_err("KernelChildren 缺少 value 必须失败");
    // 缺失诊断必须指明 value。
    assert!(children_missing.message.contains("缺少 value"));
    // 列表桥接不得带通用样式，样式归外层 UIX 容器。
    let children_attribute =
        generate(r#"<Container><KernelChildren value={nodes} class="rows" /></Container>"#)
            .expect_err("KernelChildren 样式属性必须失败");
    // 诊断必须保留越界属性名。
    assert!(children_attribute.message.contains("class"));
    // 列表交接不得同时接收声明子节点。
    let children_child = generate(
        r#"<Container><KernelChildren value={nodes}><Icon name="x" /></KernelChildren></Container>"#,
    )
    .expect_err("KernelChildren 子节点必须失败");
    // 诊断必须说明子树所有权冲突。
    assert!(children_child.message.contains("不接受子节点"));
    // 列表桥接不能脱离外层布局独立生成 View。
    let children_root =
        generate(r#"<KernelChildren value={nodes} />"#).expect_err("KernelChildren 独立根必须失败");
    // 诊断必须引导放入容器。
    assert!(children_root.message.contains("不能作为独立 View 根节点"));
}
