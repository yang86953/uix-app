// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
    // 结束测试生成入口。
}

// 验证 Popover 动态内容、悬停触发、唯一子树与公共属性的完整生成契约。
#[test]
// 声明完整 Popover 生成测试。
fn generates_popover_contract() {
    // 生成覆盖动态内容、悬停触发、按钮子树与自动化身份的气泡卡片。
    let snapshot = generate(r#"<Popover content={details} trigger="hover" width="240px" automationId="details-popover"><Button>查看</Button></Popover>"#).expect("文档属性应映射到公开 Popover API");
    // 动态内容必须以临时借用进入会复制内容的构造器。
    assert!(snapshot.contains("Popover :: new (& * (details))"));
    // 悬停关键字必须映射到公开运行时枚举。
    assert!(snapshot.contains("PopoverTrigger :: Hover"));
    // 唯一按钮必须进入运行时 trigger_view 子树提供器。
    assert!(snapshot.contains("trigger_view") && snapshot.contains("button"));
    // 公共宽度与自动化标识仍由公共属性层消费。
    assert!(snapshot.contains("width (240.0)") && snapshot.contains("automation_id"));
}

// 验证缺省点击语义与静态容器内部控制流。
#[test]
// 声明缺省触发和嵌套控制流测试。
fn preserves_default_click_and_nested_control_flow() {
    // 唯一静态 Container 内部使用条件渲染。
    let snapshot = generate(r#"<Popover content="详情"><Container><If {show_more}><Text>更多</Text></If></Container></Popover>"#).expect("静态触发容器内部应保留普通控制流");
    // 未声明 trigger 时必须沿用运行时 Click 默认值，不额外写入枚举。
    assert!(!snapshot.contains("PopoverTrigger"));
    // 唯一静态容器必须进入触发子树。
    assert!(snapshot.contains("trigger_view") && snapshot.contains("prelude :: column"));
    // 内部条件必须保留为 Rust 控制流。
    assert!(snapshot.contains("if show_more"));
}

// 验证 Popover 必需内容与直接触发 View 的静态基数诊断。
#[test]
// 声明 Popover 核心错误测试。
fn rejects_invalid_popover_core_contracts() {
    // 缺失 content 时没有运行时气泡内容来源。
    let missing = generate(r#"<Popover><Button>查看</Button></Popover>"#)
        .expect_err("缺少 content 必须被拒绝");
    // 诊断必须点名 content。
    assert!(missing.message.contains("content"));
    // 空 Popover 没有触发 View。
    let empty = generate(r#"<Popover content="详情" />"#).expect_err("空 Popover 必须被拒绝");
    // 诊断必须点明唯一直接触发 View。
    assert!(empty.message.contains("仅包含一个直接触发 View"));
    // 多个直接子节点会破坏唯一触发器身份。
    let multiple =
        generate(r#"<Popover content="详情"><Button>一</Button><Button>二</Button></Popover>"#)
            .expect_err("多个直接触发 View 必须被拒绝");
    // 多子节点沿用相同静态基数诊断。
    assert!(multiple.message.contains("仅包含一个直接触发 View"));
}

// 验证动态基数、裸内容与非法触发方式不会静默降级。
#[test]
// 声明 Popover 触发器边界错误测试。
fn rejects_unstable_trigger_shapes_and_modes() {
    // 直接 If 会令触发 View 是否存在依赖运行时条件。
    let dynamic =
        generate(r#"<Popover content="详情"><If {show}><Button>查看</Button></If></Popover>"#)
            .expect_err("直接 If 触发器必须被拒绝");
    // 诊断必须点明直接控制流边界。
    assert!(dynamic.message.contains("不能是 If 或 For"));
    // 直接 For 会令触发 View 数量依赖运行时集合长度。
    let repeated = generate(
        r#"<Popover content="详情"><For {item} in {items}><Button>{item}</Button></For></Popover>"#,
    )
    .expect_err("直接 For 触发器必须被拒绝");
    // 循环触发器必须沿用相同动态基数诊断。
    assert!(repeated.message.contains("不能是 If 或 For"));
    // 可见文本没有显式交互组件身份。
    let text = generate(r#"<Popover content="详情">查看</Popover>"#)
        .expect_err("可见文本触发器必须被拒绝");
    // 诊断必须点明可见文本边界。
    assert!(text.message.contains("可见文本"));
    // 直接插值同样没有稳定组件身份。
    let interpolation =
        generate(r#"<Popover content="详情">{label}</Popover>"#).expect_err("插值触发器必须被拒绝");
    // 诊断必须点明插值边界。
    assert!(interpolation.message.contains("插值"));
    // 文档只登记 click 与 hover。
    let focus =
        generate(r#"<Popover content="详情" trigger="focus"><Button>查看</Button></Popover>"#)
            .expect_err("未登记 focus 必须被拒绝");
    // 非法关键字诊断必须包含合法集合。
    assert!(focus.message.contains("不受支持") && focus.suggestion.contains("click 或 hover"));
}

// 验证表达式触发方式与未登记运行时属性继续诊断。
#[test]
// 声明 Popover 未登记属性错误测试。
fn rejects_non_literal_trigger_and_unregistered_attributes() {
    // trigger 必须在编译期选择枚举变体。
    let dynamic_trigger =
        generate(r#"<Popover content="详情" trigger={mode}><Button>查看</Button></Popover>"#)
            .expect_err("表达式 trigger 必须被拒绝");
    // 诊断必须点明字符串字面量约束。
    assert!(dynamic_trigger.message.contains("字符串字面量"));
    // 文档尚未登记 placement，不能静默暴露运行时构建器。
    let placement =
        generate(r#"<Popover content="详情" placement="bottom"><Button>查看</Button></Popover>"#)
            .expect_err("未登记 placement 必须被拒绝");
    // 未知属性诊断必须包含具体属性名。
    assert!(placement.message.contains("placement"));
    // Popover 当前没有专有 open 事件映射。
    let event =
        generate(r#"<Popover content="详情" @open="on_open"><Button>查看</Button></Popover>"#)
            .expect_err("未登记事件必须被拒绝");
    // 未知事件诊断必须包含具体事件名。
    assert!(event.message.contains("@open"));
}
