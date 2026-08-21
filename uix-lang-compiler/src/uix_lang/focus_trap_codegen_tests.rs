// 引入与公开宏一致的完整文档测试生成入口。
use super::generate_test_document_view as generate;

// 验证 FocusTrap 有序子树与公共属性的完整生成契约。
#[test]
// 声明完整 FocusTrap 生成测试。
fn generates_focus_trap_contract() {
    // 生成覆盖动态启用、按钮、尺寸与自动化身份的焦点作用域。
    let snapshot = generate(r#"<FocusTrap active={trap_active} width="320px" automationId="dialog-actions"><Button>确定</Button><Button>取消</Button></FocusTrap>"#).expect("焦点作用域应映射到公开 FocusTrap API");
    // 焦点作用域必须使用默认启用的公开构造器。
    assert!(snapshot.contains("FocusTrap :: new ()"));
    // 动态启用事实必须进入公开 active 构建器。
    assert!(snapshot.contains("active (trap_active)"));
    // 容器必须保留两个源码有序按钮子节点。
    assert!(snapshot.matches("prelude :: button").count() == 2);
    // 公共宽度与自动化标识仍由公共属性层消费。
    assert!(snapshot.contains("width (320.0)") && snapshot.contains("automation_id"));
}

// 验证 FocusTrap 保留动态子树控制流且允许空作用域声明。
#[test]
// 声明 FocusTrap 子树形状测试。
fn preserves_control_flow_and_empty_scope() {
    // 生成包含条件项与循环项的动态焦点作用域。
    let dynamic = generate(r#"<FocusTrap><If {show_primary}><Button>主要</Button></If><For {action} in {actions}><Button>{action}</Button></For></FocusTrap>"#).expect("焦点作用域应保留普通控制流");
    // 条件分支必须保留为 Rust if。
    assert!(dynamic.contains("if show_primary"));
    // 循环分支必须保留内部位置枚举与作者绑定。
    assert!(
        dynamic.contains("__uix_for_ordinal")
            && dynamic.contains("enumerate")
            && dynamic.contains("action")
    );
    // 空作用域不应由编译层猜测运行时可聚焦性。
    let empty = generate(r#"<FocusTrap />"#).expect("空焦点作用域应保持可构造");
    // 空作用域仍必须声明稳定 FocusTrap 身份。
    assert!(empty.contains("FocusTrap :: new ()"));
    // 省略 active 时必须沿用运行时默认启用状态。
    assert!(!empty.contains(". active"));
}

// 验证非法 active 与未登记专有事件不会静默暴露。
#[test]
// 声明 FocusTrap 属性与事件拒绝测试。
fn rejects_unregistered_focus_trap_contracts() {
    // 文档外的布尔值不能被猜测解释。
    let active = generate(r#"<FocusTrap active="yes"><Button>确定</Button></FocusTrap>"#)
        .expect_err("非法 active 必须被拒绝");
    // 诊断必须说明布尔值要求。
    assert!(active.message.contains("布尔值"));
    // 未登记属性仍不能穿过公共属性层。
    let unknown = generate(r#"<FocusTrap enabled><Button>确定</Button></FocusTrap>"#)
        .expect_err("未知 FocusTrap 属性必须被拒绝");
    // 未知属性诊断必须包含具体属性名。
    assert!(unknown.message.contains("enabled"));
    // FocusTrap 当前没有专有 open 事件。
    let event = generate(r#"<FocusTrap @open="on_open"><Button>确定</Button></FocusTrap>"#)
        .expect_err("未登记 @open 事件必须被拒绝");
    // 未知事件诊断必须包含具体事件名。
    assert!(event.message.contains("@open"));
}
