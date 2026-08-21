// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证标题、六向配置、同步回调与真实 trigger 的完整生成契约。
#[test]
fn generates_popconfirm_contract() {
    // 生成覆盖专有配置、事件和公共身份的确认气泡。
    let snapshot = generate(r#"<Popconfirm title={confirm_title} confirmText="确认" cancelText="返回" placement="bottomRight" arrow={show_arrow} icon automationId="delete-confirm" @confirm="confirm_delete" @cancel="cancel_delete"><Button type="danger">删除这个很长的项目</Button></Popconfirm>"#).expect("文档属性应映射到公开 Popconfirm API");
    // 动态标题与按钮文字必须进入运行时复制构建器。
    assert!(
        snapshot.contains("Popconfirm :: new")
            && snapshot.contains("title (confirm_title)")
            && snapshot.contains("confirm_text")
            && snapshot.contains("cancel_text"),
        "{snapshot}"
    );
    // 唯一真实 trigger 必须由运行时 owner 接收。
    assert!(
        snapshot.contains("trigger_view") && snapshot.contains("button"),
        "{snapshot}"
    );
    // 位置、布尔配置与两个同步回调必须完整生成。
    assert!(
        snapshot.contains("BottomRight")
            && snapshot.contains("arrow (show_arrow)")
            && snapshot.contains("icon (true)")
            && snapshot.contains("on_confirm")
            && snapshot.contains("on_cancel"),
        "{snapshot}"
    );
    // 公共自动化标识仍由公共属性层消费。
    assert!(snapshot.contains("automation_id"), "{snapshot}");
}

// 验证一个静态 trigger 容器可以在内部保留普通控制流。
#[test]
fn allows_control_flow_inside_static_trigger_view() {
    // 唯一直接 Container 内部使用条件渲染。
    let snapshot = generate(r#"<Popconfirm title="确认"><Container><If {danger}><Button>删除</Button></If></Container></Popconfirm>"#).expect("静态 trigger 容器内部应保留普通控制流");
    // 唯一直接 trigger 必须生成容器 View。
    assert!(snapshot.contains("prelude :: column"), "{snapshot}");
    // 内部条件必须保留为 Rust 控制流。
    assert!(snapshot.contains("if danger"), "{snapshot}");
}

// 验证必需标题与直接 trigger View 的静态基数诊断。
#[test]
fn rejects_invalid_popconfirm_core_contracts() {
    // 缺失 title 时没有确认语义来源。
    let missing = generate(r#"<Popconfirm><Button>删除</Button></Popconfirm>"#)
        .expect_err("缺少 title 必须被拒绝");
    // 诊断必须点名 title。
    assert!(missing.message.contains("title"), "{missing:?}");
    // 空 Popconfirm 没有 trigger View。
    let empty = generate(r#"<Popconfirm title="确认" />"#).expect_err("空 Popconfirm 必须被拒绝");
    // 诊断必须点明唯一直接 trigger View。
    assert!(
        empty.message.contains("仅包含一个直接 trigger View"),
        "{empty:?}"
    );
    // 多个直接子节点会破坏唯一 trigger 身份。
    let multiple =
        generate(r#"<Popconfirm title="确认"><Button>一</Button><Button>二</Button></Popconfirm>"#)
            .expect_err("多个直接 trigger View 必须被拒绝");
    // 多子节点沿用相同静态基数诊断。
    assert!(
        multiple.message.contains("仅包含一个直接 trigger View"),
        "{multiple:?}"
    );
}

// 验证直接动态基数、非法位置与未登记事件不会静默降级。
#[test]
fn rejects_dynamic_trigger_and_unregistered_contracts() {
    // 直接 If 会令 trigger 是否存在依赖运行时条件。
    let dynamic =
        generate(r#"<Popconfirm title="确认"><If {show}><Button>删除</Button></If></Popconfirm>"#)
            .expect_err("直接 If trigger 必须被拒绝");
    // 诊断必须点明直接控制流边界。
    assert!(dynamic.message.contains("不能是 If 或 For"), "{dynamic:?}");
    // 非法 placement 不得静默回退为 top。
    let placement =
        generate(r#"<Popconfirm title="确认" placement="left"><Button>删除</Button></Popconfirm>"#)
            .expect_err("非法 placement 必须被拒绝");
    // 诊断必须列出有限位置集合。
    assert!(
        placement.message.contains("topLeft") && placement.message.contains("bottomRight"),
        "{placement:?}"
    );
    // 未登记 open 事件不能伪装成已支持回调。
    let event =
        generate(r#"<Popconfirm title="确认" @open="on_open"><Button>删除</Button></Popconfirm>"#)
            .expect_err("未登记事件必须被拒绝");
    // 未知事件诊断必须包含具体事件名。
    assert!(event.message.contains("@open"), "{event:?}");
}
