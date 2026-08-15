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

// 验证 Tooltip 动态文字、方向、触发方式、唯一子树与公共属性的完整生成契约。
#[test]
// 声明完整 Tooltip 生成测试。
fn generates_tooltip_contract() {
    // 生成覆盖动态文字、右侧位置、焦点触发、按钮、宽度与自动化身份的文字提示。
    let snapshot = generate(r#"<Tooltip text={tooltip_text} placement="right" trigger="focus" width="240px" automationId="help-tooltip"><Button>悬停</Button></Tooltip>"#).expect("文档属性应映射到公开 Tooltip API");
    // 动态文字必须以临时借用进入会复制内容的构造器。
    assert!(snapshot.contains("Tooltip :: new (& * (tooltip_text))"));
    // 右侧关键字必须映射到公开位置枚举。
    assert!(snapshot.contains("TooltipPlacement :: Right"));
    // 焦点关键字必须映射到公开触发枚举。
    assert!(snapshot.contains("TriggerMode :: Focus"));
    // Tooltip 必须使用容器 ViewNode 承载真实触发子树。
    assert!(snapshot.contains("ViewNode :: new") && snapshot.contains("button"));
    // 公共宽度与自动化标识仍由公共属性层消费。
    assert!(snapshot.contains("width (240.0)") && snapshot.contains("automation_id"));
}

// 验证一个静态触发容器可以在内部保留普通控制流。
#[test]
// 声明嵌套控制流生成测试。
fn allows_control_flow_inside_static_trigger_view() {
    // 唯一直接 Container 内部使用条件渲染。
    let snapshot = generate(r#"<Tooltip text="详情"><Container><If {show_more}><Text>更多</Text></If></Container></Tooltip>"#).expect("静态触发容器内部应保留普通控制流");
    // 唯一直接触发器必须生成容器 View。
    assert!(snapshot.contains("prelude :: column"));
    // 内部条件必须保留为 Rust 控制流。
    assert!(snapshot.contains("if show_more"));
    // 省略专有枚举时必须沿用运行时 Top 与 Hover 默认值。
    assert!(!snapshot.contains("TooltipPlacement") && !snapshot.contains("TriggerMode"));
}

// 验证 Tooltip 全部方向与触发关键字具有确定公开枚举映射。
#[test]
// 声明 Tooltip 枚举全覆盖测试。
fn maps_all_tooltip_placements_and_triggers() {
    // 保存四个方向关键字及其公开变体。
    let placements = [
        // 上方。
        ("top", "Top"),
        // 下方。
        ("bottom", "Bottom"),
        // 左侧。
        ("left", "Left"),
        // 右侧。
        ("right", "Right"),
    ];
    // 逐项核验方向映射。
    for (keyword, variant) in placements {
        // 构造只改变 placement 的最小合法 Tooltip。
        let source = format!(
            // 保持唯一静态触发 View。
            "<Tooltip text=\"提示\" placement=\"{keyword}\"><Button>查看</Button></Tooltip>"
        );
        // 每个登记方向都必须成功生成。
        let snapshot = generate(&source).expect("登记的 Tooltip placement 应成功生成");
        // 目标枚举变体必须精确出现。
        assert!(snapshot.contains(&format!("TooltipPlacement :: {variant}")));
    }
    // 保存四种触发关键字及其公开变体。
    let triggers = [
        // 悬停。
        ("hover", "Hover"),
        // 点击。
        ("click", "Click"),
        // 焦点。
        ("focus", "Focus"),
        // 上下文菜单。
        ("contextMenu", "ContextMenu"),
    ];
    // 逐项核验触发方式映射。
    for (keyword, variant) in triggers {
        // 构造只改变 trigger 的最小合法 Tooltip。
        let source = format!(
            // 保持唯一静态触发 View。
            "<Tooltip text=\"提示\" trigger=\"{keyword}\"><Button>查看</Button></Tooltip>"
        );
        // 每个登记触发方式都必须成功生成。
        let snapshot = generate(&source).expect("登记的 Tooltip trigger 应成功生成");
        // 目标枚举变体必须精确出现。
        assert!(snapshot.contains(&format!("TriggerMode :: {variant}")));
    }
}

// 验证 Tooltip 必需文字与直接触发 View 的静态基数诊断。
#[test]
// 声明 Tooltip 核心错误测试。
fn rejects_invalid_tooltip_core_contracts() {
    // 缺失 text 时没有运行时提示内容来源。
    let missing =
        generate(r#"<Tooltip><Button>悬停</Button></Tooltip>"#).expect_err("缺少 text 必须被拒绝");
    // 诊断必须点名 text。
    assert!(missing.message.contains("text"));
    // 空 Tooltip 没有触发 View。
    let empty = generate(r#"<Tooltip text="提示" />"#).expect_err("空 Tooltip 必须被拒绝");
    // 诊断必须点明唯一直接触发 View。
    assert!(empty.message.contains("仅包含一个直接触发 View"));
    // 多个直接子节点会破坏唯一触发器身份。
    let multiple =
        generate(r#"<Tooltip text="提示"><Button>一</Button><Button>二</Button></Tooltip>"#)
            .expect_err("多个直接触发 View 必须被拒绝");
    // 多子节点沿用相同静态基数诊断。
    assert!(multiple.message.contains("仅包含一个直接触发 View"));
}

// 验证直接动态基数与非法专有枚举不会静默降级。
#[test]
// 声明 Tooltip 边界错误测试。
fn rejects_dynamic_trigger_and_unregistered_attributes() {
    // 直接 If 会令触发 View 是否存在依赖运行时条件。
    let dynamic =
        generate(r#"<Tooltip text="提示"><If {show}><Button>悬停</Button></If></Tooltip>"#)
            .expect_err("直接 If 触发器必须被拒绝");
    // 诊断必须点明直接控制流边界。
    assert!(dynamic.message.contains("不能是 If 或 For"));
    // 直接 For 会令触发 View 数量依赖运行时集合长度。
    let repeated = generate(
        r#"<Tooltip text="提示"><For {item} in {items}><Button>{item}</Button></For></Tooltip>"#,
    )
    .expect_err("直接 For 触发器必须被拒绝");
    // 循环触发器必须沿用相同动态基数诊断。
    assert!(repeated.message.contains("不能是 If 或 For"));
    // 动态 placement 不能在编译期选择枚举变体。
    let dynamic_placement =
        generate(r#"<Tooltip text="提示" placement={side}><Button>悬停</Button></Tooltip>"#)
            // 表达式方向必须失败。
            .expect_err("动态 placement 必须被拒绝");
    // 诊断必须点明字符串字面量约束。
    assert!(dynamic_placement.message.contains("字符串字面量"));
    // 未知方向不能静默回退到 Top。
    let placement =
        generate(r#"<Tooltip text="提示" placement="center"><Button>悬停</Button></Tooltip>"#)
            .expect_err("未知 placement 必须被拒绝");
    // 诊断必须包含非法值与合法集合。
    assert!(placement.message.contains("center") && placement.suggestion.contains("right"));
    // 动态 trigger 不能在编译期选择枚举变体。
    let dynamic_trigger =
        generate(r#"<Tooltip text="提示" trigger={mode}><Button>悬停</Button></Tooltip>"#)
            // 表达式触发方式必须失败。
            .expect_err("动态 trigger 必须被拒绝");
    // 诊断必须点明字符串字面量约束。
    assert!(dynamic_trigger.message.contains("字符串字面量"));
    // 未知触发方式不能静默回退到 Hover。
    let trigger =
        generate(r#"<Tooltip text="提示" trigger="press"><Button>悬停</Button></Tooltip>"#)
            .expect_err("未知 trigger 必须被拒绝");
    // 诊断必须包含非法值与合法集合。
    assert!(trigger.message.contains("press") && trigger.suggestion.contains("contextMenu"));
    // Tooltip 当前没有专有事件映射。
    let event = generate(r#"<Tooltip text="提示" @open="on_open"><Button>悬停</Button></Tooltip>"#)
        .expect_err("未登记事件必须被拒绝");
    // 未知事件诊断必须包含具体事件名。
    assert!(event.message.contains("@open"));
}
