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

// 验证 Popover 文本、定位、箭头、受控打开、触发方式与公共属性的完整生成契约。
#[test]
// 声明完整 Popover 生成测试。
fn generates_popover_contract() {
    // 生成覆盖动态内容、标题、方向、箭头、焦点触发、状态与自动化身份的气泡卡片。
    let snapshot = generate(r#"<Popover content={details} title={heading} placement="bottomRight" arrow="false" trigger="focus" open={popover_open} width="240px" automationId="details-popover"><Button>查看</Button></Popover>"#).expect("文档属性应映射到公开 Popover API");
    // 动态内容必须以临时借用进入会复制内容的构造器。
    assert!(snapshot.contains("Popover :: new (& * (details))"));
    // 动态标题必须进入公开 title 构建器。
    assert!(snapshot.contains("title (& * (heading))"));
    // 右下方向必须映射到公开运行时枚举。
    assert!(snapshot.contains("PopoverPlacement :: BottomRight"));
    // 静态 false 必须关闭箭头。
    assert!(snapshot.contains("arrow (false)"));
    // 焦点关键字必须映射到公开运行时枚举。
    assert!(snapshot.contains("PopoverTrigger :: Focus"));
    // 外部状态必须以句柄借用进入受控打开构建器。
    assert!(snapshot.contains("controlled_open (& (popover_open))"));
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
    // 未声明 placement 时必须沿用运行时 Top 默认值。
    assert!(!snapshot.contains("PopoverPlacement"));
    // 未声明 open 时不得伪造受控状态句柄。
    assert!(!snapshot.contains("controlled_open"));
    // 唯一静态容器必须进入触发子树。
    assert!(snapshot.contains("trigger_view") && snapshot.contains("prelude :: column"));
    // 内部条件必须保留为 Rust 控制流。
    assert!(snapshot.contains("if show_more"));
}

// 验证十二种 Popover 放置关键字都具有确定公开枚举映射。
#[test]
// 声明放置方向全覆盖测试。
fn maps_all_popover_placements() {
    // 保存文档关键字与公开枚举变体的一一对应表。
    let cases = [
        // 上方居中。
        ("top", "Top"),
        // 上方左对齐。
        ("topLeft", "TopLeft"),
        // 上方右对齐。
        ("topRight", "TopRight"),
        // 下方居中。
        ("bottom", "Bottom"),
        // 下方左对齐。
        ("bottomLeft", "BottomLeft"),
        // 下方右对齐。
        ("bottomRight", "BottomRight"),
        // 左侧居中。
        ("left", "Left"),
        // 左侧顶部对齐。
        ("leftTop", "LeftTop"),
        // 左侧底部对齐。
        ("leftBottom", "LeftBottom"),
        // 右侧居中。
        ("right", "Right"),
        // 右侧顶部对齐。
        ("rightTop", "RightTop"),
        // 右侧底部对齐。
        ("rightBottom", "RightBottom"),
    ];
    // 逐项核验不会串接或遗漏方向。
    for (keyword, variant) in cases {
        // 构造只改变 placement 的最小合法 Popover。
        let source = format!(
            // 保持唯一静态触发 View。
            "<Popover content=\"详情\" placement=\"{keyword}\"><Button>查看</Button></Popover>"
        );
        // 每个登记关键字都必须成功生成。
        let snapshot = generate(&source).expect("登记的 Popover placement 应成功生成");
        // 目标枚举变体必须精确出现在生成代码中。
        assert!(snapshot.contains(&format!("PopoverPlacement :: {variant}")));
    }
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
    // 未知触发方式不能静默回退到 click。
    let context =
        generate(r#"<Popover content="详情" trigger="context"><Button>查看</Button></Popover>"#)
            .expect_err("未知 trigger 必须被拒绝");
    // 非法关键字诊断必须包含合法集合。
    assert!(context.message.contains("不受支持") && context.suggestion.contains("focus"));
}

// 验证动态枚举、未知方向与非 State 打开值继续诊断。
#[test]
// 声明 Popover 配置属性错误测试。
fn rejects_invalid_popover_configuration() {
    // trigger 必须在编译期选择枚举变体。
    let dynamic_trigger =
        generate(r#"<Popover content="详情" trigger={mode}><Button>查看</Button></Popover>"#)
            .expect_err("表达式 trigger 必须被拒绝");
    // 诊断必须点明字符串字面量约束。
    assert!(dynamic_trigger.message.contains("字符串字面量"));
    // placement 表达式不能在编译期选择枚举变体。
    let dynamic_placement =
        generate(r#"<Popover content="详情" placement={side}><Button>查看</Button></Popover>"#)
            // 动态方向必须失败。
            .expect_err("表达式 placement 必须被拒绝");
    // 诊断必须点明字符串字面量约束。
    assert!(dynamic_placement.message.contains("字符串字面量"));
    // 未知方向不能静默回退到 Top。
    let placement =
        generate(r#"<Popover content="详情" placement="center"><Button>查看</Button></Popover>"#)
            // 未知方向必须失败。
            .expect_err("未知 placement 必须被拒绝");
    // 诊断必须包含非法值与合法集合。
    assert!(placement.message.contains("center") && placement.suggestion.contains("rightBottom"));
    // 布尔字面量不能提供可订阅和写回的 State 句柄。
    let open = generate(r#"<Popover content="详情" open><Button>查看</Button></Popover>"#)
        .expect_err("字面量 open 必须被拒绝");
    // 诊断必须明确 State<bool> 要求。
    assert!(open.message.contains("State<bool>"));
    // 文档外的箭头布尔值不能被猜测解释。
    let arrow = generate(r#"<Popover content="详情" arrow="yes"><Button>查看</Button></Popover>"#)
        // 非法布尔值必须失败。
        .expect_err("非法 arrow 必须被拒绝");
    // 诊断必须说明布尔值要求。
    assert!(arrow.message.contains("布尔值"));
    // Popover 当前没有专有 open 事件映射。
    let event =
        generate(r#"<Popover content="详情" @open="on_open"><Button>查看</Button></Popover>"#)
            .expect_err("未登记事件必须被拒绝");
    // 未知事件诊断必须包含具体事件名。
    assert!(event.message.contains("@open"));
}
